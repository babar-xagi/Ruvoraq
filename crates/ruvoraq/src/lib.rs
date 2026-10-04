//! Ruvoraq's public entry point.
//!
//! A generated application's main.rs contains only handlers:
//!
//! ```no_run
//! use ruvoraq::prelude::*;
//!
//! #[get("/")]
//! async fn hello() -> &'static str {
//!     "Hello"
//! }
//! ```
//!
//! settings.rs holds configuration and invokes bootstrap! to generate startup.
//! Cargo uses settings.rs as the binary entry point.

use std::{future::Future, io};

pub use ruvoraq_config::Env;

pub use ruvoraq_macros::{bootstrap, delete, get, patch, post, put, schema};
pub use ruvoraq_web::schemars;
pub use ruvoraq_web::{
    App, Error, HeaderMap, Inject, IntoResponse, Json, Path, Query, Reply, Response, Result,
    Settings, StatusCode, Validate, ValidatedJson, accepted, bad_request, created, invalid,
    no_content, not_found, ok,
};
pub use serde;
pub use serde::{Deserialize, Serialize};
pub use serde_json::{Value, json};

/// Start the Tokio runtime and run an application's async entry point.
///
/// Call this from synchronous `main`, outside an existing async runtime.
pub fn run<F>(future: F) -> io::Result<()>
where
    F: Future<Output = io::Result<()>>,
{
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(future)
}

/// Common application imports.
pub mod prelude {
    pub use crate::{
        App, Deserialize, Env, Error, HeaderMap, Inject, IntoResponse, Json, Path, Query, Reply,
        Response, Result, Serialize, Settings, StatusCode, Validate, ValidatedJson, Value,
        accepted, bad_request, created, delete, get, invalid, json, no_content, not_found, ok,
        patch, post, put, run, schema,
    };
}

/// Implementation details used by generated code, not a stable application API.
#[doc(hidden)]
pub mod __private {
    /// Normalize infallible and fallible configuration hooks.
    pub trait ConfiguredApp {
        fn configured(self) -> ::std::io::Result<super::App>;
    }
    impl ConfiguredApp for super::App {
        fn configured(self) -> ::std::io::Result<super::App> {
            Ok(self)
        }
    }
    impl ConfiguredApp for ::std::io::Result<super::App> {
        fn configured(self) -> ::std::io::Result<super::App> {
            self
        }
    }

    pub use ruvoraq_web::{
        Dependency, DescribeSchema, HandlerOutput, RequiredService, Respond, RouteRegistration,
        SchemaProbe, ServiceProbe, inventory, operation, parameters,
    };
}
