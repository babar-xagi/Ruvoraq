use ruvoraq::prelude::*;

use super::{
    models::{CreateStudent, PatchStudent, Student, StudentFilter},
    services::SchoolService,
};

type School = Inject<SchoolService>;

#[get("/school")]
async fn index() -> Value {
    json!({"name": "school", "message": "School API"})
}

#[get("/school/visits")]
async fn visits(service: School) -> Value {
    json!({"visits": service.visit()})
}

#[get("/school/status")]
#[post("/school/status")]
async fn status() -> Value {
    json!({"status": "ready"})
}

#[get("/school/students")]
async fn students(
    service: Inject<SchoolService>,
    Query(filter): Query<StudentFilter>,
) -> Result<Vec<Student>> {
    if !(1..=100).contains(&filter.limit) {
        return Err(invalid("limit", "Limit must be between 1 and 100"));
    }
    service.list(filter.limit, filter.min_id)
}

#[post("/school/students")]
async fn create_student(
    service: Inject<SchoolService>,
    ValidatedJson(input): ValidatedJson<CreateStudent>,
) -> Result<Reply<Student>> {
    Ok(created(service.create(input.name)?))
}

#[get("/school/students/{id}")]
async fn student(
    service: Inject<SchoolService>,
    Path(id): Path<u64>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<Student>)> {
    let student = service.find(id).await?;
    let mut response_headers = HeaderMap::new();
    if let Some(request_id) = headers.get("x-request-id") {
        response_headers.insert("x-request-id", request_id.clone());
    }
    Ok((response_headers, Json(student)))
}

#[put("/school/students/{id}")]
async fn replace_student(
    service: Inject<SchoolService>,
    Path(id): Path<u64>,
    ValidatedJson(input): ValidatedJson<CreateStudent>,
) -> Result<Student> {
    service.rename(id, input.name)
}

#[patch("/school/students/{id}")]
async fn update_student(
    service: Inject<SchoolService>,
    Path(id): Path<u64>,
    Json(input): Json<PatchStudent>,
) -> Result<Student> {
    let input = CreateStudent {
        name: input.name.ok_or_else(|| bad_request("Provide a name"))?,
    };
    input.validate()?;
    service.rename(id, input.name)
}

#[delete("/school/students/{id}")]
async fn delete_student(service: Inject<SchoolService>, Path(id): Path<u64>) -> Result<Response> {
    service.delete(id)?;
    Ok(no_content())
}

// Demonstrates the generic error response. This is an example endpoint.
#[get("/school/errors/internal")]
async fn internal_error() -> Result<Value> {
    Err(Error::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "demo_private_code",
        "demo private message",
    )
    .with_details(json!({"demo": "private details"})))
}
