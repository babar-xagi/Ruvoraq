//! Minimal HTTP routing and server lifecycle, backed by Axum and Tokio.

use std::{future::Future, io, net::SocketAddr};

use axum::{Router, handler::Handler, routing};
use tokio::net::TcpListener;

/// The application identity and listening address.
///
/// Environment profiles and configuration loading are deferred to later phases.
#[derive(Clone, Debug)]
pub struct Settings {
    pub app_name: String,
    pub address: SocketAddr,
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
#[derive(Default)]
pub struct App {
    router: Router,
    settings: Settings,
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
            previous = Some(key);
            app = (route.register)(app);
        }
        Ok(app)
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
        self.router = self.router.route(path, routing::get(handler));
        self
    }

    pub fn post<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::post(handler));
        self
    }

    pub fn put<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::put(handler));
        self
    }

    pub fn patch<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::patch(handler));
        self
    }

    pub fn delete<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::delete(handler));
        self
    }

    /// Bind the configured address and drain active requests on shutdown.
    ///
    /// Handles Ctrl+C on Unix/Windows and SIGTERM on Unix.
    pub async fn run(self) -> io::Result<()> {
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
        println!("Ruvoraq");
        println!("Application: {}", self.settings.app_name);
        println!("Server:      http://{}", listener.local_addr()?);
        println!("Ready");

        axum::serve(listener, self.router)
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
}

inventory::collect!(RouteRegistration);

#[doc(hidden)]
pub use inventory;
