use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use ruvoraq::prelude::*;
use tower::ServiceExt;

#[get("/config")]
async fn config(env: Inject<Env>) -> Value {
    json!({"limit":env.get::<usize>("CUSTOM_LIMIT").unwrap()})
}
fn env(values: &[(&str, &str)]) -> Env {
    Env::from_values(values.iter().copied())
}

#[test]
fn settings_overrides_preserve_defaults_and_support_ipv6_and_port_zero() {
    let original = Settings {
        app_name: "default-name".into(),
        address: "127.0.0.2:9000".parse().unwrap(),
    };
    let unchanged = original.clone().with_env(&env(&[])).unwrap();
    assert_eq!(unchanged.app_name, original.app_name);
    assert_eq!(unchanged.address, original.address);
    let configured = original
        .with_env(&env(&[
            ("RUVORAQ_HOST", "::1"),
            ("RUVORAQ_PORT", "0"),
            ("RUVORAQ_APP_NAME", "Config app"),
        ]))
        .unwrap();
    assert_eq!(configured.address, "[::1]:0".parse().unwrap());
    assert_eq!(configured.app_name, "Config app");
}
#[test]
fn invalid_built_in_values_are_named_but_not_printed() {
    for (key, value) in [
        ("RUVORAQ_HOST", "bad-secret-host"),
        ("RUVORAQ_PORT", "secret-port"),
        ("RUVORAQ_PORT", "65536"),
        ("RUVORAQ_APP_NAME", ""),
        ("RUVORAQ_APP_NAME", "bad\nname"),
        ("RUVORAQ_DOCS", "secret-bool"),
    ] {
        let result = App::new().environment(env(&[(key, value)]));
        let error = result.err().unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains(key));
        if !value.is_empty() {
            assert!(!error.to_string().contains(value));
        }
    }
}
#[tokio::test]
async fn configuration_is_injected_and_docs_toggle_uses_the_same_snapshot() {
    let app = App::auto()
        .unwrap()
        .environment(env(&[("RUVORAQ_DOCS", "false"), ("CUSTOM_LIMIT", "7")]))
        .unwrap();
    app.check().unwrap();
    assert_eq!(app.env().get::<usize>("CUSTOM_LIMIT").unwrap(), 7);
    let router = app.into_router();
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&to_bytes(response.into_body(), 1024).await.unwrap())
            .unwrap(),
        json!({"limit":7})
    );
    assert_eq!(
        router
            .oneshot(Request::builder().uri("/docs").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let app = App::new()
        .environment(env(&[
            ("RUVORAQ_DOCS", "true"),
            ("RUVORAQ_APP_NAME", "Env docs"),
        ]))
        .unwrap();
    assert_eq!(app.openapi()["info"]["title"], "Env docs");
}
#[tokio::test]
async fn separate_app_configurations_are_isolated() {
    for expected in [3usize, 9] {
        let app = App::auto()
            .unwrap()
            .environment(Env::from_values([("CUSTOM_LIMIT", expected.to_string())]))
            .unwrap();
        let response = app
            .into_router()
            .oneshot(
                Request::builder()
                    .uri("/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
        assert_eq!(value["limit"], expected);
    }
}
