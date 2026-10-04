use ruvoraq_db::{Database, sqlx};
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[tokio::test]
async fn bound_values_and_cloned_pool_share_memory_without_sql_injection() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();
    let title = "'); DROP TABLE notes; --";
    sqlx::query("INSERT INTO notes (title) VALUES (?)")
        .bind(title)
        .execute(db.pool())
        .await
        .unwrap();
    let clone = db.clone();
    let row: (i64, String) = sqlx::query_as("SELECT id,title FROM notes")
        .fetch_one(clone.pool())
        .await
        .unwrap();
    assert_eq!(row, (1, title.to_owned()));
    db.close().await;
}
#[tokio::test]
async fn transactions_commit_rollback_and_drop_work() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE values_table (value INTEGER)")
        .execute(db.pool())
        .await
        .unwrap();
    let mut tx = db.begin().await.unwrap();
    sqlx::query("INSERT INTO values_table VALUES (1)")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut tx = db.begin().await.unwrap();
    sqlx::query("INSERT INTO values_table VALUES (2)")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    let mut tx = db.begin().await.unwrap();
    sqlx::query("INSERT INTO values_table VALUES (3)")
        .execute(&mut *tx)
        .await
        .unwrap();
    drop(tx);
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT value FROM values_table")
        .fetch_all(db.pool())
        .await
        .unwrap();
    assert_eq!(rows, vec![(1,)]);
    db.close().await;
}
#[tokio::test]
async fn file_records_survive_closing_and_reopening() {
    let dir = std::env::temp_dir().join(format!(
        "ruvoraq-db-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    let file = dir.join("notes.sqlite");
    let url = format!("sqlite://{}", file.display());
    let db = Database::connect(&url).await.unwrap();
    sqlx::query("CREATE TABLE notes (title TEXT)")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO notes VALUES (?)")
        .bind("persisted")
        .execute(db.pool())
        .await
        .unwrap();
    db.close().await;
    let db = Database::connect(&url).await.unwrap();
    let row: (String,) = sqlx::query_as("SELECT title FROM notes")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row.0, "persisted");
    db.close().await;
    fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn foreign_keys_are_enforced_and_startup_errors_are_redacted() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE parents (id INTEGER PRIMARY KEY)")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("CREATE TABLE children (parent_id INTEGER REFERENCES parents(id))")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        sqlx::query("INSERT INTO children VALUES (42)")
            .execute(db.pool())
            .await
            .is_err()
    );
    db.close().await;
    for url in [
        "postgres://private-secret",
        "sqlite://missing-private-directory/db.sqlite",
        "sqlite://invalid?mode=private-secret",
    ] {
        let error = Database::connect(url).await.unwrap_err();
        assert!(!error.to_string().contains("private-secret"));
        assert!(!error.to_string().contains("missing-private-directory"));
    }
}
