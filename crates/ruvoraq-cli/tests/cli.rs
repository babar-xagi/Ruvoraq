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

    assert_success(&add_app(&project, &["add", "app", "school"]));
    let settings_with_service = fs::read_to_string(&settings)
        .unwrap()
        .replace("ruvoraq::bootstrap!();", "ruvoraq::bootstrap!(configure);");
    fs::write(&settings, format!("{settings_with_service}\npub struct Greeting(pub &'static str);\n\nfn configure(app: ruvoraq::App) -> ruvoraq::App {{\n    app.provide(Greeting(\"Hello from injected service\"))\n}}\n")).unwrap();
    let main_path = project.join("src/main.rs");
    let main_with_service = fs::read_to_string(&main_path).unwrap()
        + r#"
#[get("/injected")]
async fn injected(service: Inject<crate::Greeting>) -> Value {
    json!({"message":service.0.0})
}
"#;
    fs::write(main_path, main_with_service).unwrap();

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

        let mut stream = TcpStream::connect(address.unwrap()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(b"GET /school HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut module_response = String::new();
        stream.read_to_string(&mut module_response).unwrap();
        assert!(
            module_response.starts_with("HTTP/1.1 200 OK"),
            "{module_response}"
        );
        assert_eq!(
            module_response.split("\r\n\r\n").nth(1).unwrap(),
            r#"{"name":"school","message":"Hello from school"}"#
        );

        let mut stream = TcpStream::connect(address.unwrap()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(b"GET /injected HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut service_response = String::new();
        stream.read_to_string(&mut service_response).unwrap();
        assert!(
            service_response.starts_with("HTTP/1.1 200 OK"),
            "{service_response}"
        );
        assert_eq!(
            service_response.split("\r\n\r\n").nth(1).unwrap(),
            r#"{"message":"Hello from injected service"}"#
        );

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
        .current_dir(project)
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
            "#[get(\"/\", status = 404)]\nasync fn hello() -> &'static str { \"Hello\" }",
            "success status must be 200..299",
        ),
        (
            "#[get(\"/\", unknown = 201)]\nasync fn hello() -> &'static str { \"Hello\" }",
            "expected status = 201",
        ),
        (
            "#[get(\"/\")]\nfn hello() -> &'static str { \"Hello\" }",
            "route handlers must be async functions",
        ),
        (
            "#[get(\"relative\")]\nasync fn hello() -> &'static str { \"Hello\" }",
            "route paths must start with '/'",
        ),
        (
            "#[get(\"/users/{id\")]\nasync fn hello() -> &'static str { \"Hello\" }",
            "Ruvoraq path parameters must be unique identifiers in complete {name} segments",
        ),
        (
            "#[get(\"/users/{id}/{id}\")]\nasync fn hello() -> &'static str { \"Hello\" }",
            "Ruvoraq path parameters must be unique identifiers in complete {name} segments",
        ),
        (
            "#[post(\"/users/{id}\")]\nasync fn hello(Json(_): Json<Value>, Path(_): Path<u64>) {}",
            "Ruvoraq body extractors Json/ValidatedJson must be the last handler argument",
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

#[test]
fn generated_app_supports_model_derives_and_typed_handler_parameters() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "typed-generated"]));
    let project = temp.0.join("typed-generated");
    fs::write(
        project.join("src/main.rs"),
        r#"
use ruvoraq::prelude::*;

#[derive(Serialize, Deserialize)]
struct Model { name: String }

#[post("/models")]
async fn create(Json(model): Json<Model>) -> Reply<Model> {
    created(model)
}

type Output = Model;

#[get("/model")]
async fn single() -> Output {
    Model { name: "Ada".into() }
}

#[get("/model-result")]
async fn single_result() -> Result<Model> {
    Ok(Model { name: "Ada".into() })
}

#[derive(Deserialize)]
struct Filter { limit: u16 }

#[get("/models/{id}")]
async fn read(Path(id): Path<u64>, Query(filter): Query<Filter>) -> Value {
    json!({"id": id, "limit": filter.limit})
}
"#,
    )
    .unwrap();
    let manifest: toml::Table = fs::read_to_string(project.join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        manifest["dependencies"]["serde"]["features"]
            .as_array()
            .unwrap()[0]
            .as_str(),
        Some("derive")
    );
    assert_eq!(
        entries(&project),
        ["Cargo.toml", "src/", "src/main.rs", "src/settings.rs"]
    );
    check_generated_project(&temp, &project);
}

#[test]
fn incompatible_parameter_names_fail_before_binding() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "conflicting-patterns"]));
    let project = temp.0.join("conflicting-patterns");
    fs::write(
        project.join("src/main.rs"),
        r#"
use ruvoraq::prelude::*;
#[get("/users/{id}")]
async fn read() -> &'static str { "read" }
#[post("/users/{name}")]
async fn create() -> &'static str { "create" }
"#,
    )
    .unwrap();
    let output = cargo_for_project(&project, "run");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("conflicting Ruvoraq route patterns"),
        "{stderr}"
    );
}

fn add_app(project: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
        .current_dir(project)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn add_app_generates_modules_preserves_source_and_compiles_multiple_apps() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "modular"]));
    let project = temp.0.join("modular");
    let main = fs::read(project.join("src/main.rs")).unwrap();
    let manifest = fs::read(project.join("Cargo.toml")).unwrap();
    let settings = fs::read_to_string(project.join("src/settings.rs")).unwrap();
    assert_success(&add_app(&project, &["add", "app", "school"]));
    assert_eq!(
        entries(&project),
        [
            "Cargo.toml",
            "src/",
            "src/apps/",
            "src/apps/mod.rs",
            "src/apps/school/",
            "src/apps/school/mod.rs",
            "src/apps/school/models.rs",
            "src/apps/school/routes.rs",
            "src/apps/school/services.rs",
            "src/main.rs",
            "src/settings.rs",
        ]
    );
    assert_eq!(fs::read(project.join("src/main.rs")).unwrap(), main);
    assert_eq!(fs::read(project.join("Cargo.toml")).unwrap(), manifest);
    assert_eq!(
        fs::read_to_string(project.join("src/settings.rs")).unwrap(),
        format!("{}\nmod apps;\n", settings.trim_end())
    );
    let output = Command::new(env!("CARGO"))
        .args(["fmt", "--manifest-path"])
        .arg(project.join("Cargo.toml"))
        .arg("--check")
        .output()
        .unwrap();
    assert_success(&output);
    let registry = project.join("src/apps/mod.rs");
    fs::write(&registry, "// Keep this comment.\npub mod school;\n").unwrap();
    assert_success(&add_app(&project, &["add", "app", "billing"]));
    assert_eq!(
        fs::read_to_string(&registry).unwrap(),
        "// Keep this comment.\npub mod school;\npub mod billing;\n"
    );
    assert_eq!(
        fs::read_to_string(project.join("src/settings.rs"))
            .unwrap()
            .matches("mod apps;")
            .count(),
        1
    );
    let before = entries(&project);
    assert_error(
        &add_app(&project, &["add", "app", "school"]),
        "refusing to overwrite",
    );
    assert_eq!(entries(&project), before);
    check_generated_project(&temp, &project);
}

#[test]
fn add_app_help_names_and_project_checks_create_nothing() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["add", "--help"]));
    assert_success(&temp.run(&["add", "app", "--help"]));
    for args in [
        &["add"][..],
        &["add", "app"][..],
        &["add", "db"][..],
        &["add", "app", "school", "extra"][..],
    ] {
        assert_error(&temp.run(args), "usage: ruvoraq add app");
    }
    assert_success(&temp.run(&["new", "names"]));
    let project = temp.0.join("names");
    let before = entries(&project);
    for name in [
        "",
        "../escape",
        "a/b",
        "school-app",
        "School",
        "school app",
        "1school",
        "fn",
        "_",
        "nul",
        "gen",
    ] {
        let output = add_app(&project, &["add", "app", name]);
        assert!(!output.status.success(), "accepted {name}");
        assert!(output.stdout.is_empty());
        assert_eq!(entries(&project), before);
    }
    fs::write(
        temp.0.join("Cargo.toml"),
        "[package]\nname = \"ordinary\"\n",
    )
    .unwrap();
    assert_error(
        &temp.run(&["add", "app", "school"]),
        "not a Ruvoraq project",
    );
    assert!(!temp.0.join("src").exists());
}

#[test]
fn add_app_refuses_existing_paths_and_custom_module_wiring() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "guards"]));
    let project = temp.0.join("guards");
    let settings_path = project.join("src/settings.rs");
    let original = fs::read_to_string(&settings_path).unwrap();
    for suffix in [
        "\nmod apps {}\n",
        "\n#[path = \"custom.rs\"] mod apps;\n",
        "\n#[cfg(any())] mod apps;\n",
    ] {
        fs::write(&settings_path, format!("{original}{suffix}")).unwrap();
        assert_error(
            &add_app(&project, &["add", "app", "school"]),
            "custom apps wiring",
        );
        assert!(!project.join("src/apps").exists());
    }
    fs::write(&settings_path, &original).unwrap();
    fs::create_dir_all(project.join("src/apps/school")).unwrap();
    fs::write(project.join("src/apps/school/keep.txt"), "keep me").unwrap();
    assert_error(
        &add_app(&project, &["add", "app", "school"]),
        "refusing to overwrite",
    );
    assert_eq!(
        fs::read_to_string(project.join("src/apps/school/keep.txt")).unwrap(),
        "keep me"
    );
    fs::write(
        project.join("src/apps/mod.rs"),
        "// Already wired\npub mod billing;\n",
    )
    .unwrap();
    assert_error(
        &add_app(&project, &["add", "app", "billing"]),
        "already declared",
    );
    assert!(!project.join("src/apps/billing").exists());
    fs::write(project.join("src/apps/finance.rs"), "// Keep me").unwrap();
    assert_error(
        &add_app(&project, &["add", "app", "finance"]),
        "already exists",
    );
    assert_eq!(
        fs::read_to_string(project.join("src/apps/finance.rs")).unwrap(),
        "// Keep me"
    );
    assert!(!project.join("src/apps/finance").exists());
    fs::write(project.join("src/apps/mod.rs"), "mod [ broken").unwrap();
    assert_error(
        &add_app(&project, &["add", "app", "billing"]),
        "invalid Rust",
    );
    assert!(!project.join("src/apps/billing").exists());
    assert_eq!(fs::read_to_string(settings_path).unwrap(), original);
}

#[test]
fn add_app_rolls_back_created_files_if_settings_cannot_be_updated() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "rollback"]));
    let project = temp.0.join("rollback");
    let settings = project.join("src/settings.rs");
    let before = fs::read(&settings).unwrap();
    let permissions = fs::metadata(&settings).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&settings, readonly).unwrap();
    let output = add_app(&project, &["add", "app", "school"]);
    fs::set_permissions(&settings, permissions).unwrap();
    assert_error(&output, "read-only");
    assert!(!project.join("src/apps").exists());
    assert_eq!(fs::read(&settings).unwrap(), before);
    assert_eq!(
        entries(&project),
        ["Cargo.toml", "src/", "src/main.rs", "src/settings.rs"]
    );
}

#[cfg(unix)]
#[test]
fn add_app_refuses_symlink_module_directories() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "symlinks"]));
    let project = temp.0.join("symlinks");
    let destination = temp.0.join("destination");
    fs::create_dir(&destination).unwrap();
    symlink(&destination, project.join("src/apps")).unwrap();
    assert_error(
        &add_app(&project, &["add", "app", "school"]),
        "not a symlink",
    );
    assert!(entries(&destination).is_empty());
}

#[test]
fn bootstrap_configure_hook_registers_services_and_missing_providers_fail_before_startup() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "services-app"]));
    let project = temp.0.join("services-app");
    let settings_path = project.join("src/settings.rs");
    let original = fs::read_to_string(&settings_path).unwrap();
    let service = r#"
pub struct Greeting(pub &'static str);

fn configure(app: ruvoraq::App) -> ruvoraq::App {
    app.provide(Greeting("Hello from shared service"))
}
"#;
    fs::write(
        &settings_path,
        format!(
            "{}\n{service}",
            original.replace("ruvoraq::bootstrap!();", "ruvoraq::bootstrap!(configure);")
        ),
    )
    .unwrap();
    fs::write(
        project.join("src/main.rs"),
        r#"
use ruvoraq::prelude::*;
use crate::Greeting;

#[get("/")]
async fn hello(greeting: Inject<Greeting>) -> Value {
    json!({"message":greeting.0.0})
}
"#,
    )
    .unwrap();
    assert_success(&add_app(&project, &["add", "app", "school"]));
    check_generated_project(&temp, &project);
    // Keep the same route but remove the provider by using the default bootstrap.
    fs::write(
        &settings_path,
        original.clone() + "\npub struct Greeting(pub &'static str);\n",
    )
    .unwrap();
    let output = cargo_for_project(&project, "run");
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "startup banner must not be printed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("missing dependency") && stderr.contains("Greeting"),
        "{stderr}"
    );
    assert!(
        stderr.contains("App::provide") && stderr.contains("settings.rs"),
        "{stderr}"
    );
}

#[test]
fn generated_bootstrap_reports_configuration_errors_before_startup() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "configured-app"]));
    let project = temp.0.join("configured-app");
    for (content, name, secret) in [
        (
            "RUVORAQ_PORT=private-port\n",
            "RUVORAQ_PORT",
            "private-port",
        ),
        (
            "PASSWORD=\"private-password\n",
            "configuration file",
            "private-password",
        ),
    ] {
        fs::write(project.join(".env"), content).unwrap();
        let output = cargo_for_project(&project, "run");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(name), "{stderr}");
        assert!(!stderr.contains(secret), "{stderr}");
    }
    fs::remove_file(project.join(".env")).unwrap();
    let settings_path = project.join("src/settings.rs");
    let settings = fs::read_to_string(&settings_path).unwrap().replace(
        "ruvoraq::bootstrap!();",
        r#"
fn configure(app: ruvoraq::App) -> std::io::Result<ruvoraq::App> {
    let _limit: u16 = app.env().get("RUVORAQ_TEST_REQUIRED_LIMIT")?;
    Ok(app)
}
ruvoraq::bootstrap!(configure);
"#,
    );
    fs::write(settings_path, settings).unwrap();
    let output = cargo_for_project(&project, "run");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("RUVORAQ_TEST_REQUIRED_LIMIT"));
}

#[test]
fn migration_cli_applies_lists_and_validates_history() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "migration-demo"]));
    let project = temp.0.join("migration-demo");
    fs::create_dir(project.join("migrations")).unwrap();
    fs::write(
        project.join("migrations/1_create_values.sql"),
        "CREATE TABLE values_table (value INTEGER);",
    )
    .unwrap();
    fs::write(
        project.join(".env"),
        "DATABASE_URL=sqlite://values.sqlite\n",
    )
    .unwrap();
    let command = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
            .current_dir(&project)
            .env_remove("DATABASE_URL")
            .args(args)
            .output()
            .unwrap()
    };
    let status = command(&["migrate", "--status"]);
    assert_success(&status);
    assert!(String::from_utf8_lossy(&status.stdout).contains("1  pending  create values"));
    let output = command(&["migrate"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Applied 1 migration(s)."));
    let output = command(&["migrate"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Applied 0 migration(s)."));
    let status = command(&["migrate", "--status"]);
    assert_success(&status);
    assert!(String::from_utf8_lossy(&status.stdout).contains("1  applied"));
    fs::write(
        project.join("migrations/2_add_value.sql"),
        "INSERT INTO values_table VALUES (42);",
    )
    .unwrap();
    assert_success(&command(&["migrate"]));
    fs::write(
        project.join("migrations/1_create_values.sql"),
        "CREATE TABLE changed (value TEXT);",
    )
    .unwrap();
    assert_error(&command(&["migrate"]), "has changed");
    fs::write(
        project.join(".env"),
        "DATABASE_URL=mysql://private-secret\n",
    )
    .unwrap();
    let error = command(&["migrate"]);
    assert_error(&error, "sqlite: scheme");
    assert!(!String::from_utf8_lossy(&error.stderr).contains("private-secret"));
}

#[test]
fn migration_cli_checks_project_environment_and_sources_before_opening_database() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["migrate", "--help"]));
    assert_error(&temp.run(&["migrate", "unexpected"]), "usage:");
    assert_error(&temp.run(&["migrate"]), "project root");
    assert_success(&temp.run(&["new", "migration-demo"]));
    let project = temp.0.join("migration-demo");
    let command = || {
        Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
            .current_dir(&project)
            .env_remove("DATABASE_URL")
            .arg("migrate")
            .output()
            .unwrap()
    };
    assert_error(&command(), "DATABASE_URL");
    fs::write(project.join(".env"), "DATABASE_URL=sqlite://never.sqlite\n").unwrap();
    assert_error(&command(), "migrations directory");
    assert!(!project.join("never.sqlite").exists());
    fs::create_dir(project.join("migrations")).unwrap();
    fs::write(project.join("migrations/malformed.sql"), "SELECT 1;").unwrap();
    assert_error(&command(), "filenames");
    assert!(!project.join("never.sqlite").exists());
}

#[cfg(not(feature = "postgres"))]
#[test]
fn postgres_migrations_explain_the_optional_cli_feature_without_exposing_urls() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "postgres-demo"]));
    let project = temp.0.join("postgres-demo");
    fs::create_dir(project.join("migrations")).unwrap();
    fs::write(
        project.join("migrations/1_notes.sql"),
        "CREATE TABLE notes (title TEXT);",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ruvoraq"))
        .current_dir(project)
        .env("DATABASE_URL", "postgres://private-secret")
        .arg("migrate")
        .output()
        .unwrap();
    assert_error(&output, "--features postgres");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-secret"));
}

#[test]
fn migration_cli_uses_configured_directory_and_process_override() {
    let temp = TempDir::new();
    assert_success(&temp.run(&["new", "migration-path"]));
    let project = temp.0.join("migration-path");
    fs::create_dir_all(project.join("schema/sqlite")).unwrap();
    fs::write(
        project.join("schema/sqlite/1_values.sql"),
        "CREATE TABLE values_table (id INTEGER);",
    )
    .unwrap();
    fs::write(
        project.join(".env"),
        "DATABASE_URL=sqlite://values.sqlite\nMIGRATIONS_DIR=missing\n",
    )
    .unwrap();
    let command = |override_path: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ruvoraq"));
        command
            .current_dir(&project)
            .env_remove("DATABASE_URL")
            .env_remove("MIGRATIONS_DIR");
        if let Some(path) = override_path {
            command.env("MIGRATIONS_DIR", path);
        }
        command.args(["migrate", "--status"]).output().unwrap()
    };
    assert_error(&command(None), "migrations directory");
    assert!(!project.join("values.sqlite").exists());
    let output = command(Some("schema/sqlite"));
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("1  pending"));
}
