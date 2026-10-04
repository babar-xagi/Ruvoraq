//! Optional SQLite persistence. SQLx remains available for parameterized queries.
pub use sqlx;
use sqlx::{
    Sqlite, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{fmt, io, str::FromStr, time::Duration};

/// A clonable shared pool. This initial SQLite adapter serializes access through
/// one connection, which also makes :memory: databases consistent across requests.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}
impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database")
            .field("connections", &self.pool.size())
            .finish_non_exhaustive()
    }
}
impl Database {
    /// Open SQLite and create a missing database file. Parent directories must exist.
    /// Connection errors never print the URL or underlying driver message.
    pub async fn connect(url: &str) -> io::Result<Self> {
        if !url.starts_with("sqlite:") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "DATABASE_URL must use the sqlite: scheme",
            ));
        }
        let options = SqliteConnectOptions::from_str(url)
            .map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid SQLite DATABASE_URL")
            })?
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool=SqlitePoolOptions::new().max_connections(1).acquire_timeout(Duration::from_secs(5))
            .connect_with(options).await
            .map_err(|_|io::Error::other("cannot open SQLite database; check DATABASE_URL, parent directory and file permissions"))?;
        Ok(Self { pool })
    }
    /// The SQLx executor for bind(), typed results and other driver capabilities.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
    /// Begin a transaction. Call commit() or rollback(); dropping also rolls back.
    pub async fn begin(&self) -> sqlx::Result<Transaction<'_, Sqlite>> {
        self.pool.begin().await
    }
    /// Close this shared pool and wait for outstanding connections.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}
