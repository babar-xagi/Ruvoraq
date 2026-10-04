use ruvoraq_config::Env;
use ruvoraq_db::{Database, Migrations};
use std::{fs, path::Path};

/// Apply or inspect migrations without compiling or starting the application.
pub fn run(project: &Path, status: bool) -> Result<(), String> {
    let source = fs::read_to_string(project.join("Cargo.toml"))
        .map_err(|_| "run 'ruvoraq migrate' from a Ruvoraq project root".to_owned())?;
    let manifest: toml::Table = source
        .parse()
        .map_err(|_| "invalid Cargo.toml".to_owned())?;
    if manifest
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("ruvoraq"))
        .and_then(|r| r.get("project"))
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Err("not a Ruvoraq project: Cargo.toml must contain [package.metadata.ruvoraq] with project = true".into());
    }
    let env = Env::load_from(project).map_err(|e| e.to_string())?;
    let url: String = env.get("DATABASE_URL").map_err(|e| e.to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "cannot start migration runtime".to_owned())?;
    runtime.block_on(async {
        // Validate sources before opening/creating a database.
        let migrations = Migrations::load(project.join("migrations"))
            .await
            .map_err(|e| e.to_string())?;
        let database = Database::connect(&url).await.map_err(|e| e.to_string())?;
        let result = if status {
            migrations.status(&database).await.map(|items| {
                for item in items {
                    println!(
                        "{}  {}  {}",
                        item.version,
                        if item.applied { "applied" } else { "pending" },
                        item.description
                    );
                }
            })
        } else {
            migrations
                .run(&database)
                .await
                .map(|count| println!("Applied {count} migration(s)."))
        };
        database.close().await;
        result.map_err(|e| e.to_string())
    })
}
