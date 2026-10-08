//! Minimal HTTP routing and server lifecycle, backed by Axum and Tokio.

mod error;
mod extract;
mod middleware;
mod openapi;
mod reply;
mod respond;
mod state;

use std::{collections::BTreeMap, future::Future, io, net::SocketAddr, sync::Arc};

use axum::{Router, handler::Handler, routing};
use ruvoraq_config::Env;
use state::Services;
use tokio::net::TcpListener;
use tower_http::catch_panic::CatchPanicLayer;

pub use axum::http::{HeaderMap, StatusCode};
pub use axum::response::{IntoResponse, Response};
pub use error::{Error, Result};
pub use extract::{Json, Path, Query, Validate, ValidatedJson};
pub use middleware::{Cors, RequestId, init_logging};
#[doc(hidden)]
pub use openapi::{DescribeSchema, SchemaProbe, operation, parameters};
pub use reply::{Reply, accepted, bad_request, created, invalid, no_content, not_found, ok};
#[doc(hidden)]
pub use respond::{HandlerOutput, Respond};
pub use schemars;
pub use state::Inject;
#[doc(hidden)]
pub use state::{Dependency, RequiredService, ServiceProbe};

/// The application identity and listening address.
///
/// settings.rs defaults can be overridden by a typed Env snapshot.
#[derive(Clone, Debug)]
pub struct Settings {
    pub app_name: String,
    pub address: SocketAddr,
}

impl Settings {
    /// Process/.env overrides take priority over settings.rs defaults.
    pub fn with_env(mut self, env: &Env) -> io::Result<Self> {
        self.app_name = env.get_or("RUVORAQ_APP_NAME", self.app_name)?;
        if self.app_name.trim().is_empty() || self.app_name.chars().any(char::is_control) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "RUVORAQ_APP_NAME must be non-empty text without control characters",
            ));
        }
        let host = env.get_or("RUVORAQ_HOST", self.address.ip())?;
        let port = env.get_or("RUVORAQ_PORT", self.address.port())?;
        self.address = SocketAddr::new(host, port);
        Ok(self)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            app_name: "ruvoraq".into(),
            address: SocketAddr::from(([127, 0, 0, 1], 8000)),
        }
    }
}

/// A small HTTP application with explicit route registration.
///
/// Handler and response conversion currently use Axum's traits. Invalid or
/// conflicting routes follow Axum's registration rules and may panic.
pub struct App {
    router: Router,
    settings: Settings,
    services: Services,
    required: Vec<(Dependency, &'static str, &'static str)>,
    documentation: Vec<(&'static str, &'static str, serde_json::Value)>,
    docs_enabled: bool,
    route_paths: Vec<String>,
    environment: Env,
    policy: middleware::Policy,
}

impl Default for App {
    fn default() -> Self {
        Self {
            router: Router::new(),
            settings: Settings::default(),
            services: Services::default(),
            required: Vec::new(),
            documentation: Vec::new(),
            docs_enabled: true,
            route_paths: Vec::new(),
            environment: Env::default(),
            policy: middleware::Policy::default(),
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an application from route attributes linked into this executable.
    ///
    /// Registration order is deterministic. Duplicate method/path pairs and
    /// applications with no registered routes return errors before server startup.
    pub fn auto() -> io::Result<Self> {
        let mut routes: Vec<_> = inventory::iter::<RouteRegistration>.into_iter().collect();
        routes.sort_by_key(|route| (route.path, route.method));
        if routes.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no Ruvoraq routes registered; add #[get(\"/\")] to an async handler",
            ));
        }
        let mut shapes = BTreeMap::new();
        let mut previous = None;
        let mut app = Self::new();
        for route in routes {
            let key = (route.path, route.method);
            if previous == Some(key) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("duplicate Ruvoraq route: {} {}", route.method, route.path),
                ));
            }
            let shape = route
                .path
                .split('/')
                .map(|segment| {
                    if segment.starts_with('{') {
                        "{}"
                    } else {
                        segment
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            if let Some(existing) = shapes.insert(shape, route.path) {
                if existing != route.path {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "conflicting Ruvoraq route patterns: {existing} and {}; use identical parameter names for shared paths",
                            route.path
                        ),
                    ));
                }
            }
            previous = Some(key);
            app.required.extend(
                (route.dependencies)()
                    .into_iter()
                    .map(|dependency| (dependency, route.method, route.path)),
            );
            app.documentation
                .push((route.path, route.method, (route.document)()));
            app = (route.register)(app);
        }
        Ok(app)
    }

    /// Expose the Axum adapter with fallbacks, panic handling and shared services.
    ///
    /// For standalone router use, call check() first to validate attribute routes.
    /// run() and serve() perform this check automatically.
    pub fn into_router(mut self) -> Router {
        if self.docs_enabled {
            let document = self.openapi();
            self.router = openapi::mount(self.router, document);
        }
        let policy = self.policy;
        let router = self
            .router
            .fallback(|| async { Error::not_found("Route not found") })
            .method_not_allowed_fallback(|| async { Error::method_not_allowed() })
            .layer(CatchPanicLayer::custom(
                |_: Box<dyn std::any::Any + Send>| Error::internal().into_response(),
            ))
            .layer(axum::Extension(Arc::new(self.services)));
        middleware::apply(router, policy)
    }

    /// Register one shared instance of a concrete service type.
    ///
    /// Registering the same type again replaces the previous provider.
    pub fn provide<T: Send + Sync + 'static>(self, service: T) -> Self {
        self.provide_shared(Arc::new(service))
    }

    /// Register an existing Arc without wrapping it in another Arc.
    pub fn provide_shared<T: Send + Sync + 'static>(mut self, service: Arc<T>) -> Self {
        self.services.insert(service);
        self
    }

    /// Validate dependencies of attribute-registered routes without binding.
    pub fn check(&self) -> io::Result<()> {
        self.policy.validate()?;
        if self.docs_enabled {
            for path in &self.route_paths {
                if openapi::reserved(path) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("route {path} conflicts with built-in docs; use App::docs(false)"),
                    ));
                }
            }
        }
        for &(dependency, method, path) in &self.required {
            if !self.services.contains(dependency) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "missing dependency '{}' required by {method} {path}; register it with App::provide(...) in settings.rs",
                        dependency.name
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Apply a configuration snapshot, then expose it through `env()` and `Inject<Env>`.
    /// Bootstrap calls this automatically; explicit App builders opt in.
    pub fn environment(mut self, env: Env) -> io::Result<Self> {
        self.settings = self.settings.with_env(&env)?;
        self.docs_enabled = env.get_or("RUVORAQ_DOCS", self.docs_enabled)?;
        self.policy.ids = env.get_or("RUVORAQ_REQUEST_ID", self.policy.ids)?;
        self.policy.logging = env.get_or("RUVORAQ_REQUEST_LOG", self.policy.logging)?;
        if let Some(milliseconds) = env.optional::<u64>("RUVORAQ_REQUEST_TIMEOUT_MS")? {
            self.policy.timeout = Some(std::time::Duration::from_millis(milliseconds));
        }
        self.policy.validate()?;
        self.environment = env.clone();
        Ok(self.provide(env))
    }

    /// Read custom configuration in a configure hook without loading files again.
    pub fn env(&self) -> &Env {
        &self.environment
    }

    /// Enable/disable validated request IDs (enabled by default).
    pub fn request_ids(mut self, enabled: bool) -> Self {
        self.policy.ids = enabled;
        self
    }

    /// Emit structured tracing events. Bootstrap initializes JSON logging when enabled.
    pub fn request_logging(mut self, enabled: bool) -> Self {
        self.policy.logging = enabled;
        self
    }

    pub fn logging_enabled(&self) -> bool {
        self.policy.logging
    }

    /// Bound time until response headers; streaming bodies and blocking work are not bounded.
    pub fn request_timeout(mut self, duration: std::time::Duration) -> Self {
        self.policy.timeout = Some(duration);
        self
    }

    pub fn without_request_timeout(mut self) -> Self {
        self.policy.timeout = None;
        self
    }

    /// Apply an explicit browser-origin policy.
    pub fn cors(mut self, cors: Cors) -> Self {
        self.policy.cors = Some(cors);
        self
    }

    /// Disable built-in documentation endpoints, for example in production.
    pub fn docs(mut self, enabled: bool) -> Self {
        self.docs_enabled = enabled;
        self
    }

    /// Generate OpenAPI metadata for attribute-registered routes.
    pub fn openapi(&self) -> serde_json::Value {
        openapi::document(&self.settings.app_name, &self.documentation)
    }

    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Register GET; Axum also supplies HEAD for this route.
    pub fn get<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.route_paths.push(path.to_owned());
        self.router = self.router.route(path, routing::get(handler));
        self
    }

    pub fn post<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.route_paths.push(path.to_owned());
        self.router = self.router.route(path, routing::post(handler));
        self
    }

    pub fn put<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.route_paths.push(path.to_owned());
        self.router = self.router.route(path, routing::put(handler));
        self
    }

    pub fn patch<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.route_paths.push(path.to_owned());
        self.router = self.router.route(path, routing::patch(handler));
        self
    }

    pub fn delete<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.route_paths.push(path.to_owned());
        self.router = self.router.route(path, routing::delete(handler));
        self
    }

    /// Bind the configured address and drain active requests on shutdown.
    ///
    /// Handles Ctrl+C on Unix/Windows and SIGTERM on Unix.
    pub async fn run(self) -> io::Result<()> {
        self.check()?;
        let address = self.settings.address;
        let listener = TcpListener::bind(address).await.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot bind server to {address}: {error}"),
            )
        })?;
        let shutdown = shutdown_signal()?;
        self.serve(listener, shutdown).await
    }

    /// Serve an already-bound listener with an application-provided shutdown signal.
    ///
    /// This is the adapter's escape hatch for tests and lifecycle integrations.
    /// Port 0 listeners report the actual assigned address.
    pub async fn serve<F>(self, listener: TcpListener, shutdown: F) -> io::Result<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.check()?;
        println!("Ruvoraq");
        println!("Application: {}", self.settings.app_name);
        println!("Server:      http://{}", listener.local_addr()?);
        if self.docs_enabled {
            println!("Docs:        http://{}/docs", listener.local_addr()?);
        }
        println!("Ready");

        axum::serve(listener, self.into_router())
            .with_graceful_shutdown(shutdown)
            .await?;
        println!("Stopped");
        Ok(())
    }
}

fn shutdown_signal() -> io::Result<impl Future<Output = ()> + Send> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut interrupt = signal(SignalKind::interrupt())?;
        let mut terminate = signal(SignalKind::terminate())?;
        Ok(async move {
            tokio::select! {
                _ = interrupt.recv() => {}
                _ = terminate.recv() => {}
            }
        })
    }
    #[cfg(windows)]
    {
        let mut interrupt = tokio::signal::windows::ctrl_c()?;
        Ok(async move {
            interrupt.recv().await;
        })
    }
}

/// Internal route descriptor emitted by Ruvoraq's attribute macros.
#[doc(hidden)]
pub struct RouteRegistration {
    pub method: &'static str,
    pub path: &'static str,
    pub register: fn(App) -> App,
    pub dependencies: fn() -> Vec<Dependency>,
    pub document: fn() -> serde_json::Value,
}

inventory::collect!(RouteRegistration);

#[doc(hidden)]
pub use inventory;
