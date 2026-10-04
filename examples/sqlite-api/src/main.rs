use super::models::{CreateNote, Note};
use ruvoraq::prelude::*;

#[get("/")]
async fn hello() -> &'static str {
    "Persistent notes API"
}

#[get("/notes")]
async fn notes(db: Inject<Database>) -> Result<Vec<Note>> {
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id,title FROM notes ORDER BY id")
        .fetch_all(db.pool())
        .await
        .map_err(|_| Error::internal())?;
    Ok(rows.into_iter().map(Note::from).collect())
}
#[post("/notes", status = 201)]
async fn create(
    db: Inject<Database>,
    ValidatedJson(input): ValidatedJson<CreateNote>,
) -> Result<Reply<Note>> {
    let row: (i64, String) =
        sqlx::query_as("INSERT INTO notes (title) VALUES (?) RETURNING id,title")
            .bind(input.title.trim())
            .fetch_one(db.pool())
            .await
            .map_err(|_| Error::internal())?;
    Ok(created(Note::from(row)))
}
#[get("/notes/{id}")]
async fn note(db: Inject<Database>, Path(id): Path<i64>) -> Result<Note> {
    let row: Option<(i64, String)> = sqlx::query_as("SELECT id,title FROM notes WHERE id=?")
        .bind(id)
        .fetch_optional(db.pool())
        .await
        .map_err(|_| Error::internal())?;
    row.map(Note::from)
        .ok_or_else(|| not_found("Note not found"))
}
#[put("/notes/{id}")]
async fn update(
    db: Inject<Database>,
    Path(id): Path<i64>,
    ValidatedJson(input): ValidatedJson<CreateNote>,
) -> Result<Note> {
    let row: Option<(i64, String)> =
        sqlx::query_as("UPDATE notes SET title=? WHERE id=? RETURNING id,title")
            .bind(input.title.trim())
            .bind(id)
            .fetch_optional(db.pool())
            .await
            .map_err(|_| Error::internal())?;
    row.map(Note::from)
        .ok_or_else(|| not_found("Note not found"))
}
#[delete("/notes/{id}", status = 204)]
async fn remove(db: Inject<Database>, Path(id): Path<i64>) -> Result<Response> {
    let result = sqlx::query("DELETE FROM notes WHERE id=?")
        .bind(id)
        .execute(db.pool())
        .await
        .map_err(|_| Error::internal())?;
    if result.rows_affected() == 0 {
        return Err(not_found("Note not found"));
    }
    Ok(no_content())
}
