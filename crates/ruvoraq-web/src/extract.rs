use std::ops::{Deref, DerefMut};

use axum::{
    extract::{FromRequest, FromRequestParts, Request, rejection::JsonRejection},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{Error, Result};

/// Typed JSON input/output, with consistent extractor and serialization errors.
///
/// A body extractor must be the last argument in a handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Error;

    async fn from_request(request: Request, state: &S) -> Result<Self> {
        axum::Json::<T>::from_request(request, state)
            .await
            .map(|value| Self(value.0))
            .map_err(json_error)
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        let response = axum::Json(self.0).into_response();
        if response.status().is_server_error() {
            Error::internal().into_response()
        } else {
            response
        }
    }
}

fn json_error(rejection: JsonRejection) -> Error {
    match rejection {
        JsonRejection::JsonSyntaxError(_) => Error::new(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Request body is not valid JSON",
        ),
        JsonRejection::JsonDataError(_) => {
            Error::validation("JSON body does not match the expected schema")
        }
        JsonRejection::MissingJsonContentType(_) => Error::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Expected Content-Type: application/json",
        ),
        JsonRejection::BytesRejection(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            Error::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                "Request body exceeds the size limit",
            )
        }
        JsonRejection::BytesRejection(error) => Error::new(
            error.status(),
            "invalid_body",
            "Could not read request body",
        ),
        _ => Error::bad_request("Could not read JSON body"),
    }
}

/// Extract typed values from named route parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|value| Self(value.0))
            .map_err(|error| Error::new(error.status(), "invalid_path", "Invalid path parameters"))
    }
}

/// Extract typed query parameters. Serde defaults can make fields optional.
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|value| Self(value.0))
            .map_err(|error| {
                Error::new(error.status(), "invalid_query", "Invalid query parameters")
            })
    }
}

/// Application-specific validation, invoked by ValidatedJson after deserialization.
pub trait Validate {
    fn validate(&self) -> Result<()>;
}

/// Typed JSON input that also runs the model's Validate implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate + Send,
{
    type Rejection = Error;

    async fn from_request(request: Request, state: &S) -> Result<Self> {
        let Json(value) = Json::<T>::from_request(request, state).await?;
        value.validate()?;
        Ok(Self(value))
    }
}

// Match the usual ergonomic access of tuple extractors without hiding their value.
macro_rules! deref {
    ($name:ident) => {
        impl<T> Deref for $name<T> {
            type Target = T;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl<T> DerefMut for $name<T> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}
deref!(Json);
deref!(Path);
deref!(Query);
deref!(ValidatedJson);
