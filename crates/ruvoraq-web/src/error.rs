use std::{error, fmt};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::{Value, json};

/// The result type for HTTP handlers and application validation.
pub type Result<T> = std::result::Result<T, Error>;

/// An HTTP error with a stable JSON envelope.
///
/// Server error responses always redact message, code and details.
#[derive(Debug)]
pub struct Error {
    status: StatusCode,
    code: String,
    message: String,
    details: Value,
}

impl Error {
    /// Construct an error. Non-error HTTP statuses become 500.
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        let status = if status.is_client_error() || status.is_server_error() {
            status
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        Self {
            status,
            code: code.into(),
            message: message.into(),
            details: json!({}),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_error",
            message,
        )
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn method_not_allowed() -> Self {
        Self::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "Method not allowed",
        )
    }

    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Internal server error",
        )
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl error::Error for Error {}

#[derive(Serialize)]
struct Envelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: String,
    message: String,
    details: Value,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status;
        let body = if status.is_server_error() {
            ErrorBody {
                code: "internal_error".into(),
                message: "Internal server error".into(),
                details: json!({}),
            }
        } else {
            ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            }
        };
        (status, axum::Json(Envelope { error: body })).into_response()
    }
}
