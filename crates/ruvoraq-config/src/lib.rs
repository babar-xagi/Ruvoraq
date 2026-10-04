//! Typed configuration snapshots. Process environment overrides optional .env
//! values; no loading method changes the process environment.

use std::{
    any::type_name, collections::HashMap, ffi::OsString, fmt, fs::File, io, path::Path,
    str::FromStr,
};

/// A configuration snapshot. Debug output never includes names or values.
#[derive(Clone)]
pub struct Env {
    process: HashMap<OsString, OsString>,
    file: HashMap<String, String>,
}
impl Default for Env {
    fn default() -> Self {
        Self {
            process: std::env::vars_os().collect(),
            file: HashMap::new(),
        }
    }
}
impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Env")
            .field("process_entries", &self.process.len())
            .field("file_entries", &self.file.len())
            .finish_non_exhaustive()
    }
}
impl Env {
    /// Snapshot process variables and load only .env in the current directory.
    /// Missing .env is allowed; malformed or unreadable files return an error.
    pub fn load() -> io::Result<Self> {
        Self::load_from(".")
    }

    /// Load an optional .env from exactly this directory (no parent search).
    pub fn load_from(directory: impl AsRef<Path>) -> io::Result<Self> {
        let env = Self::default();
        let path = directory.as_ref().join(".env");
        match File::open(&path) {
            Ok(file) => env.read_file(file, &path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(env),
            Err(error) => Err(file_error(
                &path,
                error.kind(),
                "cannot open configuration file",
            )),
        }
    }

    /// Load an explicitly named file; a missing file is an error.
    pub fn from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::default().with_file(path)
    }

    /// Add file defaults to this snapshot. Existing process values take precedence.
    pub fn with_file(self, path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|e| file_error(path, e.kind(), "cannot open configuration file"))?;
        self.read_file(file, path)
    }

    /// Create an isolated snapshot for tests or explicitly supplied configuration.
    /// This does not read or modify process variables.
    pub fn from_values<K: Into<OsString>, V: Into<OsString>>(
        values: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        Self {
            process: values
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            file: HashMap::new(),
        }
    }

    fn read_file(mut self, file: File, path: &Path) -> io::Result<Self> {
        let mut values = HashMap::new();
        for item in dotenvy::from_read_iter(file) {
            let (name, value) = item.map_err(|error| {
                let kind = if let dotenvy::Error::Io(e) = error {
                    e.kind()
                } else {
                    io::ErrorKind::InvalidData
                };
                // Parser errors can contain the full line, including secrets.
                file_error(path, kind, "cannot parse configuration file")
            })?;
            validate_name(&name)?;
            if values.insert(name.clone(), value).is_some() {
                return Err(file_error(
                    path,
                    io::ErrorKind::InvalidData,
                    &format!("duplicate variable {name}"),
                ));
            }
        }
        self.file.extend(values);
        Ok(self)
    }

    /// Read a required typed value. Error messages identify the name, never value.
    pub fn get<T: FromStr>(&self, name: &str) -> io::Result<T> {
        self.optional(name)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("required environment variable {name} is missing"),
            )
        })
    }

    /// Read a typed value if present. Empty and invalid values are not treated as missing.
    pub fn optional<T: FromStr>(&self, name: &str) -> io::Result<Option<T>> {
        validate_name(name)?;
        let value = if let Some(value) = self.process.get(std::ffi::OsStr::new(name)) {
            Some(value.to_str().ok_or_else(|| invalid(name, "UTF-8 text"))?)
        } else {
            self.file.get(name).map(String::as_str)
        };
        value
            .map(|value| value.parse().map_err(|_| invalid(name, type_name::<T>())))
            .transpose()
    }

    /// Use a default only when the variable is absent, never when it is invalid.
    pub fn get_or<T: FromStr>(&self, name: &str, default: T) -> io::Result<T> {
        Ok(self.optional(name)?.unwrap_or(default))
    }
}
fn invalid(name: &str, expected: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("environment variable {name} must be valid {expected}"),
    )
}
fn validate_name(name: &str) -> io::Result<()> {
    let mut chars = name.chars();
    if !chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "environment variable names must be ASCII letters, digits and underscores, starting with a letter or underscore",
        ));
    }
    Ok(())
}
fn file_error(path: &Path, kind: io::ErrorKind, message: &str) -> io::Error {
    io::Error::new(kind, format!("{message}: {}", path.display()))
}
