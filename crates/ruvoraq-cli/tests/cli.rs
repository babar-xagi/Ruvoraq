use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ruvoraq-test-{}-{timestamp}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .expect("run the Ruvoraq CLI")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // The path was created exclusively by this test, under the OS temporary directory.
        fs::remove_dir_all(&self.0).expect("remove owned test directory");
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_error(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(stderr.contains(expected), "{stderr}");
}

fn entries(root: &Path) -> Vec<String> {
    fn walk(root: &Path, relative: &Path, result: &mut Vec<String>) {
        for entry in fs::read_dir(root.join(relative)).unwrap() {
            let entry = entry.unwrap();
            let path = relative.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                result.push(format!("{}/", path.to_string_lossy().replace('\\', "/")));
                walk(root, &path, result);
            } else {
                result.push(path.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut result = Vec::new();
    walk(root, Path::new(""), &mut result);
    result.sort();
    result
}

fn check_generated_project(temp: &TempDir, project: &Path) {
    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--manifest-path"])
        .arg(project.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(temp.0.join("build-output"))
        .current_dir(&temp.0)
        .output()
        .expect("cargo check the generated project");
    assert_success(&output);
}

#[test]
fn help_and_version() {
    let temp = TempDir::new();
    for args in [&[][..], &["--help"][..], &["-h"][..]] {
        let output = temp.run(args);
        assert_success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("new <project-name>"));
        assert!(output.stderr.is_empty());
    }
    for flag in ["--version", "-V"] {
        let output = temp.run(&[flag]);
        assert_success(&output);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("ruvoraq {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
    let output = temp.run(&["new", "--help"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Existing directories must be empty"));
    assert!(entries(&temp.0).is_empty());
}

#[test]
fn generates_exactly_three_files_and_project_compiles() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "hello-app"]));
    let project = temp.0.join("hello-app");
    assert_eq!(
        entries(&temp.0),
        [
            "hello-app/",
            "hello-app/Cargo.toml",
            "hello-app/src/",
            "hello-app/src/main.rs",
            "hello-app/src/settings.rs"
        ]
    );
    // Cargo itself may add a lockfile during checking, so inspect the initial scaffold first.
    check_generated_project(&temp, &project);
}

#[test]
fn accepts_empty_target_and_generates_compilable_underscore_name() {
    let temp = TempDir::new();
    let project = temp.0.join("_hello_2");
    fs::create_dir(&project).unwrap();
    assert_success(&temp.run(&["new", "_hello_2"]));
    assert_eq!(
        entries(&project),
        ["Cargo.toml", "src/", "src/main.rs", "src/settings.rs"]
    );
    check_generated_project(&temp, &project);
}

#[test]
fn generated_project_is_independent_of_surrounding_workspace() {
    let temp = TempDir::new();
    fs::write(
        temp.0.join("Cargo.toml"),
        "[workspace]\nmembers = []\nresolver = \"3\"\n",
    )
    .unwrap();
    assert_success(&temp.run(&["new", "nested-app"]));
    check_generated_project(&temp, &temp.0.join("nested-app"));
}

#[test]
fn refuses_nonempty_targets_without_changing_existing_content() {
    let temp = TempDir::new();
    for (name, sentinel) in [("existing-app", "keep.txt"), ("hidden-app", ".keep")] {
        let project = temp.0.join(name);
        fs::create_dir(&project).unwrap();
        fs::write(project.join(sentinel), "keep me").unwrap();
        assert_error(&temp.run(&["new", name]), "not empty");
        assert_eq!(entries(&project), [sentinel]);
        assert_eq!(
            fs::read_to_string(project.join(sentinel)).unwrap(),
            "keep me"
        );
    }
    assert_success(&temp.run(&["new", "generated-app"]));
    let project = temp.0.join("generated-app");
    let before = fs::read(project.join("Cargo.toml")).unwrap();
    assert_error(&temp.run(&["new", "generated-app"]), "not empty");
    assert_eq!(fs::read(project.join("Cargo.toml")).unwrap(), before);
    assert_eq!(
        entries(&project),
        ["Cargo.toml", "src/", "src/main.rs", "src/settings.rs"]
    );
}

#[test]
fn refuses_existing_file_target() {
    let temp = TempDir::new();
    fs::write(temp.0.join("file-app"), "keep me").unwrap();
    assert_error(&temp.run(&["new", "file-app"]), "not a regular directory");
    assert_eq!(
        fs::read_to_string(temp.0.join("file-app")).unwrap(),
        "keep me"
    );
}

#[test]
fn invalid_and_reserved_names_create_nothing() {
    let temp = TempDir::new();
    for name in [
        "",
        ".",
        "..",
        "../escape",
        "/absolute",
        "nested/app",
        "nested\\app",
        "1app",
        "-app",
        "bad name",
        "bad.name",
        "bad\"name",
        "café",
    ] {
        assert_error(&temp.run(&["new", name]), "invalid project name");
        assert!(entries(&temp.0).is_empty(), "name: {name}");
    }
    assert_error(&temp.run(&["new", &"a".repeat(65)]), "invalid project name");
    for name in [
        "_",
        "fn",
        "Self",
        "async",
        "gen",
        "try",
        "build",
        "deps",
        "examples",
        "incremental",
        "CON",
        "nul",
        "Com1",
        "lpt9",
    ] {
        assert_error(&temp.run(&["new", name]), "reserved");
        assert!(entries(&temp.0).is_empty(), "name: {name}");
    }
}

#[test]
fn malformed_commands_have_clear_errors_and_create_nothing() {
    let temp = TempDir::new();
    assert_error(&temp.run(&["new"]), "missing project name");
    assert_error(
        &temp.run(&["new", "app", "extra"]),
        "expected one project name",
    );
    for args in [
        &["unknown"][..],
        &["--unknown"][..],
        &["--help", "extra"][..],
    ] {
        assert_error(&temp.run(args), "unknown command or option");
    }
    assert!(entries(&temp.0).is_empty());
}

#[cfg(unix)]
#[test]
fn refuses_symlink_targets_without_touching_destination() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new();
    let destination = temp.0.join("destination");
    fs::create_dir(&destination).unwrap();
    symlink(&destination, temp.0.join("linked-app")).unwrap();
    assert_error(&temp.run(&["new", "linked-app"]), "not a regular directory");
    assert!(entries(&destination).is_empty());
    assert!(
        fs::symlink_metadata(temp.0.join("linked-app"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_arguments_without_panicking() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let temp = TempDir::new();
    let output = Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
        .current_dir(&temp.0)
        .arg("new")
        .arg(OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert_error(&output, "arguments must be valid UTF-8");
    assert!(entries(&temp.0).is_empty());
}
