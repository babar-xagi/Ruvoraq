//! Forward-only SQLite migrations with validation before applying pending files.
use crate::{Database, sqlx};
use sqlx::migrate::{MigrateError, Migrator};
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
    /// Load forward-only files named <positive-version>_<lowercase_description>.sql.
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
