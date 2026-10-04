use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

fn validate_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    if !chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        || name.len() > 64
    {
        return Err(format!(
            "invalid project name {name:?}: use 1–64 ASCII letters, digits, '-' or '_', \
             starting with a letter or underscore (no paths)"
        ));
    }

    // Reject strict and reserved Rust keywords, plus Cargo artifact directories.
    if matches!(
        name,
        "_" | "as"
            | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "gen"
            | "macro"
            | "override"
            | "priv"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "build"
            | "deps"
            | "examples"
            | "incremental"
    ) {
        return Err(format!(
            "reserved project name {name:?}; choose another name"
        ));
    }

    let lower = name.to_ascii_lowercase();
    if matches!(lower.as_str(), "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|prefix| {
            lower.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        })
    {
        return Err(format!(
            "reserved Windows device name {name:?}; choose another name"
        ));
    }
    Ok(())
}

fn io_error(action: &str, path: &Path, error: io::Error) -> String {
    format!("cannot {action} '{}': {error}", path.display())
}

pub fn create(parent: &Path, name: &str) -> Result<PathBuf, String> {
    validate_name(name)?;
    let target = parent.join(name);
    let created_target = match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(format!(
                    "target '{}' already exists and is not a regular directory",
                    target.display()
                ));
            }
            let mut entries = fs::read_dir(&target)
                .map_err(|error| io_error("inspect target directory", &target, error))?;
            if let Some(entry) = entries.next() {
                entry.map_err(|error| io_error("inspect target directory", &target, error))?;
                return Err(format!(
                    "target directory '{}' is not empty; refusing to overwrite it",
                    target.display()
                ));
            }
            false
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&target)
                .map_err(|error| io_error("create target directory", &target, error))?;
            true
        }
        Err(error) => return Err(io_error("inspect target", &target, error)),
    };

    let src = target.join("src");
    let mut created_files = Vec::new();
    let mut created_src = false;
    let result = (|| {
        fs::create_dir(&src).map_err(|error| io_error("create source directory", &src, error))?;
        created_src = true;

        let manifest = format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\n\n\
             # Keep this project independent of any surrounding Cargo workspace.\n\
             [workspace]\n"
        );
        let main = "mod settings;\n\nfn main() {\n    println!(\"Hello from {}!\", settings::APP_NAME);\n}\n";
        let settings = format!("pub const APP_NAME: &str = \"{name}\";\n");

        for (relative, content) in [
            ("Cargo.toml", manifest.as_str()),
            ("src/main.rs", main),
            ("src/settings.rs", settings.as_str()),
        ] {
            let path = target.join(relative);
            // Exclusive creation also prevents overwriting files added after inspection.
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|error| io_error("create file", &path, error))?;
            created_files.push(path.clone());
            file.write_all(content.as_bytes())
                .map_err(|error| io_error("write file", &path, error))?;
        }
        Ok(())
    })();

    if let Err(error) = result {
        // Remove only entries created by this call; never recursively remove a target.
        let mut cleanup_errors = Vec::new();
        for path in created_files.iter().rev() {
            if let Err(error) = fs::remove_file(path) {
                cleanup_errors.push(io_error("clean up file", path, error));
            }
        }
        if created_src {
            if let Err(error) = fs::remove_dir(&src) {
                cleanup_errors.push(io_error("clean up source directory", &src, error));
            }
        }
        if created_target {
            if let Err(error) = fs::remove_dir(&target) {
                cleanup_errors.push(io_error("clean up target directory", &target, error));
            }
        }
        return if cleanup_errors.is_empty() {
            Err(error)
        } else {
            Err(format!(
                "{error}; cleanup incomplete: {}",
                cleanup_errors.join("; ")
            ))
        };
    }
    Ok(target)
}
