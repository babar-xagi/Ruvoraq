use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{Error, Json};

/// A JSON response with a named success status, created by helpers such as created().
#[derive(Debug)]
pub struct Reply<T> {
    status: StatusCode,
    value: T,
}

impl<T: Serialize> IntoResponse for Reply<T> {
    fn into_response(self) -> Response {
        let mut response = Json(self.value).into_response();
        // Serialization errors must remain 500 instead of being overwritten by 201/202.
        if !response.status().is_server_error() {
            *response.status_mut() = self.status;
        }
        response
    }
}

/// Return JSON with 200 OK. Also works with explicit App builder routes.
pub fn ok<T: Serialize>(value: T) -> Reply<T> {
    Reply {
        status: StatusCode::OK,
        value,
    }
}

/// Return JSON with 201 Created.
pub fn created<T: Serialize>(value: T) -> Reply<T> {
    Reply {
        status: StatusCode::CREATED,
        value,
    }
}

/// Return JSON with 202 Accepted.
pub fn accepted<T: Serialize>(value: T) -> Reply<T> {
    Reply {
        status: StatusCode::ACCEPTED,
        value,
    }
}

/// Return 204 No Content with an empty body.
pub fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Return a 400 error using the standard JSON envelope.
pub fn bad_request(message: impl Into<String>) -> Error {
    Error::bad_request(message)
}

/// Return a 404 error using the standard JSON envelope.
pub fn not_found(message: impl Into<String>) -> Error {
    Error::not_found(message)
}

/// Return a 422 error with a message and a single field in details.
pub fn invalid(field: impl Into<String>, message: impl Into<String>) -> Error {
    let message = message.into();
    let mut details = serde_json::Map::new();
    details.insert(field.into(), serde_json::Value::String(message.clone()));
    Error::validation(message).with_details(details.into())
}
