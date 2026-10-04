use axum::{Router, response::Html, routing::get};
use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::{Value, json};
use std::marker::PhantomData;

pub struct SchemaProbe<T>(PhantomData<fn() -> T>);
impl<T> Default for SchemaProbe<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
pub trait DescribeSchema {
    fn describe_schema(self, output: bool) -> Value;
}
impl<T: JsonSchema> DescribeSchema for &&SchemaProbe<T> {
    fn describe_schema(self, output: bool) -> Value {
        let settings = SchemaSettings::draft2020_12();
        let settings = if output {
            settings.for_serialize()
        } else {
            settings.for_deserialize()
        };
        let mut value = serde_json::to_value(settings.into_generator().into_root_schema_for::<T>())
            .expect("schema serialization");
        value
            .as_object_mut()
            .expect("root schema")
            .remove("$schema");
        let mut root_name = "RuvoraqRoot".to_owned();
        while value
            .get("$defs")
            .and_then(|defs| defs.get(&root_name))
            .is_some()
        {
            root_name.push('_');
        }
        if rebase_root(&mut value, &root_name) {
            let mut root = value.clone();
            root.as_object_mut().unwrap().remove("$defs");
            let defs = value
                .as_object_mut()
                .unwrap()
                .entry("$defs")
                .or_insert_with(|| json!({}));
            defs.as_object_mut().unwrap().insert(root_name, root);
        }
        value
    }
}
impl<T> DescribeSchema for &SchemaProbe<T> {
    fn describe_schema(self, _: bool) -> Value {
        json!({"description": "Schema unavailable; add #[schema] to this model", "x-ruvoraq-schema-unavailable": true})
    }
}
fn rebase_root(value: &mut Value, name: &str) -> bool {
    let mut found = false;
    match value {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                if reference == "#" {
                    *reference = format!("#/$defs/{name}");
                    found = true;
                }
            }
            for child in map.values_mut() {
                found |= rebase_root(child, name);
            }
        }
        Value::Array(items) => {
            for child in items {
                found |= rebase_root(child, name);
            }
        }
        _ => {}
    }
    found
}
fn collect_definitions(value: &mut Value, components: &mut serde_json::Map<String, Value>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(definitions)) = map.remove("$defs") {
                let offset = components.len();
                let names: std::collections::BTreeMap<_, _> = definitions
                    .keys()
                    .enumerate()
                    .map(|(i, name)| {
                        (
                            name.clone(),
                            format!(
                                "{}_{}",
                                name.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "_"),
                                offset + i
                            ),
                        )
                    })
                    .collect();
                for child in map.values_mut() {
                    rewrite_refs(child, &names);
                }
                for (name, mut schema) in definitions {
                    rewrite_refs(&mut schema, &names);
                    components.insert(names[&name].clone(), schema);
                }
            }
            for child in map.values_mut() {
                collect_definitions(child, components);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_definitions(child, components);
            }
        }
        _ => {}
    }
}
fn rewrite_refs(value: &mut Value, names: &std::collections::BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/$defs/") {
                    let name = name.replace("~1", "/").replace("~0", "~");
                    if let Some(mapped) = names.get(&name) {
                        *reference = format!("#/components/schemas/{mapped}");
                    }
                }
            }
            for child in map.values_mut() {
                rewrite_refs(child, names);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_refs(child, names);
            }
        }
        _ => {}
    }
}

pub fn parameters(kind: &str, schema: Value, path: &str) -> Vec<Value> {
    let names: Vec<_> = path
        .split('/')
        .filter_map(|s| s.strip_prefix('{').and_then(|s| s.strip_suffix('}')))
        .collect();
    if kind == "path" {
        return names.iter().enumerate().map(|(i, name)| {
            let field = schema.get("properties").and_then(|p| p.get(*name)).cloned()
                .or_else(|| schema.get("prefixItems").and_then(|p| p.get(i)).cloned())
                .unwrap_or_else(|| if names.len() == 1 {schema.clone()} else {json!({})});
            // Definitions must be available to references in extracted properties.
            json!({"name":name, "in":"path", "required":true, "schema":with_defs(field, &schema)})
        }).collect();
    }
    let required = schema.get("required").and_then(Value::as_array);
    schema.get("properties").and_then(Value::as_object).map(|fields| fields.iter().map(|(name, field)| {
        json!({"name":name,"in":"query","required":required.is_some_and(|r| r.contains(&json!(name))),"schema":with_defs(field.clone(), &schema)})
    }).collect()).unwrap_or_default()
}
fn with_defs(mut field: Value, root: &Value) -> Value {
    if let (Some(map), Some(defs)) = (field.as_object_mut(), root.get("$defs")) {
        map.insert("$defs".into(), defs.clone());
    }
    field
}
pub fn operation(
    id: &str,
    summary: &str,
    tag: &str,
    parameters: Vec<Value>,
    body: Option<Value>,
    output: Value,
    response: (&str, &str),
) -> Value {
    let (status, media) = response;
    let tag = if tag.ends_with("::__ruvoraq_routes") {
        "default"
    } else {
        tag.rsplit("::")
            .find(|part| *part != "routes")
            .unwrap_or("default")
    };
    let mut responses = serde_json::Map::new();
    let response = if status == "204" {
        json!({"description":"No Content"})
    } else if media.is_empty() {
        json!({"description":"Success; response depends on handler"})
    } else {
        json!({"description":"Successful response","content":{media:{"schema":output}}})
    };
    responses.insert(status.into(), response);
    for (code, description) in [
        ("400", "Invalid path, query or JSON syntax"),
        ("404", "Not found"),
        ("422", "Validation error"),
        ("500", "Internal server error"),
    ] {
        responses.insert(code.into(), json!({"description":description,"content":{"application/json":{"schema":error_schema()}}}));
    }
    if body.is_some() {
        for (code, description) in [
            ("413", "Body exceeds 2 MiB"),
            ("415", "Expected application/json"),
        ] {
            responses.insert(code.into(),json!({"description":description,"content":{"application/json":{"schema":error_schema()}}}));
        }
    }
    let mut result = json!({"operationId":id,"summary":summary,"tags":[tag],"parameters":parameters,"responses":responses});
    if let Some(schema) = body {
        result["requestBody"] =
            json!({"required":true,"content":{"application/json":{"schema":schema}}});
    }
    result
}
fn error_schema() -> Value {
    json!({"type":"object","required":["error"],"properties":{"error":{"type":"object","required":["code","message","details"],"properties":{"code":{"type":"string"},"message":{"type":"string"},"details":{"example":{}}}}}})
}
pub fn document(title: &str, routes: &[(&str, &str, Value)]) -> Value {
    let mut paths = serde_json::Map::new();
    for (path, method, operation) in routes {
        let item = paths.entry((*path).to_owned()).or_insert_with(|| json!({}));
        item[method.to_ascii_lowercase()] = operation.clone();
    }
    let mut document = json!({"openapi":"3.1.0","info":{"title":title,"version":env!("CARGO_PKG_VERSION")},"paths":paths});
    let mut components = serde_json::Map::new();
    collect_definitions(&mut document, &mut components);
    document["components"] = json!({"schemas":components});
    document
}
pub fn reserved(path: &str) -> bool {
    matches!(
        path,
        "/docs" | "/openapi.json" | "/docs/swagger-ui.css" | "/docs/swagger-ui-bundle.js"
    )
}
pub fn mount(router: Router, document: Value) -> Router {
    router
        .route(
            "/openapi.json",
            get(move || {
                let value = document.clone();
                async move { axum::Json(value) }
            }),
        )
        .route(
            "/docs",
            get(|| async { Html(include_str!("swagger/index.html")) }),
        )
        .route(
            "/docs/swagger-ui.css",
            get(|| async {
                (
                    [("content-type", "text/css; charset=utf-8")],
                    include_str!("swagger/swagger-ui.css"),
                )
            }),
        )
        .route(
            "/docs/swagger-ui-bundle.js",
            get(|| async {
                (
                    [("content-type", "application/javascript; charset=utf-8")],
                    include_str!("swagger/swagger-ui-bundle.js"),
                )
            }),
        )
}
