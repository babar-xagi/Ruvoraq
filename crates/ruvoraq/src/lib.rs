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

pub use ruvoraq_macros::{bootstrap, delete, get, patch, post, put};
pub use ruvoraq_web::{App, Settings};

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
    pub use crate::{App, Settings, delete, get, patch, post, put, run};
}

/// Implementation details used by generated code, not a stable application API.
#[doc(hidden)]
pub mod __private {
    pub use ruvoraq_web::{RouteRegistration, inventory};
}
