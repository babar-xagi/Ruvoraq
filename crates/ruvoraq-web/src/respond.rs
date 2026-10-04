//! Compile-time response selection used only by route attributes.
//!
//! Generated code starts at &&&HandlerOutput and Rust's method lookup selects
//! the first applicable implementation. Explicit IntoResponse takes precedence;
//! Result<serializable model, response error> unwraps the successful model;
//! otherwise a serializable return value becomes JSON.
//!
//! The Cell owns a value only after the handler finishes. It allows the selected
//! reference receiver to consume it once without unsafe code or a heap allocation.

use std::cell::Cell;

use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::Json;

#[doc(hidden)]
pub struct HandlerOutput<T>(Cell<Option<T>>);

impl<T> HandlerOutput<T> {
    pub fn new(value: T) -> Self {
        Self(Cell::new(Some(value)))
    }

    fn take(&self) -> T {
        self.0
            .take()
            .expect("handler output converted exactly once")
    }
}

#[doc(hidden)]
pub trait Respond {
    fn respond(self) -> Response;
}

// Preserve text, explicit JSON, status tuples, responses and custom IntoResponse.
impl<T: IntoResponse> Respond for &&&HandlerOutput<T> {
    fn respond(self) -> Response {
        self.take().into_response()
    }
}

// Serialize only the successful value, never a Result's externally tagged enum.
impl<T: Serialize, E: IntoResponse> Respond for &&HandlerOutput<std::result::Result<T, E>> {
    fn respond(self) -> Response {
        match self.take() {
            Ok(value) => Json(value).into_response(),
            Err(error) => error.into_response(),
        }
    }
}

impl<T: Serialize> Respond for &HandlerOutput<T> {
    fn respond(self) -> Response {
        Json(self.take()).into_response()
    }
}
