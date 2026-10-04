use std::{
    env, fs,
    path::Path,
    process::{Command, ExitCode},
};

/// Run only explicitly marked Ruvoraq projects in the current directory.
pub fn run(project: &Path) -> Result<ExitCode, String> {
    let manifest = project.join("Cargo.toml");
    let source = fs::read_to_string(&manifest).map_err(|error| {
        format!(
            "cannot read '{}': {error}; run 'ruvoraq dev' from a Ruvoraq project",
            manifest.display()
        )
    })?;
    let value: toml::Table = source
        .parse()
        .map_err(|error| format!("invalid Cargo.toml '{}': {error}", manifest.display()))?;
    if value
        .get("package")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("ruvoraq"))
        .and_then(|value| value.get("project"))
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Err("not a Ruvoraq project: Cargo.toml must contain [package.metadata.ruvoraq] with project = true".into());
    }

    let mut cargo = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cargo
        .arg("run")
        .arg("--manifest-path")
        .arg(&manifest)
        .current_dir(project);

    // Replacing this process lets Cargo preserve terminal signals and exit status.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(format!("cannot launch Cargo: {}", cargo.exec()))
    }
    #[cfg(not(unix))]
    {
        let status = cargo
            .status()
            .map_err(|error| format!("cannot launch Cargo: {error}"))?;
        Ok(ExitCode::from(
            status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(1),
        ))
    }
}
