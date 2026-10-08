use axum::{
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode},
};
use ruvoraq_web::{App, Cors, Error, RequestId, init_logging};
use std::{
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tower::ServiceExt;
use tracing::instrument::WithSubscriber;

async fn send(
    app: axum::Router,
    request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| serde_json::json!({"text": String::from_utf8_lossy(&bytes)}));
    (status, headers, body)
}

fn get(path: &str) -> Request<Body> {
    Request::builder().uri(path).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn request_ids_validate_propagate_and_replace_untrusted_values() {
    let router = App::new().get("/", |id: RequestId, headers: axum::http::HeaderMap| async move {
        axum::Json(serde_json::json!({"id": id.as_str(), "header": headers["x-request-id"].to_str().unwrap()}))
    }).into_router();
    for input in [
        None,
        Some("client-1.a_b"),
        Some("a"),
        Some(""),
        Some("two words"),
        Some("bad/secret"),
        Some("a:b"),
    ] {
        let mut request = get("/");
        if let Some(input) = input {
            request
                .headers_mut()
                .insert("x-request-id", HeaderValue::from_str(input).unwrap());
        }
        let (status, headers, body) = send(router.clone(), request).await;
        assert_eq!(status, StatusCode::OK);
        let id = headers["x-request-id"].to_str().unwrap();
        assert_eq!(body["id"], id);
        assert_eq!(body["header"], id);
        if matches!(input, Some("client-1.a_b" | "a")) {
            assert_eq!(Some(id), input);
        } else {
            assert_eq!(uuid::Uuid::parse_str(id).unwrap().get_version_num(), 4);
        }
    }
    for (length, preserved) in [(64, true), (65, false)] {
        let input = "a".repeat(length);
        let mut request = get("/");
        request
            .headers_mut()
            .insert("x-request-id", HeaderValue::from_str(&input).unwrap());
        let (_, headers, _) = send(router.clone(), request).await;
        assert_eq!(headers["x-request-id"] == input, preserved);
    }
    let mut request = get("/");
    request
        .headers_mut()
        .insert("x-request-id", HeaderValue::from_bytes(&[0x80]).unwrap());
    let (_, headers, _) = send(router.clone(), request).await;
    assert!(uuid::Uuid::parse_str(headers["x-request-id"].to_str().unwrap()).is_ok());
    let mut request = get("/");
    request
        .headers_mut()
        .append("x-request-id", HeaderValue::from_static("one"));
    request
        .headers_mut()
        .append("x-request-id", HeaderValue::from_static("two"));
    let (_, headers, _) = send(router, request).await;
    assert!(uuid::Uuid::parse_str(headers["x-request-id"].to_str().unwrap()).is_ok());
}

#[tokio::test]
async fn ids_are_unique_under_concurrency_and_override_handler_spoofing() {
    let router = App::new()
        .get("/", || async { ([("x-request-id", "spoofed")], "ok") })
        .into_router();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..64 {
        let router = router.clone();
        tasks.spawn(async move { send(router, get("/")).await.1["x-request-id"].clone() });
    }
    let mut ids = std::collections::HashSet::new();
    while let Some(result) = tasks.join_next().await {
        let id = result.unwrap();
        assert_ne!(id, "spoofed");
        assert!(ids.insert(id));
    }
    assert_eq!(ids.len(), 64);
}

#[tokio::test]
async fn ids_cover_errors_panics_methods_and_documentation() {
    let router = App::new()
        .get("/panic", || async {
            panic!("private-secret");
            #[allow(unreachable_code)]
            "never"
        })
        .get("/failure", || async { Error::internal() })
        .into_router();
    for (method, path, expected) in [
        ("GET", "/missing", 404),
        ("POST", "/failure", 405),
        ("GET", "/failure", 500),
        ("GET", "/panic", 500),
        ("GET", "/docs", 200),
        ("GET", "/openapi.json", 200),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let (status, headers, body) = send(router.clone(), request).await;
        assert_eq!(status.as_u16(), expected);
        assert!(headers.contains_key("x-request-id"));
        assert!(!body.to_string().contains("private-secret"));
    }
}

#[tokio::test]
async fn disabled_request_ids_and_extractor_rejection_are_explicit() {
    let app = App::new()
        .request_ids(false)
        .get("/", || async { "ok" })
        .get("/id", |_: RequestId| async { "id" });
    let router = app.into_router();
    let (_, headers, _) = send(router.clone(), get("/")).await;
    assert!(!headers.contains_key("x-request-id"));
    let (status, headers, body) = send(router, get("/id")).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!headers.contains_key("x-request-id"));
    assert_eq!(body["error"]["code"], "internal_error");
}

#[test]
fn cors_rejects_unsafe_or_empty_configuration() {
    assert!(Cors::new(Vec::<String>::new()).is_err());
    assert!(Cors::new(vec!["https://example.com".to_owned()]).is_ok());
    for origin in [
        "",
        "*",
        "null",
        "ftp://example.com",
        "http:/path",
        "https://user:pass@example.com",
        "https://example.com/path",
        "https://example.com?secret=yes",
        "https://example.com/",
        "https://example.com/#fragment",
        "https://example.com\n",
    ] {
        let error = Cors::new([origin]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!error.to_string().contains("secret"));
    }
    assert!(
        Cors::new([
            "http://localhost:3000",
            "https://example.com",
            "http://[::1]:3000",
            "https://example.com"
        ])
        .is_ok()
    );
}

#[tokio::test]
async fn cors_actual_requests_allow_only_the_explicit_origin() {
    let router = App::new()
        .cors(
            Cors::new(["https://app.example"])
                .unwrap()
                .credentials(true),
        )
        .get("/", || async { "ok" })
        .into_router();
    for (origin, allowed) in [
        (Some("https://app.example"), true),
        (Some("https://evil.example"), false),
        (Some("null"), false),
        (None, false),
    ] {
        let mut request = get("/");
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert("origin", HeaderValue::from_static(origin));
        }
        let (status, headers, _) = send(router.clone(), request).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.contains_key("access-control-allow-origin"), allowed);
        if allowed {
            assert_eq!(
                headers["access-control-allow-origin"],
                "https://app.example"
            );
            assert_eq!(headers["access-control-allow-credentials"], "true");
            assert_eq!(headers["access-control-expose-headers"], "x-request-id");
        }
        assert!(headers["vary"].to_str().unwrap().contains("origin"));
    }
}

#[tokio::test]
async fn cors_preflight_includes_request_id_and_never_runs_the_handler() {
    let called = Arc::new(AtomicBool::new(false));
    let observed = called.clone();
    let router = App::new()
        .cors(Cors::new(["http://localhost:3000"]).unwrap())
        .post("/", move || {
            let observed = observed.clone();
            async move {
                observed.store(true, Ordering::SeqCst);
                "ok"
            }
        })
        .into_router();
    for (origin, allowed) in [("http://localhost:3000", true), ("http://untrusted", false)] {
        let request = Request::builder()
            .method("OPTIONS")
            .uri("/")
            .header("origin", origin)
            .header("access-control-request-method", "POST")
            .header(
                "access-control-request-headers",
                "content-type,authorization,x-request-id",
            )
            .body(Body::empty())
            .unwrap();
        let (status, headers, body) = send(router.clone(), request).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["text"], "");
        assert_eq!(headers.contains_key("access-control-allow-origin"), allowed);
        assert!(headers.contains_key("x-request-id"));
        assert!(!headers.contains_key("access-control-allow-credentials"));
        assert!(
            headers["access-control-allow-methods"]
                .to_str()
                .unwrap()
                .contains("POST")
        );
    }
    assert!(!called.load(Ordering::SeqCst));
}

#[tokio::test(start_paused = true)]
async fn timeout_cancels_pending_handlers_and_preserves_cors_and_ids() {
    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let observed = dropped.clone();
    let completed = finished.clone();
    let router = App::new()
        .request_timeout(Duration::from_millis(10))
        .cors(Cors::new(["https://app.example"]).unwrap())
        .get("/slow", move || {
            let observed = observed.clone();
            let completed = completed.clone();
            async move {
                let _guard = Guard(observed);
                tokio::time::sleep(Duration::from_secs(1)).await;
                completed.store(true, Ordering::SeqCst);
                "done"
            }
        })
        .get("/fast", || async { "fast" })
        .into_router();
    let mut request = get("/slow");
    request
        .headers_mut()
        .insert("origin", HeaderValue::from_static("https://app.example"));
    let (status, headers, body) = send(router.clone(), request).await;
    assert_eq!(status, StatusCode::REQUEST_TIMEOUT);
    assert_eq!(body["error"]["code"], "request_timeout");
    assert!(headers.contains_key("x-request-id"));
    // Timeout is outside CORS: middleware must add its own policy to the timeout response.
    assert_eq!(
        headers["access-control-allow-origin"],
        "https://app.example"
    );
    assert!(dropped.load(Ordering::SeqCst));
    assert!(!finished.load(Ordering::SeqCst));
    assert_eq!(send(router, get("/fast")).await.0, StatusCode::OK);
}

#[tokio::test(start_paused = true)]
async fn timeout_can_be_removed_and_disabled_defaults_do_not_cancel() {
    let router = App::new()
        .request_timeout(Duration::from_millis(1))
        .without_request_timeout()
        .get("/", || async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            "done"
        })
        .into_router();
    assert_eq!(send(router, get("/")).await.0, StatusCode::OK);
    assert!(App::new().request_timeout(Duration::ZERO).check().is_err());
    assert!(
        App::new()
            .request_timeout(Duration::ZERO)
            .without_request_timeout()
            .check()
            .is_ok()
    );
}

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn structured_logs_use_route_patterns_and_redact_request_contents() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(captured.clone());
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_writer(move || writer.clone())
        .without_time()
        .finish();
    let router = App::new()
        .request_logging(true)
        .get("/users/{id}", || async { "ok" })
        .into_router();
    let request = Request::builder()
        .uri("/users/private-secret?token=private-secret")
        .header("authorization", "Bearer private-secret")
        .header("x-request-id", "trace-1")
        .body(Body::from("private-secret"))
        .unwrap();
    send(router.clone(), request)
        .with_subscriber(subscriber)
        .await;
    let bytes = captured.lock().unwrap().clone();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("private-secret"));
    let record: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(record["fields"]["route"], "/users/{id}");
    assert_eq!(record["fields"]["request_id"], "trace-1");
    assert_eq!(record["fields"]["status"], 200);
    assert!(record["fields"]["duration_ms"].is_number());
    assert!(!App::new().logging_enabled());
    assert!(App::new().request_logging(true).logging_enabled());
    init_logging();
    init_logging();
}

#[tokio::test]
async fn logging_without_ids_and_unmatched_routes_is_safe() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(captured.clone());
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_writer(move || writer.clone())
        .without_time()
        .finish();
    let router = App::new()
        .request_logging(true)
        .request_ids(false)
        .into_router();
    send(router, get("/private-secret?token=private-secret"))
        .with_subscriber(subscriber)
        .await;
    let text = String::from_utf8(captured.lock().unwrap().clone()).unwrap();
    assert!(!text.contains("private-secret"));
    let record: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(record["fields"]["route"], "<unmatched>");
    assert_eq!(record["fields"]["request_id"], "");
    assert_eq!(record["fields"]["status"], 404);
}

#[test]
fn typed_environment_controls_policy_and_rejects_invalid_values() {
    fn environment(content: &str) -> ruvoraq_config::Env {
        let dir = std::env::temp_dir().join(format!("ruvoraq-policy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join(".env"), content).unwrap();
        let env = ruvoraq_config::Env::load_from(&dir).unwrap();
        std::fs::remove_file(dir.join(".env")).unwrap();
        std::fs::remove_dir(dir).unwrap();
        env
    }
    let app = App::new()
        .environment(environment(
            "RUVORAQ_REQUEST_ID=false\nRUVORAQ_REQUEST_LOG=true\nRUVORAQ_REQUEST_TIMEOUT_MS=10\n",
        ))
        .unwrap();
    assert!(app.logging_enabled());
    assert!(app.check().is_ok());
    for content in [
        "RUVORAQ_REQUEST_ID=private-secret\n",
        "RUVORAQ_REQUEST_LOG=private-secret\n",
        "RUVORAQ_REQUEST_TIMEOUT_MS=private-secret\n",
        "RUVORAQ_REQUEST_TIMEOUT_MS=0\n",
    ] {
        let error = App::new().environment(environment(content)).err().unwrap();
        assert!(!error.to_string().contains("private-secret"));
    }
}

#[tokio::test]
async fn cors_preserves_ordinary_options_method_errors() {
    let router = App::new()
        .cors(Cors::new(["https://app.example"]).unwrap())
        .get("/", || async { "ok" })
        .into_router();
    for headers in [
        vec![],
        vec![("origin", "https://app.example")],
        vec![("access-control-request-method", "GET")],
    ] {
        let mut request = Request::builder()
            .method("OPTIONS")
            .uri("/")
            .body(Body::empty())
            .unwrap();
        for (name, value) in headers {
            request
                .headers_mut()
                .insert(name, HeaderValue::from_static(value));
        }
        let (status, headers, body) = send(router.clone(), request).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(body["error"]["code"], "method_not_allowed");
        assert!(headers.contains_key("allow"));
        assert!(headers.contains_key("x-request-id"));
    }
}

#[test]
fn cors_validates_ports_and_accepts_ipv6_without_a_port() {
    for origin in [
        "http://example.com:65536",
        "http://example.com:invalid",
        "http://example.com:",
        "http://*",
        "https://*.example.com",
        "http://:80",
    ] {
        assert!(
            Cors::new([origin]).is_err(),
            "accepted invalid origin: {origin}"
        );
    }
    assert!(Cors::new(["http://[::1]", "http://example.com:65535"]).is_ok());
}

#[tokio::test]
async fn handler_tracing_events_inherit_request_context() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(captured.clone());
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_writer(move || writer.clone())
        .without_time()
        .finish();
    let router = App::new()
        .request_logging(true)
        .get("/span", || async {
            tracing::info!(target: "application", "handler event");
            "ok"
        })
        .into_router();
    let request = Request::builder()
        .uri("/span")
        .header("x-request-id", "span-123")
        .body(Body::empty())
        .unwrap();
    send(router, request).with_subscriber(subscriber).await;
    let text = String::from_utf8(captured.lock().unwrap().clone()).unwrap();
    let records: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let record = records
        .iter()
        .find(|record| record["target"] == "application")
        .unwrap();
    assert_eq!(record["span"]["request_id"], "span-123");
    assert_eq!(record["span"]["route"], "/span");
}
