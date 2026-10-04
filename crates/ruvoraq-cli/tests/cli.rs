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

fn generated_target_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/generated-project-tests")
}

fn check_generated_project(temp: &TempDir, project: &Path) {
    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--manifest-path"])
        .arg(project.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(generated_target_dir())
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

#[test]
fn generated_manifest_identifies_project_and_uses_local_framework() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "metadata-app"]));
    let manifest = fs::read_to_string(temp.0.join("metadata-app/Cargo.toml")).unwrap();
    let manifest: toml::Table = manifest.parse().unwrap();
    assert_eq!(
        manifest["package"]["metadata"]["ruvoraq"]["project"].as_bool(),
        Some(true)
    );
    let dependency = Path::new(
        manifest["dependencies"]["ruvoraq"]["path"]
            .as_str()
            .unwrap(),
    );
    assert!(dependency.is_absolute());
    assert_eq!(
        dependency,
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ruvoraq")
            .canonicalize()
            .unwrap()
    );
}

#[test]
fn dev_help_and_errors_do_not_launch_unmarked_projects() {
    let temp = TempDir::new();
    let help = temp.run(&["dev", "--help"]);
    assert_success(&help);
    assert!(String::from_utf8_lossy(&help.stdout).contains("File watching is not implemented"));
    assert!(entries(&temp.0).is_empty());
    assert_error(&temp.run(&["dev", "extra"]), "unexpected arguments");
    assert_error(
        &temp.run(&["dev"]),
        "run 'ruvoraq dev' from a Ruvoraq project",
    );

    fs::write(temp.0.join("Cargo.toml"), "package = [").unwrap();
    assert_error(&temp.run(&["dev"]), "invalid Cargo.toml");

    for manifest in [
        "[package]\nname = \"ordinary\"\nversion = \"0.1.0\"\n",
        "[package.metadata.ruvoraq]\nproject = false\n",
        "[package.metadata.ruvoraq]\nproject = \"true\"\n",
        "# [package.metadata.ruvoraq]\n# project = true\n",
    ] {
        fs::write(temp.0.join("Cargo.toml"), manifest).unwrap();
        assert_error(&temp.run(&["dev"]), "not a Ruvoraq project");
        assert_eq!(
            fs::read_to_string(temp.0.join("Cargo.toml")).unwrap(),
            manifest
        );
    }
}

#[test]
fn dev_reports_missing_cargo() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "missing-cargo"]));
    let output = Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
        .current_dir(temp.0.join("missing-cargo"))
        .arg("dev")
        .env("CARGO", temp.0.join("nonexistent-cargo"))
        .output()
        .unwrap();
    assert_error(&output, "cannot launch Cargo");
}

#[cfg(unix)]
#[test]
fn dev_passes_manifest_and_preserves_cargo_output_and_exit_status() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "cargo-failure"]));
    let project = temp.0.join("cargo-failure");
    let fake_cargo = temp.0.join("fake-cargo");
    fs::write(
        &fake_cargo,
        "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\"\nprintf 'build failed\\n' >&2\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&fake_cargo, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
        .current_dir(&project)
        .arg("dev")
        .env("CARGO", &fake_cargo)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "{}\nrun\n--manifest-path\n{}\n",
            project.display(),
            project.join("Cargo.toml").display()
        )
    );
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "build failed\n");
}

#[cfg(unix)]
#[test]
fn generated_app_runs_through_dev_and_stops_on_interrupt_and_termination() {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::{SocketAddr, TcpStream},
        os::unix::process::CommandExt,
        process::{Child, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    // Keep this test's process group isolated from Cargo and the test runner.
    struct ServerProcess(Child);
    impl Drop for ServerProcess {
        fn drop(&mut self) {
            if self.0.try_wait().unwrap().is_none() {
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", self.0.id())])
                    .status();
                let _ = self.0.wait();
            }
        }
    }

    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "web-smoke"]));
    let project = temp.0.join("web-smoke");
    assert_eq!(
        entries(&project),
        ["Cargo.toml", "src/", "src/main.rs", "src/settings.rs"]
    );
    let settings = project.join("src/settings.rs");
    let original = fs::read_to_string(&settings).unwrap();
    assert!(original.contains("pub const PORT: u16 = 8000;"));
    fs::write(
        &settings,
        original.replace("pub const PORT: u16 = 8000;", "pub const PORT: u16 = 0;"),
    )
    .unwrap();

    for signal in ["INT", "TERM"] {
        let log_path = temp.0.join(format!("dev-{signal}.log"));
        let log = fs::File::create(&log_path).unwrap();
        let mut child = ServerProcess(
            Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
                .arg("dev")
                .current_dir(&project)
                .env("CARGO", env!("CARGO"))
                .env("CARGO_NET_OFFLINE", "true")
                .env("CARGO_TARGET_DIR", generated_target_dir())
                .process_group(0)
                .stdout(Stdio::piped())
                .stderr(Stdio::from(log))
                .spawn()
                .unwrap(),
        );
        let stdout = child.0.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });

        let mut address: Option<SocketAddr> = None;
        loop {
            let line = receiver
                .recv_timeout(Duration::from_secs(60))
                .unwrap_or_else(|error| {
                    panic!(
                        "waiting for server readiness: {error}; stderr: {}",
                        fs::read_to_string(&log_path).unwrap()
                    )
                });
            if let Some(value) = line.strip_prefix("Server:      http://") {
                address = Some(value.parse().unwrap());
            }
            if line == "Ready" {
                break;
            }
        }
        let mut stream =
            TcpStream::connect(address.expect("server reported its bound address")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert_eq!(response.split("\r\n\r\n").nth(1).unwrap(), "Hello");

        assert_success(
            &Command::new("kill")
                .args([&format!("-{signal}"), "--", &format!("-{}", child.0.id())])
                .output()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "server did not stop on SIG{signal}"
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert!(
            status.success(),
            "SIG{signal}: {status}; stderr: {}",
            fs::read_to_string(&log_path).unwrap()
        );
        reader.join().unwrap();
        assert!(
            receiver.try_iter().any(|line| line == "Stopped"),
            "server did not finish graceful shutdown"
        );
    }
}

#[test]
fn generated_main_contains_only_the_requested_route_and_import() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "route-only"]));
    let project = temp.0.join("route-only");
    assert_eq!(
        fs::read_to_string(project.join("src/main.rs")).unwrap(),
        "use ruvoraq::prelude::*;\n\n#[get(\"/\")]\nasync fn hello() -> &'static str {\n    \"Hello\"\n}\n"
    );
    let manifest: toml::Table = fs::read_to_string(project.join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(manifest["package"]["autobins"].as_bool(), Some(false));
    let binaries = manifest["bin"].as_array().unwrap();
    assert_eq!(binaries.len(), 1);
    assert_eq!(binaries[0]["path"].as_str(), Some("src/settings.rs"));
    assert!(
        fs::read_to_string(project.join("src/settings.rs"))
            .unwrap()
            .contains("ruvoraq::bootstrap!();")
    );
    assert_eq!(
        entries(&project),
        ["Cargo.toml", "src/", "src/main.rs", "src/settings.rs"]
    );
}

fn cargo_for_project(project: &Path, command: &str) -> Output {
    Command::new(env!("CARGO"))
        .args([command, "--offline", "--manifest-path"])
        .arg(project.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(generated_target_dir())
        .output()
        .unwrap()
}

#[test]
fn invalid_route_attributes_fail_at_compile_time_with_useful_diagnostics() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "invalid-routes"]));
    let project = temp.0.join("invalid-routes");
    for (handler, diagnostic) in [
        (
            "#[get(\"/\")]\nfn hello() -> &'static str { \"Hello\" }",
            "route handlers must be async functions",
        ),
        (
            "#[get(\"relative\")]\nasync fn hello() -> &'static str { \"Hello\" }",
            "route paths must be static paths starting with '/'",
        ),
        (
            "#[get(\"/users/{id}\")]\nasync fn hello() -> &'static str { \"Hello\" }",
            "route paths must be static paths starting with '/'",
        ),
        (
            "#[get(\"/\")]\nasync fn hello<T>() -> &'static str { \"Hello\" }",
            "route handlers must be non-generic, safe, free async functions",
        ),
        (
            "#[get(\"/\")]\nasync unsafe fn hello() -> &'static str { \"Hello\" }",
            "route handlers must be non-generic, safe, free async functions",
        ),
    ] {
        fs::write(
            project.join("src/main.rs"),
            format!("use ruvoraq::prelude::*;\n{handler}\n"),
        )
        .unwrap();
        let output = cargo_for_project(&project, "check");
        assert!(
            !output.status.success(),
            "invalid handler unexpectedly compiled: {handler}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(diagnostic), "{stderr}");
        assert!(stderr.contains("src/main.rs"), "{stderr}");
    }
}

#[test]
fn duplicate_and_missing_routes_fail_before_binding_a_server() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "route-errors"]));
    let project = temp.0.join("route-errors");
    for (source, diagnostic) in [
        (
            "use ruvoraq::prelude::*;\n#[get(\"/\")]\nasync fn first() -> &'static str { \"one\" }\n#[get(\"/\")]\nasync fn second() -> &'static str { \"two\" }\n",
            "duplicate Ruvoraq route: GET /",
        ),
        (
            "// No routes in this application.\n",
            "no Ruvoraq routes registered",
        ),
    ] {
        fs::write(project.join("src/main.rs"), source).unwrap();
        let output = cargo_for_project(&project, "run");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(diagnostic), "{stderr}");
        assert!(
            output.stdout.is_empty(),
            "a server banner was emitted despite invalid routes"
        );
    }
}
