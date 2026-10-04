use axum::{
    body::{Body, to_bytes},
    http::{Request, header},
};
use ruvoraq::prelude::*;
use tower::ServiceExt;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    name: String,
}

impl Validate for Input {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::validation("Name must not be empty")
                .with_details(json!({"name": "must not be blank"})));
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

#[post("/echo")]
async fn echo(Json(input): Json<Input>) -> Json<User> {
    Json(User {
        id: 1,
        name: input.name,
    })
}

#[post("/users")]
async fn create(ValidatedJson(input): ValidatedJson<Input>) -> Result<(StatusCode, Json<User>)> {
    Ok((
        StatusCode::CREATED,
        Json(User {
            id: 1,
            name: input.name,
        }),
    ))
}

#[get("/users/{id}")]
#[post("/users/{id}")]
async fn read(Path(id): Path<u64>) -> Json<Value> {
    Json(json!({"id": id}))
}

#[derive(Deserialize)]
struct Ids {
    id: u64,
    item_id: u16,
}

#[get("/users/{id}/items/{item_id}")]
async fn item(Path(ids): Path<Ids>) -> Json<Value> {
    Json(json!({"user": ids.id, "item": ids.item_id}))
}

#[derive(Deserialize)]
struct Search {
    #[serde(default = "default_limit")]
    limit: u16,
    q: Option<String>,
}

fn default_limit() -> u16 {
    10
}

#[get("/search")]
async fn search(Query(query): Query<Search>) -> Json<Value> {
    Json(json!({"limit": query.limit, "q": query.q}))
}

#[get("/headers")]
async fn headers(headers: HeaderMap) -> Json<Value> {
    Json(json!({"request_id": headers.get("x-request-id").and_then(|value| value.to_str().ok())}))
}

#[get("/returned-error")]
async fn returned_error() -> Result<Json<Value>> {
    Err(Error::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "secret_code",
        "secret password",
    )
    .with_details(json!({"secret": "sensitive"})))
}

#[get("/panic")]
async fn panicking() -> &'static str {
    panic!("sensitive panic payload");
}

struct Broken;

impl Serialize for Broken {
    fn serialize<S>(&self, _: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("sensitive serialization failure"))
    }
}

#[get("/serialization-error")]
async fn serialization_error() -> Json<Broken> {
    Json(Broken)
}

#[get("/simple/model")]
async fn simple_model() -> User {
    User {
        id: 7,
        name: "Ada".into(),
    }
}

#[get("/simple/json")]
async fn simple_json() -> Value {
    json!({"Hello":"World"})
}

#[post("/simple/created")]
async fn simple_created(Json(input): Json<Input>) -> Reply<User> {
    created(User {
        id: 8,
        name: input.name,
    })
}

#[post("/simple/accepted")]
async fn simple_accepted() -> impl IntoResponse {
    accepted(json!({"queued":true}))
}

#[get("/simple/ok")]
async fn simple_ok() -> impl IntoResponse {
    ok(json!({"ready":true}))
}

#[delete("/simple/empty")]
async fn simple_empty() -> impl IntoResponse {
    no_content()
}

#[get("/simple/result/{id}")]
async fn simple_result(Path(id): Path<u64>) -> Result<User> {
    if id == 0 {
        return Err(not_found("User not found"));
    }
    Ok(User {
        id,
        name: "Ada".into(),
    })
}

#[get("/simple/list")]
async fn simple_list() -> Result<Vec<User>> {
    Ok(vec![User {
        id: 1,
        name: "Ada".into(),
    }])
}

#[post("/simple/result-created")]
async fn simple_result_created() -> Result<Reply<User>> {
    Ok(created(User {
        id: 9,
        name: "Grace".into(),
    }))
}

#[get("/simple/bad")]
async fn simple_bad() -> Result<User> {
    Err(bad_request("Bad input"))
}

#[get("/simple/invalid")]
async fn simple_invalid() -> Result<User> {
    Err(invalid("name", "must not be blank"))
}

#[get("/simple/text")]
async fn simple_text() -> Result<String> {
    Ok("still text".into())
}

#[derive(Serialize)]
struct CustomResponse;

impl IntoResponse for CustomResponse {
    fn into_response(self) -> Response {
        (StatusCode::PARTIAL_CONTENT, "custom response").into_response()
    }
}

#[get("/simple/custom")]
async fn simple_custom() -> CustomResponse {
    CustomResponse
}

#[get("/simple/serialization-error")]
async fn simple_serialization_error() -> Broken {
    Broken
}

#[get("/simple/result-serialization-error")]
async fn simple_result_serialization_error() -> Result<Broken> {
    Ok(Broken)
}

#[post("/simple/created-serialization-error")]
async fn simple_created_serialization_error() -> Reply<Broken> {
    created(Broken)
}

async fn raw_call(
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: impl Into<Body>,
) -> Response {
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    App::auto()
        .unwrap()
        .into_router()
        .oneshot(request.body(body.into()).unwrap())
        .await
        .unwrap()
}

async fn call(
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: impl Into<Body>,
) -> (StatusCode, HeaderMap, Value) {
    let response = raw_call(method, uri, headers, body).await;
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    (status, headers, json)
}

fn assert_error(response: &(StatusCode, HeaderMap, Value), status: StatusCode, code: &str) {
    assert_eq!(response.0, status);
    let error = &response.2["error"];
    assert_eq!(error["code"], code);
    assert!(error["message"].is_string());
    assert!(error["details"].is_object());
    assert_eq!(response.2.as_object().unwrap().len(), 1);
    assert_eq!(error.as_object().unwrap().len(), 3);
}

#[tokio::test]
async fn typed_json_and_created_status() {
    let response = call(
        "POST",
        "/echo",
        &[("content-type", "application/json")],
        r#"{"name":"Ada"}"#,
    )
    .await;
    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(response.2, json!({"id": 1, "name": "Ada"}));
    let response = call(
        "POST",
        "/users",
        &[("content-type", "application/json")],
        r#"{"name":"Grace"}"#,
    )
    .await;
    assert_eq!(response.0, StatusCode::CREATED);
    assert_eq!(response.2, json!({"id": 1, "name": "Grace"}));
    let response = call(
        "POST",
        "/echo",
        &[("content-type", "application/vnd.api+json")],
        r#"{"name":"Ada"}"#,
    )
    .await;
    assert_eq!(response.0, StatusCode::OK);
}

#[tokio::test]
async fn named_paths_queries_defaults_and_headers() {
    let response = call("GET", "/users/42", &[], "").await;
    assert_eq!(response.2, json!({"id":42}));
    let response = call("POST", "/users/42", &[], "").await;
    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(response.2, json!({"id":42}));
    let response = call("GET", "/users/42/items/7", &[], "").await;
    assert_eq!(response.2, json!({"user":42,"item":7}));
    let response = call("GET", "/search?limit=3&q=hello%20world", &[], "").await;
    assert_eq!(response.2, json!({"limit":3,"q":"hello world"}));
    let response = call("GET", "/search", &[], "").await;
    assert_eq!(response.2, json!({"limit":10,"q":null}));
    let response = call("GET", "/headers", &[("x-request-id", "request-123")], "").await;
    assert_eq!(response.2, json!({"request_id":"request-123"}));
}

#[tokio::test]
async fn syntax_schema_content_type_and_validation_errors() {
    for body in ["{", "", r#"{"name":"Ada"} trailing"#] {
        let response = call(
            "POST",
            "/echo",
            &[("content-type", "application/json")],
            body,
        )
        .await;
        assert_error(&response, StatusCode::BAD_REQUEST, "invalid_json");
    }
    for body in [r#"{"name":42}"#, "{}", r#"{"name":"Ada","extra":true}"#] {
        let response = call(
            "POST",
            "/echo",
            &[("content-type", "application/json")],
            body,
        )
        .await;
        assert_error(
            &response,
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_error",
        );
    }
    for headers in [&[][..], &[("content-type", "text/plain")][..]] {
        let response = call("POST", "/echo", headers, r#"{"name":"Ada"}"#).await;
        assert_error(
            &response,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        );
    }
    let response = call(
        "POST",
        "/users",
        &[("content-type", "application/json")],
        r#"{"name":"  "}"#,
    )
    .await;
    assert_error(
        &response,
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_error",
    );
    assert_eq!(
        response.2["error"]["details"],
        json!({"name":"must not be blank"})
    );
    let response = call("GET", "/users/not-a-number", &[], "").await;
    assert_error(&response, StatusCode::BAD_REQUEST, "invalid_path");
    let response = call("GET", "/search?limit=wrong", &[], "").await;
    assert_error(&response, StatusCode::BAD_REQUEST, "invalid_query");
}

#[tokio::test]
async fn missing_routes_and_unsupported_methods_have_json_errors_and_allow_header() {
    let response = call("GET", "/missing", &[], "").await;
    assert_error(&response, StatusCode::NOT_FOUND, "not_found");
    let response = call("GET", "/echo", &[], "").await;
    assert_error(
        &response,
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
    );
    assert!(response.1[header::ALLOW].to_str().unwrap().contains("POST"));
}

#[tokio::test]
async fn body_limit_rejections_are_consistent() {
    let response = call(
        "POST",
        "/echo",
        &[("content-type", "application/json")],
        vec![b'x'; 2 * 1024 * 1024 + 1],
    )
    .await;
    assert_error(
        &response,
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
    );
}

#[tokio::test]
async fn server_errors_panics_and_serialization_failures_hide_details() {
    for (method, path) in [
        ("GET", "/returned-error"),
        ("GET", "/panic"),
        ("GET", "/serialization-error"),
        ("GET", "/simple/serialization-error"),
        ("GET", "/simple/result-serialization-error"),
        ("POST", "/simple/created-serialization-error"),
    ] {
        let response = call(method, path, &[], "").await;
        assert_error(
            &response,
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
        );
        assert_eq!(
            response.2,
            json!({"error":{
                "code":"internal_error","message":"Internal server error","details":{}
            }})
        );
        assert!(!response.2.to_string().contains("sensitive"));
        assert!(!response.2.to_string().contains("secret"));
    }
}

#[tokio::test]
async fn typed_requests_and_errors_are_served_over_http() {
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::oneshot,
        time::timeout,
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown, receive) = oneshot::channel();
    let server = tokio::spawn(App::auto().unwrap().serve(listener, async {
        receive.await.ok();
    }));

    for (body, expected_status, expected_body) in [
        (
            r#"{"name":"Ada"}"#,
            "201 Created",
            json!({"id":1,"name":"Ada"}),
        ),
        (
            r#"{"name":" "}"#,
            "422 Unprocessable Entity",
            json!({"error":{
                "code":"validation_error",
                "message":"Name must not be empty",
                "details":{"name":"must not be blank"}
            }}),
        ),
    ] {
        let response = timeout(Duration::from_secs(5), async {
            let mut stream = TcpStream::connect(address).await.unwrap();
            let request = format!(
                "POST /users HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(request.as_bytes()).await.unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).await.unwrap();
            response
        })
        .await
        .expect("HTTP response timed out");
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        assert!(
            headers.starts_with(&format!("HTTP/1.1 {expected_status}")),
            "{response}"
        );
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("content-type: application/json")
        );
        assert_eq!(serde_json::from_str::<Value>(body).unwrap(), expected_body);
    }
    shutdown.send(()).unwrap();
    timeout(Duration::from_secs(5), server)
        .await
        .expect("server shutdown timed out")
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn simple_model_responses_named_statuses_and_error_helpers() {
    assert_eq!(simple_model().await.name, "Ada");
    assert_eq!(simple_result(Path(4)).await.unwrap().id, 4);
    for (method, uri, status, body) in [
        (
            "GET",
            "/simple/model",
            StatusCode::OK,
            json!({"id":7,"name":"Ada"}),
        ),
        (
            "GET",
            "/simple/json",
            StatusCode::OK,
            json!({"Hello":"World"}),
        ),
        (
            "GET",
            "/simple/result/42",
            StatusCode::OK,
            json!({"id":42,"name":"Ada"}),
        ),
        (
            "GET",
            "/simple/list",
            StatusCode::OK,
            json!([{"id":1,"name":"Ada"}]),
        ),
        (
            "POST",
            "/simple/accepted",
            StatusCode::ACCEPTED,
            json!({"queued":true}),
        ),
        ("GET", "/simple/ok", StatusCode::OK, json!({"ready":true})),
        (
            "POST",
            "/simple/result-created",
            StatusCode::CREATED,
            json!({"id":9,"name":"Grace"}),
        ),
    ] {
        let response = call(method, uri, &[], "").await;
        assert_eq!(response.0, status);
        assert_eq!(response.2, body);
    }
    let response = call(
        "POST",
        "/simple/created",
        &[("content-type", "application/json")],
        r#"{"name":"Ada"}"#,
    )
    .await;
    assert_eq!(response.0, StatusCode::CREATED);
    assert_eq!(response.2, json!({"id":8,"name":"Ada"}));
    for (path, status, code, message, details) in [
        (
            "/simple/result/0",
            StatusCode::NOT_FOUND,
            "not_found",
            "User not found",
            json!({}),
        ),
        (
            "/simple/bad",
            StatusCode::BAD_REQUEST,
            "bad_request",
            "Bad input",
            json!({}),
        ),
        (
            "/simple/invalid",
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_error",
            "must not be blank",
            json!({"name":"must not be blank"}),
        ),
    ] {
        let response = call("GET", path, &[], "").await;
        assert_error(&response, status, code);
        assert_eq!(response.2["error"]["message"], message);
        assert_eq!(response.2["error"]["details"], details);
    }
}

#[tokio::test]
async fn empty_text_and_custom_responses_keep_native_behavior() {
    let response = raw_call("DELETE", "/simple/empty", &[], "").await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty()
    );
    for (path, status, expected) in [
        ("/simple/text", StatusCode::OK, "still text"),
        (
            "/simple/custom",
            StatusCode::PARTIAL_CONTENT,
            "custom response",
        ),
    ] {
        let response = raw_call("GET", path, &[], "").await;
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            expected
        );
    }
}
