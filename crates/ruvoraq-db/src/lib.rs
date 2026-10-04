//! Optional, explicit SQLite and PostgreSQL adapters.
mod migrations;
pub use migrations::{MigrationStatus, Migrations};
pub use sqlx;

#[cfg(feature = "sqlite")]
mod sqlite;
#[cfg(feature = "sqlite")]
pub use sqlite::Database;

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "postgres")]
pub use postgres::PostgresDatabase;
