use super::models::{CreateTask, Task};
use ruvoraq::prelude::*;

#[get("/")]
async fn hello() -> &'static str {
    "SQLite migration demo"
}

/// List tasks persisted in SQLite.
#[get("/tasks")]
async fn tasks(db: Inject<Database>) -> Result<Vec<Task>> {
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, title FROM tasks ORDER BY id")
        .fetch_all(db.pool())
        .await
        .map_err(|_| Error::internal())?;
    Ok(rows.into_iter().map(Task::from).collect())
}

/// Create a validated task.
#[post("/tasks", status = 201)]
async fn create_task(
    db: Inject<Database>,
    ValidatedJson(input): ValidatedJson<CreateTask>,
) -> Result<Reply<Task>> {
    let row: (i64, String) =
        sqlx::query_as("INSERT INTO tasks (title) VALUES (?) RETURNING id, title")
            .bind(input.title.trim())
            .fetch_one(db.pool())
            .await
            .map_err(|_| Error::internal())?;
    Ok(created(Task::from(row)))
}

/// Read a task by its database identifier.
#[get("/tasks/{id}")]
async fn task(db: Inject<Database>, Path(id): Path<i64>) -> Result<Task> {
    let row: Option<(i64, String)> = sqlx::query_as("SELECT id, title FROM tasks WHERE id = ?")
        .bind(id)
        .fetch_optional(db.pool())
        .await
        .map_err(|_| Error::internal())?;
    row.map(Task::from)
        .ok_or_else(|| not_found("Task not found"))
}
