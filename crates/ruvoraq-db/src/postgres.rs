//! Explicit PostgreSQL adapter; SQLite's Database type stays unchanged.
use crate::{MigrationStatus, Migrations, sqlx};
use sqlx::{
    PgPool, Postgres, Transaction,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{fmt, io, path::Path, str::FromStr, time::Duration};

/// A shared PostgreSQL pool with up to five connections.
#[derive(Clone)]
pub struct PostgresDatabase {
    pool: PgPool,
}

impl fmt::Debug for PostgresDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresDatabase")
            .field("connections", &self.pool.size())
            .finish_non_exhaustive()
    }
}

impl PostgresDatabase {
    /// Connect to an existing database. URLs and underlying errors are redacted.
    /// TLS is supported through SQLx URL options; production TLS policy is explicit.
    pub async fn connect(url: &str) -> io::Result<Self> {
        if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "DATABASE_URL must use postgres:// or postgresql://",
            ));
        }
        let options = PgConnectOptions::from_str(url)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid PostgreSQL DATABASE_URL",
                )
            })?
            .options([("lock_timeout", "5s")]);
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
            .map_err(|_| io::Error::other("cannot connect to PostgreSQL; check DATABASE_URL, database availability and access"))?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn begin(&self) -> sqlx::Result<Transaction<'_, Postgres>> {
        self.pool.begin().await
    }

    /// Validate and apply forward-only migrations under a PostgreSQL advisory lock.
    pub async fn migrate(&self, directory: impl AsRef<Path>) -> io::Result<usize> {
        Migrations::load(directory).await?.run_postgres(self).await
    }

    /// Validate and inspect history without executing pending migration SQL.
    pub async fn migration_status(
        &self,
        directory: impl AsRef<Path>,
    ) -> io::Result<Vec<MigrationStatus>> {
        Migrations::load(directory)
            .await?
            .status_postgres(self)
            .await
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}
