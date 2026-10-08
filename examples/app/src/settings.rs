use std::{net::Ipv4Addr, sync::Arc, time::Duration};

use ruvoraq::{App, Cors, Settings};

use apps::{billing::services::BillingService, school::services::SchoolService};

mod apps;
mod notes;

pub const APP_NAME: &str = "app";
pub const HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const PORT: u16 = 8000;

pub fn settings() -> Settings {
    Settings {
        app_name: APP_NAME.to_owned(),
        address: (HOST, PORT).into(),
    }
}

async fn configure(app: App) -> std::io::Result<App> {
    let greeting = app
        .env()
        .get_or("SCHOOL_GREETING", "School API".to_owned())?;
    #[cfg(not(feature = "postgres"))]
    let database = {
        let url = app
            .env()
            .get_or("DATABASE_URL", "sqlite://notes.sqlite".to_owned())?;
        let db = ruvoraq::Database::connect(&url).await?;
        let directory = app
            .env()
            .get_or("MIGRATIONS_DIR", "migrations/sqlite".to_owned())?;
        db.migrate(directory).await?;
        db
    };
    #[cfg(feature = "postgres")]
    let database = {
        let url: String = app.env().get("DATABASE_URL")?;
        let db = ruvoraq::PostgresDatabase::connect(&url).await?;
        let directory = app
            .env()
            .get_or("MIGRATIONS_DIR", "migrations/postgres".to_owned())?;
        db.migrate(directory).await?;
        db
    };
    let logging = app.env().get_or("RUVORAQ_REQUEST_LOG", true)?;
    let timeout_ms = app.env().get_or("RUVORAQ_REQUEST_TIMEOUT_MS", 2000u64)?;
    let frontend = app
        .env()
        .get_or("FRONTEND_ORIGIN", "http://localhost:3000".to_owned())?;
    let cors = Cors::new([frontend])?;
    Ok(app
        .request_logging(logging)
        .request_timeout(Duration::from_millis(timeout_ms))
        .cors(cors)
        .provide(database)
        .provide(SchoolService::with_greeting(greeting))
        .provide_shared(Arc::new(BillingService::default())))
}

ruvoraq::bootstrap!(async configure);
