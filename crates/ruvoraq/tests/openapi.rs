use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use ruvoraq::prelude::*;
use tower::ServiceExt;

#[schema]
#[derive(Serialize, Deserialize)]
struct Address {
    city: String,
}

#[schema]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    #[serde(rename = "displayName")]
    name: String,
    address: Address,
    note: Option<String>,
}
impl Validate for Input {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[schema]
#[derive(Serialize, Deserialize)]
struct Filter {
    term: String,
    #[serde(default = "default_limit")]
    limit: u16,
    tag: Option<String>,
}
fn default_limit() -> u16 {
    10
}

#[schema]
#[derive(Serialize)]
struct Recursive {
    child: Option<Box<Recursive>>,
}

/// Create a documented item.
#[post("/items", status = 201)]
async fn create(ValidatedJson(input): ValidatedJson<Input>) -> Result<Reply<Input>> {
    Ok(created(input))
}
#[get("/items/{id}")]
#[post("/items/{id}")]
async fn read(Path(id): Path<u64>, Query(filter): Query<Filter>) -> Value {
    json!({"id":id,"term":filter.term})
}
#[delete("/items/{id}", status = 204)]
async fn remove(Path(_id): Path<u64>) -> Response {
    no_content()
}
#[get("/")]
async fn hello() -> &'static str {
    "Hello"
}
#[get("/tree")]
async fn tree() -> Recursive {
    Recursive { child: None }
}
#[get("/opaque", status = 202)]
async fn opaque() -> impl IntoResponse {
    accepted(json!({"ready":true}))
}
#[get("/disabled")]
#[cfg(any())]
async fn disabled() -> &'static str {
    "disabled"
}
#[derive(Serialize)]
struct Legacy {
    name: String,
}
#[get("/json-string")]
async fn json_string() -> Json<String> {
    Json("hello".to_owned())
}
#[get("/binary")]
async fn binary() -> Vec<u8> {
    vec![1, 2, 3]
}
#[get("/status")]
async fn native_status() -> StatusCode {
    StatusCode::ACCEPTED
}

#[get("/legacy")]
async fn legacy() -> Legacy {
    Legacy {
        name: "legacy".into(),
    }
}

fn app() -> App {
    App::auto().unwrap().settings(Settings {
        app_name: "Docs <example>".into(),
        ..Settings::default()
    })
}
async fn get(app: App, path: &str) -> (StatusCode, String, Vec<u8>) {
    let response = app
        .into_router()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content = response.headers()["content-type"]
        .to_str()
        .unwrap()
        .to_owned();
    (
        status,
        content,
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}
fn validate_refs(root: &Value, value: &Value) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get("$ref") {
                assert!(
                    reference.starts_with("#/components/schemas/"),
                    "{reference}"
                );
                assert!(
                    root.pointer(&reference[1..]).is_some(),
                    "unresolved {reference}"
                );
            }
            for child in map.values() {
                validate_refs(root, child);
            }
        }
        Value::Array(items) => {
            for child in items {
                validate_refs(root, child);
            }
        }
        _ => {}
    }
}
#[test]
fn automatic_metadata_schemas_statuses_parameters_and_references() {
    let document = app().openapi();
    assert_eq!(document["openapi"], "3.1.0");
    assert_eq!(document["info"]["title"], "Docs <example>");
    let paths = &document["paths"];
    assert!(paths.get("/disabled").is_none());
    assert!(paths.get("/docs").is_none());
    assert!(paths["/items/{id}"]["get"].is_object());
    assert!(paths["/items/{id}"]["post"].is_object());
    assert_ne!(
        paths["/items/{id}"]["get"]["operationId"],
        paths["/items/{id}"]["post"]["operationId"]
    );
    let create = &paths["/items"]["post"];
    assert_eq!(create["summary"], "Create a documented item.");
    assert!(create["responses"]["201"].is_object());
    for code in ["400", "404", "413", "415", "422", "500"] {
        assert!(create["responses"][code].is_object());
    }
    let body = &create["requestBody"]["content"]["application/json"]["schema"];
    assert_eq!(body["properties"]["displayName"]["type"], "string");
    assert_eq!(body["additionalProperties"], false);
    assert!(
        body["required"]
            .as_array()
            .unwrap()
            .contains(&json!("displayName"))
    );
    let params = paths["/items/{id}"]["get"]["parameters"]
        .as_array()
        .unwrap();
    let param = |name: &str| params.iter().find(|p| p["name"] == name).unwrap();
    assert_eq!(param("id")["in"], "path");
    assert_eq!(param("id")["required"], true);
    assert_eq!(param("id")["schema"]["type"], "integer");
    assert_eq!(param("term")["required"], true);
    assert_eq!(param("limit")["required"], false);
    assert_eq!(param("limit")["schema"]["default"], 10);
    assert_eq!(param("tag")["required"], false);
    assert!(
        paths["/items/{id}"]["delete"]["responses"]["204"]
            .get("content")
            .is_none()
    );
    assert_eq!(
        paths["/"]["get"]["responses"]["200"]["content"]["text/plain"]["schema"]["type"],
        "string"
    );
    assert!(
        paths["/opaque"]["get"]["responses"]["202"]
            .get("content")
            .is_none()
    );
    assert_eq!(
        paths["/legacy"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]["x-ruvoraq-schema-unavailable"],
        true
    );
    assert_eq!(
        paths["/json-string"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]["type"],
        "string"
    );
    assert_eq!(
        paths["/binary"]["get"]["responses"]["200"]["content"]["application/octet-stream"]["schema"]
            ["format"],
        "binary"
    );
    assert!(
        paths["/status"]["get"]["responses"]["default"]
            .get("content")
            .is_none()
    );
    validate_refs(&document, &document);
    assert!(
        !document["components"]["schemas"]
            .as_object()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn docs_and_all_assets_work_offline_and_schema_matches_public_api() {
    let (status, content, body) = get(app(), "/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content, "application/json");
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        app().openapi()
    );
    let (status, content, body) = get(app(), "/docs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(content.starts_with("text/html"));
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("SwaggerUIBundle"));
    assert!(html.contains("validatorUrl: null"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("Docs <example>"));
    for (path, media) in [
        ("/docs/swagger-ui.css", "text/css"),
        ("/docs/swagger-ui-bundle.js", "application/javascript"),
    ] {
        let (status, content, body) = get(app(), path).await;
        assert_eq!(status, StatusCode::OK);
        assert!(content.starts_with(media));
        assert!(body.len() > 10000);
    }
}
#[tokio::test]
async fn docs_can_be_disabled_and_conflicts_have_actionable_errors() {
    for path in [
        "/docs",
        "/openapi.json",
        "/docs/swagger-ui.css",
        "/docs/swagger-ui-bundle.js",
    ] {
        let (status, _, _) = get(app().docs(false), path).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let conflict = app().get("/docs", || async { "custom" });
    assert!(
        conflict
            .check()
            .unwrap_err()
            .to_string()
            .contains("docs(false)")
    );
    let (status, _, body) = get(conflict.docs(false), "/docs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"custom");
}
