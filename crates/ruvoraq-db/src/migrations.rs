//! Forward-only database migrations with validation before applying pending files.
#[cfg(feature = "sqlite")]
use crate::Database;
#[cfg(feature = "postgres")]
use crate::PostgresDatabase;
use crate::sqlx;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
use sqlx::migrate::MigrateError;
use sqlx::migrate::Migrator;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path,
};

/// A migration's source version, description, and database state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStatus {
    pub version: i64,
    pub description: String,
    pub applied: bool,
}

/// SQL migrations loaded from a directory. Files are read when loading, not when running.
#[derive(Debug)]
pub struct Migrations {
    migrator: Migrator,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl Migrations {
    /// Load forward-only files named `<positive-version>_<lowercase_description>.sql`.
    /// Non-SQL files are ignored. Duplicate versions and malformed SQL filenames fail.
    pub async fn load(directory: impl AsRef<Path>) -> io::Result<Self> {
        let directory = directory.as_ref();
        let entries = fs::read_dir(directory).map_err(|_| {
            invalid("cannot read migrations directory; create migrations/ with numbered SQL files")
        })?;
        let mut versions = BTreeSet::new();
        for entry in entries {
            let entry = entry.map_err(|_| invalid("cannot read migration directory entry"))?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| invalid("migration filenames must be valid UTF-8"))?;
            if !name.ends_with(".sql") {
                continue;
            }
            if !entry
                .file_type()
                .map_err(|_| invalid("cannot inspect migration file"))?
                .is_file()
            {
                return Err(invalid(
                    "SQL migration entries must be regular files, not directories or symlinks",
                ));
            }
            let stem = name.strip_suffix(".sql").unwrap();
            let (number, description) = stem.split_once('_').ok_or_else(|| {
                invalid("migration filenames must use <version>_<description>.sql")
            })?;
            let version: i64 = number
                .parse()
                .map_err(|_| invalid("migration version must be a positive integer"))?;
            if version <= 0 || !number.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid("migration version must be a positive integer"));
            }
            if description.is_empty()
                || !description
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            {
                return Err(invalid(
                    "migration descriptions must use lowercase ASCII letters, digits or underscores; reversible files are not supported",
                ));
            }
            if !versions.insert(version) {
                return Err(invalid(format!("duplicate migration version {version}")));
            }
        }
        if versions.is_empty() {
            return Err(invalid(
                "migrations directory contains no numbered SQL files",
            ));
        }
        let migrator = Migrator::new(directory).await.map_err(|_| {
            invalid("cannot load SQL migrations; check file encoding and permissions")
        })?;
        for migration in migrator.iter() {
            if migration.no_tx {
                return Err(invalid("non-transactional migrations are not supported"));
            }
            if migration.sql.trim().is_empty() {
                return Err(invalid(format!("migration {} is empty", migration.version)));
            }
        }
        Ok(Self { migrator })
    }

    /// Validate recorded history and report applied/pending files without applying SQL.
    #[cfg(feature = "sqlite")]
    pub async fn status(&self, database: &Database) -> io::Result<Vec<MigrationStatus>> {
        let exists: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_one(database.pool())
        .await
        .map_err(|_| invalid("cannot inspect migration history"))?;
        let mut applied = BTreeMap::new();
        if exists.0 != 0 {
            let rows: Vec<(i64, bool, Vec<u8>)> = sqlx::query_as(
                "SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version",
            )
            .fetch_all(database.pool())
            .await
            .map_err(|_| invalid("cannot read migration history"))?;
            for (version, success, checksum) in rows {
                if !success {
                    return Err(invalid(format!(
                        "migration {version} is incomplete; inspect database history before continuing"
                    )));
                }
                applied.insert(version, checksum);
            }
        }
        self.validated_status(applied)
    }

    fn validated_status(
        &self,
        applied: BTreeMap<i64, Vec<u8>>,
    ) -> io::Result<Vec<MigrationStatus>> {
        // Validate all history before applying anything, including lower-numbered additions.
        for (version, checksum) in &applied {
            let source = self
                .migrator
                .iter()
                .find(|m| m.version == *version)
                .ok_or_else(|| {
                    invalid(format!(
                        "applied migration {version} is missing; restore its original file"
                    ))
                })?;
            if source.checksum.as_ref() != checksum.as_slice() {
                return Err(invalid(format!(
                    "applied migration {version} has changed; restore its original contents"
                )));
            }
        }
        let latest = applied.keys().next_back().copied().unwrap_or(0);
        let mut result = Vec::new();
        for migration in self.migrator.iter() {
            let present = applied.contains_key(&migration.version);
            if !present && migration.version < latest {
                return Err(invalid(format!(
                    "pending migration {} precedes applied version {latest}; append a newer version",
                    migration.version
                )));
            }
            result.push(MigrationStatus {
                version: migration.version,
                description: migration.description.to_string(),
                applied: present,
            });
        }
        Ok(result)
    }

    /// Apply pending migrations in order, one transaction per file.
    /// SQLx stores checksums and versions in _sqlx_migrations. Earlier successful
    /// files remain committed if a later file fails. Scripts must not contain
    /// transaction-control statements; the runner owns transaction boundaries.
    #[cfg(feature = "sqlite")]
    pub async fn run(&self, database: &Database) -> io::Result<usize> {
        let pending = self
            .status(database)
            .await?
            .iter()
            .filter(|m| !m.applied)
            .count();
        self.migrator.run(database.pool()).await.map_err(|error| {
            match error {
                MigrateError::ExecuteMigration(_, version) => invalid(format!("migration {version} failed; its transaction was rolled back; fix the pending SQL and retry")),
                MigrateError::VersionMismatch(version) => invalid(format!("applied migration {version} has changed")),
                MigrateError::VersionMissing(version) => invalid(format!("applied migration {version} is missing")),
                _ => invalid("cannot apply migrations; check database access and migration history"),
            }
        })?;
        Ok(pending)
    }
}

#[cfg(feature = "sqlite")]
impl Database {
    /// Load and apply a directory of forward-only migrations.
    pub async fn migrate(&self, directory: impl AsRef<Path>) -> io::Result<usize> {
        Migrations::load(directory).await?.run(self).await
    }

    /// Inspect migration state without running pending SQL.
    pub async fn migration_status(
        &self,
        directory: impl AsRef<Path>,
    ) -> io::Result<Vec<MigrationStatus>> {
        Migrations::load(directory).await?.status(self).await
    }
}

#[cfg(feature = "postgres")]
impl Migrations {
    async fn postgres_status_connection(
        &self,
        connection: &mut sqlx::PgConnection,
    ) -> io::Result<Vec<MigrationStatus>> {
        let (exists,): (bool,) =
            sqlx::query_as("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
                .fetch_one(&mut *connection)
                .await
                .map_err(|_| invalid("cannot inspect PostgreSQL migration history"))?;
        let mut applied = BTreeMap::new();
        if exists {
            let rows: Vec<(i64, bool, Vec<u8>)> = sqlx::query_as(
                "SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version",
            )
            .fetch_all(&mut *connection)
            .await
            .map_err(|_| invalid("cannot read PostgreSQL migration history"))?;
            for (version, success, checksum) in rows {
                if !success {
                    return Err(invalid(format!(
                        "migration {version} is incomplete; inspect database history before continuing"
                    )));
                }
                applied.insert(version, checksum);
            }
        }
        self.validated_status(applied)
    }

    /// Read history in the connection's current PostgreSQL search path.
    pub async fn status_postgres(
        &self,
        database: &PostgresDatabase,
    ) -> io::Result<Vec<MigrationStatus>> {
        let mut connection = database
            .pool()
            .acquire()
            .await
            .map_err(|_| invalid("cannot acquire PostgreSQL migration connection"))?;
        self.postgres_status_connection(&mut connection).await
    }

    /// Serialize migrations with SQLx's PostgreSQL advisory lock.
    /// Validation and per-file application share the same locked connection.
    /// The connection is closed on success or error, releasing session locks.
    pub async fn run_postgres(&self, database: &PostgresDatabase) -> io::Result<usize> {
        use sqlx::migrate::Migrate;
        let mut connection = database
            .pool()
            .acquire()
            .await
            .map_err(|_| invalid("cannot acquire PostgreSQL migration connection"))?;
        let result: io::Result<usize> = async {
            connection.lock().await
                .map_err(|_| invalid("cannot lock PostgreSQL migrations; check database access or retry after the other migrator finishes"))?;
            let statuses = self.postgres_status_connection(&mut connection).await?;
            connection.ensure_migrations_table().await
                .map_err(|_| invalid("cannot initialize PostgreSQL migration history"))?;
            let mut applied = 0;
            for (migration, status) in self.migrator.iter().zip(statuses) {
                if status.applied { continue; }
                connection.apply(migration).await.map_err(|error| match error {
                    MigrateError::ExecuteMigration(_, version) => invalid(format!(
                        "migration {version} failed; its transaction was rolled back; fix the pending SQL and retry")),
                    _ => invalid(format!(
                        "cannot complete migration {}; inspect database history before retrying",
                        migration.version)),
                })?;
                applied += 1;
            }
            Ok(applied)
        }.await;
        let close = connection
            .close()
            .await
            .map_err(|_| invalid("cannot close PostgreSQL migration connection"));
        result.and_then(|count| close.map(|_| count))
    }
}
