use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use ruvoraq::prelude::*;
use tower::ServiceExt;

// Deliberately not Clone: requests share the service, not copies of its state.
struct Counter(AtomicUsize);
struct Greeting(String);
type Visits = Inject<Counter>;

#[get("/alias")]
async fn visits(counter: Visits) -> Value {
    json!({"visits":counter.0.0.fetch_add(1, Ordering::SeqCst) + 1})
}

#[derive(Deserialize)]
struct Increment {
    amount: usize,
}

#[post("/count")]
async fn increment(counter: Inject<Counter>, Json(input): Json<Increment>) -> Value {
    json!({"visits":counter.0.0.fetch_add(input.amount, Ordering::SeqCst) + input.amount})
}

#[get("/shared")]
async fn same_service(first: Inject<Counter>, second: Inject<Counter>) -> Value {
    let cloned = first.clone();
    json!({"shared":Arc::ptr_eq(&first.0, &second.0) && Arc::ptr_eq(&first.0, &cloned.0)})
}

impl Greeting {
    async fn hello(&self) -> String {
        tokio::task::yield_now().await;
        self.0.clone()
    }
}

#[get("/greeting")]
async fn greeting(service: ruvoraq::Inject<Greeting>) -> Value {
    json!({"message":service.hello().await})
}

async fn call(router: axum::Router, method: &str, path: &str, body: &str) -> (StatusCode, Value) {
    let response = router
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

fn configured(counter: Arc<Counter>) -> App {
    App::auto()
        .unwrap()
        .provide_shared(counter)
        .provide(Greeting("Hello Babar".into()))
}

#[tokio::test]
async fn alias_dependencies_are_checked_before_binding() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let error = App::auto()
        .unwrap()
        .settings(Settings {
            address: listener.local_addr().unwrap(),
            ..Settings::default()
        })
        .run()
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    let message = error.to_string();
    assert!(
        message.contains("Counter") && message.contains("GET /alias"),
        "{message}"
    );
    assert!(
        message.contains("App::provide") && message.contains("settings.rs"),
        "{message}"
    );
    let error = App::auto()
        .unwrap()
        .provide(Counter(AtomicUsize::new(0)))
        .check()
        .unwrap_err();
    assert!(error.to_string().contains("Greeting"));
    configured(Arc::new(Counter(AtomicUsize::new(0))))
        .check()
        .unwrap();
}

#[tokio::test]
async fn concurrent_requests_share_one_service_instance() {
    let service = Arc::new(Counter(AtomicUsize::new(0)));
    let router = configured(service.clone()).into_router();
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let router = router.clone();
        requests.spawn(async move { call(router, "POST", "/count", r#"{"amount":1}"#).await });
    }
    let mut counts = Vec::new();
    while let Some(result) = requests.join_next().await {
        let (status, value) = result.unwrap();
        assert_eq!(status, StatusCode::OK);
        counts.push(value["visits"].as_u64().unwrap());
    }
    counts.sort();
    assert_eq!(counts, (1..=16).collect::<Vec<_>>());
    assert_eq!(service.0.load(Ordering::SeqCst), 16);
    assert_eq!(
        call(router.clone(), "GET", "/alias", "").await.1,
        json!({"visits":17})
    );
    assert_eq!(
        call(router, "GET", "/shared", "").await.1,
        json!({"shared":true})
    );
}

#[tokio::test]
async fn multiple_types_async_service_methods_and_json_work_together() {
    let service = Arc::new(Counter(AtomicUsize::new(0)));
    let router = configured(service).into_router();
    assert_eq!(
        call(router.clone(), "GET", "/greeting", "").await.1,
        json!({"message":"Hello Babar"})
    );
    let (status, body) = call(router, "POST", "/count", r#"{"amount":3}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"visits":3}));
}

#[tokio::test]
async fn missing_service_in_explicit_routes_returns_a_generic_json_error() {
    let router = App::new()
        .get("/missing", |_: Inject<Counter>| async { "unreachable" })
        .into_router();
    let (status, body) = call(router, "GET", "/missing", "").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        json!({"error":{"code":"internal_error","message":"Internal server error","details":{}}})
    );
    assert!(!body.to_string().contains("Counter"));
}

#[tokio::test]
async fn later_provider_replaces_an_earlier_provider_of_the_same_type() {
    let app = App::auto()
        .unwrap()
        .provide(Counter(AtomicUsize::new(0)))
        .provide(Counter(AtomicUsize::new(10)))
        .provide(Greeting("Hello".into()));
    app.check().unwrap();
    assert_eq!(
        call(app.into_router(), "GET", "/alias", "").await.1,
        json!({"visits":11})
    );
}

#[tokio::test]
async fn services_are_scoped_to_each_application() {
    let first = configured(Arc::new(Counter(AtomicUsize::new(0)))).into_router();
    let second = configured(Arc::new(Counter(AtomicUsize::new(0)))).into_router();
    assert_eq!(
        call(first.clone(), "GET", "/alias", "").await.1,
        json!({"visits":1})
    );
    assert_eq!(
        call(first, "GET", "/alias", "").await.1,
        json!({"visits":2})
    );
    assert_eq!(
        call(second, "GET", "/alias", "").await.1,
        json!({"visits":1})
    );
}
