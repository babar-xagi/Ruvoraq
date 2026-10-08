use std::{
    io,
    time::{Duration, Instant},
};

use axum::{
    Router,
    extract::{FromRequestParts, MatchedPath, Request, State},
    http::{HeaderMap, HeaderValue, Method, Uri, header, request::Parts},
    middleware::{self, Next},
    response::Response,
};
use tower::{Layer, ServiceExt};
use tower_http::cors::CorsLayer;
use tracing::Instrument;

use crate::{Error, IntoResponse};

/// A validated correlation identifier shared by the request and response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestId(String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S: Send + Sync> FromRequestParts<S> for RequestId {
    type Rejection = Error;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Error> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or_else(Error::internal)
    }
}

/// An explicit browser-origin allowlist. CORS is not authentication.
#[derive(Clone, Debug)]
pub struct Cors {
    origins: Vec<HeaderValue>,
    credentials: bool,
}

impl Cors {
    /// Accept HTTP(S) origins such as `https://example.com`, without paths or wildcards.
    pub fn new(origins: impl IntoIterator<Item = impl AsRef<str>>) -> io::Result<Self> {
        Self::parse(
            origins
                .into_iter()
                .map(|value| value.as_ref().to_owned())
                .collect(),
        )
    }

    fn parse(origins: Vec<String>) -> io::Result<Self> {
        let mut values = Vec::new();
        for origin in origins {
            let origin = origin.as_str();
            let invalid = || {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "CORS requires HTTP(S) origins without paths, queries, credentials or wildcards",
                )
            };
            let uri: Uri = origin.parse().map_err(|_| invalid())?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri.authority().is_none()
                || uri.authority().is_some_and(|a| a.as_str().contains('@'))
                || uri.path() != "/"
                || uri.query().is_some()
                || origin.ends_with('/')
                || origin.contains('*')
                || origin.contains('#')
            {
                return Err(invalid());
            }
            let authority = uri.authority().expect("checked origin authority").as_str();
            let port = if authority.starts_with('[') {
                authority
                    .rsplit_once(']')
                    .and_then(|(_, suffix)| suffix.strip_prefix(':'))
            } else {
                authority.rsplit_once(':').map(|(_, port)| port)
            };
            if uri
                .authority()
                .expect("checked origin authority")
                .host()
                .is_empty()
                || port.is_some_and(|port| port.parse::<u16>().is_err())
            {
                return Err(invalid());
            }
            let value = HeaderValue::from_str(origin).expect("validated origins are HTTP headers");
            if !values.contains(&value) {
                values.push(value);
            }
        }
        if values.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "CORS requires at least one allowed origin",
            ));
        }
        Ok(Self {
            origins: values,
            credentials: false,
        })
    }

    /// Enable credentialed browser requests for this explicit allowlist.
    pub fn credentials(mut self, enabled: bool) -> Self {
        self.credentials = enabled;
        self
    }

    fn layer(self) -> CorsLayer {
        CorsLayer::new()
            .allow_origin(self.origins)
            .allow_methods([
                Method::GET,
                Method::HEAD,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::HeaderName::from_static("x-request-id"),
            ])
            .expose_headers([header::HeaderName::from_static("x-request-id")])
            .allow_credentials(self.credentials)
    }
}

#[derive(Clone)]
pub(crate) struct Policy {
    pub ids: bool,
    pub logging: bool,
    pub timeout: Option<Duration>,
    pub cors: Option<Cors>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            ids: true,
            logging: false,
            timeout: None,
            cors: None,
        }
    }
}

impl Policy {
    pub fn validate(&self) -> io::Result<()> {
        if self.timeout.is_some_and(|duration| duration.is_zero()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "request timeout must be greater than zero",
            ));
        }
        Ok(())
    }
}

/// Install JSON tracing on stderr at the binary entry point.
/// Existing application subscribers are preserved. Repeated calls are harmless.
pub fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .try_init();
}

pub(crate) fn apply(router: Router, policy: Policy) -> Router {
    let mut router = router.layer(middleware::from_fn_with_state(policy.timeout, deadline));
    if let Some(cors) = policy.cors.clone() {
        router = router.layer(middleware::from_fn_with_state(cors.layer(), browser_policy));
    }
    router.layer(middleware::from_fn_with_state(policy, handle))
}

fn identifier(headers: &HeaderMap) -> HeaderValue {
    if headers.get_all("x-request-id").iter().count() == 1 {
        let value = &headers["x-request-id"];
        if value.to_str().is_ok_and(|id| {
            !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        }) {
            return value.clone();
        }
    }
    HeaderValue::from_str(&uuid::Uuid::new_v4().to_string()).expect("UUIDs are valid HTTP headers")
}

async fn handle(State(policy): State<Policy>, mut request: Request, next: Next) -> Response {
    let started = Instant::now();
    let id = policy.ids.then(|| identifier(request.headers()));
    if let Some(id) = &id {
        request.headers_mut().insert("x-request-id", id.clone());
        request.extensions_mut().insert(RequestId(
            id.to_str().expect("validated request ID").to_owned(),
        ));
    }
    let method = request.method().to_string();
    // Log the static route pattern, never URL parameters, query strings or bodies.
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str())
        .unwrap_or("<unmatched>")
        .to_owned();
    let correlation = id.as_ref().and_then(|id| id.to_str().ok()).unwrap_or("");
    let span = if policy.logging {
        tracing::info_span!("http.request", request_id = correlation, method = %method, route = %route)
    } else {
        tracing::Span::none()
    };
    let response = next.run(request).instrument(span).await;
    let mut response = response;
    if policy.logging {
        tracing::info!(target: "ruvoraq::http", request_id = correlation, method = %method, route = %route,
            status = response.status().as_u16(), duration_ms = started.elapsed().as_millis() as u64, "request completed");
    }
    if let Some(id) = id {
        response.headers_mut().insert("x-request-id", id);
    }
    response
}

async fn deadline(
    State(duration): State<Option<Duration>>,
    request: Request,
    next: Next,
) -> Response {
    match duration {
        Some(duration) => match tokio::time::timeout(duration, next.run(request)).await {
            Ok(response) => response,
            Err(_) => Error::new(
                axum::http::StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
                "Request timed out",
            )
            .into_response(),
        },
        None => next.run(request).await,
    }
}

async fn browser_policy(State(cors): State<CorsLayer>, request: Request, next: Next) -> Response {
    if request.method() == Method::OPTIONS
        && (!request.headers().contains_key(header::ORIGIN)
            || !request
                .headers()
                .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD))
    {
        return next.run(request).await;
    }
    cors.layer(next)
        .oneshot(request)
        .await
        .expect("the router is infallible")
}
