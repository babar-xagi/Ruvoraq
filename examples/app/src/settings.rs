use std::{net::Ipv4Addr, sync::Arc};

use ruvoraq::{App, Settings};

use apps::{billing::services::BillingService, school::services::SchoolService};

mod apps;

pub const APP_NAME: &str = "app";
pub const HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const PORT: u16 = 8000;

pub fn settings() -> Settings {
    Settings {
        app_name: APP_NAME.to_owned(),
        address: (HOST, PORT).into(),
    }
}

fn configure(app: App) -> std::io::Result<App> {
    let greeting = app
        .env()
        .get_or("SCHOOL_GREETING", "School API".to_owned())?;
    Ok(app
        .provide(SchoolService::with_greeting(greeting))
        .provide_shared(Arc::new(BillingService::default())))
}

ruvoraq::bootstrap!(configure);
