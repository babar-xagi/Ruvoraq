use ruvoraq::{App, Database, Settings, sqlx};
use std::net::Ipv4Addr;
mod models;
pub const APP_NAME: &str = "sqlite-api";
pub const HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const PORT: u16 = 8000;
pub fn settings() -> Settings {
    Settings {
        app_name: APP_NAME.into(),
        address: (HOST, PORT).into(),
    }
}
async fn configure(app: App) -> std::io::Result<App> {
    let url = app
        .env()
        .get_or("DATABASE_URL", "sqlite://notes.sqlite".to_owned())?;
    let db = Database::connect(&url).await?;
    // An initial idempotent schema, not a versioned migration system.
    sqlx::query("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 120))")
        .execute(db.pool()).await.map_err(|_|std::io::Error::other("cannot initialize notes schema"))?;
    Ok(app.provide(db))
}
ruvoraq::bootstrap!(async configure);
