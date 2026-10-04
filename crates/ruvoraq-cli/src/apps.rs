use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);

fn read_file(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect '{}': {error}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "'{}' must be a regular file, not a symlink",
            path.display()
        ));
    }
    fs::read_to_string(path).map_err(|error| format!("cannot read '{}': {error}", path.display()))
}

fn regular_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect '{}': {error}", path.display()))?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "'{}' must be a regular directory, not a symlink",
            path.display()
        ));
    }
    Ok(())
}

fn exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("cannot inspect '{}': {error}", path.display())),
    }
}

fn parse_source(path: &Path, source: &str) -> Result<syn::File, String> {
    syn::parse_file(source)
        .map_err(|error| format!("invalid Rust in '{}': {error}", path.display()))
}

fn append(source: &str, declaration: &str) -> String {
    if source.trim_end().is_empty() {
        format!("{declaration}\n")
    } else {
        format!("{}\n{declaration}\n", source.trim_end())
    }
}

// Stage in the same directory, preserving permissions, then replace atomically.
// Refuse a changed original rather than silently overwriting a concurrent edit.
fn replace(path: &Path, original: &str, updated: &str) -> Result<(), String> {
    if fs::metadata(path)
        .map_err(|error| format!("cannot inspect '{}': {error}", path.display()))?
        .permissions()
        .readonly()
    {
        return Err(format!(
            "'{}' is read-only; refusing to update it",
            path.display()
        ));
    }
    if read_file(path)? != original {
        return Err(format!(
            "'{}' changed while adding the app; retry",
            path.display()
        ));
    }
    let temporary = path.with_file_name(format!(
        ".ruvoraq-{}-{}.tmp",
        std::process::id(),
        NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("cannot stage '{}': {error}", path.display()))?;
    let result = (|| {
        file.write_all(updated.as_bytes())?;
        file.set_permissions(fs::metadata(path)?.permissions())?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(format!("cannot update '{}': {error}", path.display()));
    }
    Ok(())
}

struct Change {
    path: PathBuf,
    original: Option<String>,
    updated: String,
}

impl Change {
    fn apply(&self) -> Result<(), String> {
        if let Some(original) = &self.original {
            replace(&self.path, original, &self.updated)
        } else {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.path)
                .map_err(|error| format!("cannot create '{}': {error}", self.path.display()))?;
            if let Err(error) = file.write_all(self.updated.as_bytes()) {
                drop(file);
                let _ = fs::remove_file(&self.path);
                return Err(format!("cannot write '{}': {error}", self.path.display()));
            }
            Ok(())
        }
    }

    fn undo(&self) -> Result<(), String> {
        if read_file(&self.path)? != self.updated {
            return Err(format!(
                "'{}' was edited; leaving it untouched",
                self.path.display()
            ));
        }
        if let Some(original) = &self.original {
            replace(&self.path, &self.updated, original)
        } else {
            fs::remove_file(&self.path)
                .map_err(|error| format!("cannot remove '{}': {error}", self.path.display()))
        }
    }
}

/// Add an ordinary Rust module without changing main.rs or project dependencies.
pub fn add(project: &Path, name: &str) -> Result<PathBuf, String> {
    super::project::validate_name(name)
        .map_err(|error| error.replace("project name", "app name"))?;
    if !name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(
            "invalid app name: use lowercase ASCII letters, digits and underscores (snake_case)"
                .into(),
        );
    }
    let manifest_path = project.join("Cargo.toml");
    let manifest: toml::Table = read_file(&manifest_path)?
        .parse()
        .map_err(|error| format!("invalid Cargo.toml: {error}"))?;
    if manifest
        .get("package")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("ruvoraq"))
        .and_then(|v| v.get("project"))
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Err("not a Ruvoraq project; run 'ruvoraq add app' from its root".into());
    }
    let binaries = manifest.get("bin").and_then(toml::Value::as_array);
    if !binaries.is_some_and(|bins| {
        bins.len() == 1
            && bins[0].get("path").and_then(toml::Value::as_str) == Some("src/settings.rs")
    }) || manifest
        .get("package")
        .and_then(|v| v.get("autobins"))
        .and_then(toml::Value::as_bool)
        != Some(false)
    {
        return Err("add app requires the route-only scaffold: autobins = false and a single binary at src/settings.rs".into());
    }

    let src = project.join("src");
    regular_directory(&src)?;
    let settings_path = src.join("settings.rs");
    let settings = read_file(&settings_path)?;
    let syntax = parse_source(&settings_path, &settings)?;
    let bootstrap = syntax.items.iter().any(|item| matches!(item, syn::Item::Macro(item)
        if item.mac.path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>() == ["ruvoraq", "bootstrap"]));
    if !bootstrap {
        return Err("settings.rs must contain ruvoraq::bootstrap!(); migrate to the route-only scaffold first".into());
    }
    let declarations: Vec<_> = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) if module.ident == "apps" => Some(module),
            _ => None,
        })
        .collect();
    if declarations.len() > 1
        || declarations.iter().any(|module| {
            module.content.is_some()
                || module
                    .attrs
                    .iter()
                    .any(|attribute| !attribute.path().is_ident("doc"))
        })
    {
        return Err("settings.rs has custom apps wiring; use one unconditional external 'mod apps;' declaration".into());
    }
    let main_path = src.join("main.rs");
    let main = read_file(&main_path)?;
    if parse_source(&main_path, &main)?.items.iter().any(|item| {
        matches!(item,
        syn::Item::Mod(module) if module.ident == "apps")
    }) {
        return Err(
            "apps is already declared in main.rs; move that declaration to settings.rs first"
                .into(),
        );
    }
    if exists(&src.join("apps.rs"))? {
        return Err("src/apps.rs already exists; add app uses src/apps/mod.rs".into());
    }

    let apps = src.join("apps");
    let existing_apps = exists(&apps)?;
    if existing_apps {
        regular_directory(&apps)?;
    }
    let sibling = apps.join(format!("{name}.rs"));
    if exists(&sibling)? {
        return Err(format!(
            "app source '{}' already exists; refusing to create an ambiguous module",
            sibling.display()
        ));
    }
    let target = apps.join(name);
    if exists(&target)? {
        return Err(format!(
            "app target '{}' already exists; refusing to overwrite it",
            target.display()
        ));
    }
    let registry = apps.join("mod.rs");
    let original_registry = if exists(&registry)? {
        Some(read_file(&registry)?)
    } else {
        None
    };
    if let Some(source) = &original_registry {
        let syntax = parse_source(&registry, source)?;
        if syntax.items.iter().any(|item| {
            matches!(item,
            syn::Item::Mod(module) if module.ident == name)
        }) {
            return Err(format!(
                "app '{name}' is already declared in src/apps/mod.rs"
            ));
        }
    }

    let mut changes = Vec::new();
    for (filename, content) in [
        ("mod.rs", "mod models;\nmod routes;\nmod services;\n".to_owned()),
        ("models.rs", "use ruvoraq::prelude::*;\n\n#[schema]\n#[derive(Serialize)]\n#[serde(crate = \"ruvoraq::serde\")]\npub struct AppInfo {\n    pub name: &'static str,\n    pub message: &'static str,\n}\n".to_owned()),
        ("services.rs", format!("pub fn greeting() -> &'static str {{\n    \"Hello from {name}\"\n}}\n")),
        ("routes.rs", format!("use ruvoraq::prelude::*;\n\nuse super::{{models::AppInfo, services}};\n\n#[get(\"/{name}\")]\nasync fn index() -> AppInfo {{\n    AppInfo {{\n        name: \"{name}\",\n        message: services::greeting(),\n    }}\n}}\n")),
    ] {
        changes.push(Change { path: target.join(filename), original: None, updated: content });
    }
    changes.push(Change {
        path: registry,
        updated: append(
            original_registry.as_deref().unwrap_or(""),
            &format!("pub mod {name};"),
        ),
        original: original_registry,
    });
    if declarations.is_empty() {
        changes.push(Change {
            path: settings_path,
            updated: append(&settings, "mod apps;"),
            original: Some(settings),
        });
    }

    let mut directories = Vec::new();
    let mut applied = Vec::new();
    let result = (|| {
        if !existing_apps {
            fs::create_dir(&apps)
                .map_err(|error| format!("cannot create '{}': {error}", apps.display()))?;
            directories.push(apps.clone());
        }
        fs::create_dir(&target)
            .map_err(|error| format!("cannot create '{}': {error}", target.display()))?;
        directories.push(target.clone());
        for change in &changes {
            change.apply()?;
            applied.push(change);
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        for change in applied.into_iter().rev() {
            if let Err(error) = change.undo() {
                failures.push(error);
            }
        }
        for directory in directories.into_iter().rev() {
            if let Err(error) = fs::remove_dir(&directory) {
                failures.push(format!("cannot remove '{}': {error}", directory.display()));
            }
        }
        return if failures.is_empty() {
            Err(error)
        } else {
            Err(format!(
                "{error}; rollback incomplete: {}",
                failures.join("; ")
            ))
        };
    }
    Ok(target)
}
