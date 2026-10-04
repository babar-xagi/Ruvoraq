#![cfg(feature = "sqlite")]
use ruvoraq_db::{Database, Migrations, sqlx};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ruvoraq-migrations-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, sql: &str) {
        fs::write(self.0.join(name), sql).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
async fn table_exists(database: &Database, table: &str) -> bool {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?")
            .bind(table)
            .fetch_one(database.pool())
            .await
            .unwrap();
    count != 0
}

#[tokio::test]
async fn ordered_files_status_repeat_and_append_preserve_data() {
    let files = Directory::new();
    files.write("0010_add_value.sql", "INSERT INTO notes VALUES ('first');");
    files.write("0002_create_notes.sql", "CREATE TABLE notes (title TEXT);");
    let database = Database::connect("sqlite::memory:").await.unwrap();
    let states = database.migration_status(&files.0).await.unwrap();
    assert_eq!(
        states
            .iter()
            .map(|m| (m.version, m.applied))
            .collect::<Vec<_>>(),
        vec![(2, false), (10, false)]
    );
    assert!(!table_exists(&database, "_sqlx_migrations").await);
    assert!(!table_exists(&database, "notes").await);
    assert_eq!(database.migrate(&files.0).await.unwrap(), 2);
    assert_eq!(database.migrate(&files.0).await.unwrap(), 0);
    files.write(
        "0011_add_second.sql",
        "INSERT INTO notes VALUES ('second');",
    );
    assert_eq!(database.migrate(&files.0).await.unwrap(), 1);
    let rows: Vec<(String,)> = sqlx::query_as("SELECT title FROM notes ORDER BY rowid")
        .fetch_all(database.pool())
        .await
        .unwrap();
    assert_eq!(rows, vec![("first".into(),), ("second".into(),)]);
    assert!(
        database
            .migration_status(&files.0)
            .await
            .unwrap()
            .iter()
            .all(|m| m.applied)
    );
    database.close().await;
}

#[tokio::test]
async fn failed_file_rolls_back_and_can_be_fixed_without_reapplying_history() {
    let files = Directory::new();
    files.write(
        "1_create_notes.sql",
        "CREATE TABLE notes (title TEXT); INSERT INTO notes VALUES ('original');",
    );
    files.write("2_broken.sql", "CREATE TABLE transient (value TEXT); INSERT INTO notes VALUES ('temporary'); INSERT INTO private_secret_missing_table VALUES (1);");
    let database = Database::connect("sqlite::memory:").await.unwrap();
    let error = database.migrate(&files.0).await.unwrap_err().to_string();
    assert!(error.contains("migration 2 failed"));
    assert!(!error.contains("private_secret"));
    assert!(!table_exists(&database, "transient").await);
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        database
            .migration_status(&files.0)
            .await
            .unwrap()
            .iter()
            .map(|m| m.applied)
            .collect::<Vec<_>>(),
        vec![true, false]
    );
    files.write("2_broken.sql", "INSERT INTO notes VALUES ('fixed');");
    assert_eq!(database.migrate(&files.0).await.unwrap(), 1);
    assert_eq!(database.migrate(&files.0).await.unwrap(), 0);
    database.close().await;
}

#[tokio::test]
async fn changed_missing_and_older_files_fail_before_pending_sql() {
    let files = Directory::new();
    let original = "CREATE TABLE notes (title TEXT);";
    files.write("2_create_notes.sql", original);
    let database = Database::connect("sqlite::memory:").await.unwrap();
    database.migrate(&files.0).await.unwrap();
    files.write(
        "3_pending.sql",
        "CREATE TABLE should_not_exist (id INTEGER);",
    );
    files.write(
        "2_create_notes.sql",
        "CREATE TABLE notes (title TEXT, changed INTEGER);",
    );
    assert!(
        database
            .migrate(&files.0)
            .await
            .unwrap_err()
            .to_string()
            .contains("has changed")
    );
    assert!(!table_exists(&database, "should_not_exist").await);
    fs::remove_file(files.0.join("2_create_notes.sql")).unwrap();
    assert!(
        database
            .migration_status(&files.0)
            .await
            .unwrap_err()
            .to_string()
            .contains("is missing")
    );
    assert!(!table_exists(&database, "should_not_exist").await);
    files.write("2_create_notes.sql", original);
    files.write(
        "1_old.sql",
        "CREATE TABLE also_should_not_exist (id INTEGER);",
    );
    assert!(
        database
            .migrate(&files.0)
            .await
            .unwrap_err()
            .to_string()
            .contains("precedes")
    );
    assert!(!table_exists(&database, "also_should_not_exist").await);
    fs::remove_file(files.0.join("1_old.sql")).unwrap();
    assert_eq!(database.migrate(&files.0).await.unwrap(), 1);
    // Dirty history must not be bypassed by status or apply.
    sqlx::query("UPDATE _sqlx_migrations SET success=false WHERE version=2")
        .execute(database.pool())
        .await
        .unwrap();
    assert!(
        database
            .migration_status(&files.0)
            .await
            .unwrap_err()
            .to_string()
            .contains("incomplete")
    );
    database.close().await;
}

#[tokio::test]
async fn invalid_sources_are_refused_instead_of_silently_ignored() {
    for (name, content) in [
        ("bad.sql", "SELECT 1;"),
        ("0_zero.sql", "SELECT 1;"),
        ("-1_negative.sql", "SELECT 1;"),
        ("1_.sql", "SELECT 1;"),
        ("1_UPPER.sql", "SELECT 1;"),
        ("1_example.up.sql", "SELECT 1;"),
        ("1_example.sql", ""),
        ("1_example.sql", "-- no-transaction\nSELECT 1;"),
        ("9999999999999999999999_large.sql", "SELECT 1;"),
    ] {
        let files = Directory::new();
        files.write(name, content);
        assert!(Migrations::load(&files.0).await.is_err(), "{name}");
    }
    let files = Directory::new();
    assert!(Migrations::load(&files.0).await.is_err());
    files.write("1_first.sql", "SELECT 1;");
    files.write("01_duplicate.sql", "SELECT 1;");
    assert!(
        Migrations::load(&files.0)
            .await
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
    assert!(
        Migrations::load(Path::new("/missing-ruvoraq-migrations"))
            .await
            .is_err()
    );
    #[cfg(unix)]
    {
        let files = Directory::new();
        files.write("source.txt", "SELECT 1;");
        std::os::unix::fs::symlink(files.0.join("source.txt"), files.0.join("1_link.sql")).unwrap();
        assert!(Migrations::load(&files.0).await.is_err());
    }
}

#[tokio::test]
async fn reopened_database_retains_history_and_legacy_example_data() {
    let files = Directory::new();
    files.write(
        "1_create_notes.sql",
        "CREATE TABLE IF NOT EXISTS notes (title TEXT);",
    );
    let file = files.0.join("database.sqlite");
    let url = format!("sqlite://{}", file.display());
    let database = Database::connect(&url).await.unwrap();
    sqlx::query("CREATE TABLE notes (title TEXT); INSERT INTO notes VALUES ('legacy');")
        .execute(database.pool())
        .await
        .unwrap();
    assert_eq!(database.migrate(&files.0).await.unwrap(), 1);
    database.close().await;
    let database = Database::connect(&url).await.unwrap();
    assert_eq!(database.migrate(&files.0).await.unwrap(), 0);
    let (title,): (String,) = sqlx::query_as("SELECT title FROM notes")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(title, "legacy");
    database.close().await;
}
