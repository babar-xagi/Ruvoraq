use ruvoraq::{App, Database, Settings};
use std::net::Ipv4Addr;

mod models;

pub const APP_NAME: &str = "migration-demo";
pub const HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const PORT: u16 = 8010;

pub fn settings() -> Settings {
    Settings {
        app_name: APP_NAME.to_owned(),
        address: (HOST, PORT).into(),
    }
}

async fn configure(app: App) -> std::io::Result<App> {
    let url: String = app.env().get("DATABASE_URL")?;
    let db = Database::connect(&url).await?;
    db.migrate("migrations").await?;
    Ok(app.provide(db))
}

ruvoraq::bootstrap!(async configure);
