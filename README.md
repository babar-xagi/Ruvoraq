# Ruvoraq

Ruvoraq is being developed through small experiments. Experiment 001 delivered
a protected three-file project generator. Experiment 002 added route attributes,
automatic registration, server startup, settings, graceful shutdown and a development
command. Experiment 003 added typed APIs and simple response helpers.
**Experiment 004 adds optional app modules** with `ruvoraq add app <name>`.
Small projects still start with exactly three files.

## Quick start

Rust 1.85 or newer is required (edition 2024). Install the CLI from this checkout:

```sh
cargo install --path crates/ruvoraq-cli --locked --force
ruvoraq --help
ruvoraq --version
ruvoraq new web-demo
cd web-demo
ruvoraq dev
```

After updating this checkout, rerun the `cargo install` command above.
Building or testing the workspace does not replace the CLI installed in
`~/.cargo/bin`.

Open http://127.0.0.1:8000/ or run `curl http://127.0.0.1:8000/`.
The response is `Hello`. Press Ctrl+C to stop.

`ruvoraq dev` runs Cargo with inherited terminal input/output and propagates its
exit status. It is a build-and-run command; file watching is not implemented.
Run it from the project's root. Projects are identified by
`[package.metadata.ruvoraq]` with `project = true`.
Set `CARGO` to override the Cargo executable if needed.

## Three-file applications

`new` still creates exactly three files and one source directory:

```text
web-demo/
├── Cargo.toml
└── src/
    ├── main.rs
    └── settings.rs
```

`main.rs` contains only your routes:

```rust
use ruvoraq::prelude::*;

#[get("/")]
async fn hello() -> &'static str {
    "Hello"
}
```

Add handlers with `#[get]`, `#[post]`, `#[put]`, `#[patch]`, or `#[delete]`,
each taking a path such as `#[get("/hello")]`. There is no manual route list,
settings import, runtime setup, or main function in this file.

`settings.rs` contains `APP_NAME`, `HOST`, `PORT`, a `settings()` function,
and `ruvoraq::bootstrap!();`:

```rust
use std::net::Ipv4Addr;

use ruvoraq::Settings;

pub const APP_NAME: &str = "web-demo";
pub const HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const PORT: u16 = 8000;

pub fn settings() -> Settings {
    Settings {
        app_name: APP_NAME.to_owned(),
        address: (HOST, PORT).into(),
    }
}

ruvoraq::bootstrap!();
```

Rust still requires a binary entry point. The manifest sets `autobins = false`
and points its single binary at `src/settings.rs`. `bootstrap!` generates the
entry point, loads `main.rs` as a handler module, applies settings and runs
the server. Both `cargo run` and `ruvoraq dev` work. No build script or fourth
project file is created.

Route attributes preserve callable async functions and emit typed registrations
collected with [inventory](https://docs.rs/inventory/0.3.24/). Registration does
not scan source files at runtime or depend on macro invocation order.
Routes are sorted before startup; duplicate method/path pairs and apps without
routes return errors before binding. Disabled `#[cfg]` handlers are not registered.

Edit host or port in settings.rs. Port 0 selects an available port; the startup
banner reports the actual bound address. The default binds only to 127.0.0.1:8000.

Ruvoraq is not published yet. The generated manifest uses an absolute path
dependency on the local `crates/ruvoraq` directory from which the CLI was built.
Keep that checkout available. If it moves, update the dependency path or reinstall
the CLI and regenerate a project. No registry package is assumed to exist.
Existing explicit-startup apps remain compatible and are not migrated automatically.
To migrate an existing app, add route attributes,
keep configuration and `bootstrap!` in settings.rs, and configure its binary target
as described above.

The generator does not run Cargo, initialize Git, or create a lockfile. Running
Cargo later may create `Cargo.lock` and `target/`.
An empty `[workspace]` keeps generated apps independent of surrounding workspaces.

Names are 1–64 ASCII letters, digits, hyphens or underscores, starting with a
letter or underscore. Rust keywords, Cargo artifact directory names and Windows
device names are rejected. Paths are not accepted. Existing empty directories
are accepted; non-empty directories (including hidden entries), files and
symlink targets are refused. Errors go to stderr with exit code 1. Files use
exclusive creation; failures attempt to remove only entries created by that call.

## Optional app modules (Experiment 004)

When your application grows, run this from its root:

```sh
ruvoraq add app school
ruvoraq dev
# In another terminal:
curl http://127.0.0.1:8000/school
```

The command creates:

```text
src/apps/
├── mod.rs
└── school/
    ├── mod.rs
    ├── routes.rs
    ├── models.rs
    └── services.rs
```

It appends `mod apps;` to settings.rs once and adds `pub mod school;` to
src/apps/mod.rs. Your existing settings, main.rs and Cargo.toml stay intact
apart from that settings declaration. Normal Rust modules compile the app's
routes, and existing route registration discovers them automatically.
There is no runtime folder scanning or separate route list.

The initial route returns:

```json
{"name":"school","message":"Hello from school"}
```

Routes handle HTTP, models describe data, and services hold application logic.
These are ordinary Rust files; no framework base classes or service macros are
required. Edit routes.rs to add typed handlers and use models/services as needed.
Add another module with `ruvoraq add app billing`; it gets `GET /billing`.
Route paths are explicit in attributes, so you can change them or add subpaths.

App names use 1–64 lowercase ASCII letters, digits and underscores, starting
with a letter or underscore. Rust keywords, Cargo build names, Windows device
names and paths are rejected. Existing target files/directories, duplicate
module declarations, malformed source and symlinked source paths are refused.
Comments and existing declarations are preserved. Run `cargo fmt` when you
want Rustfmt to sort module declarations.

The command requires the current route-only scaffold with a single binary at
src/settings.rs and `ruvoraq::bootstrap!();`. If you maintain custom inline,
conditional or alternate-path apps wiring, the CLI reports it and leaves it
for you to manage explicitly.

New files use exclusive creation. Existing wiring files are staged beside the
originals and replaced atomically, preserving file permissions. A failed add
attempts to undo completed writes and remove only its own entries; it reports
incomplete rollback rather than deleting edited files. This is not a
crash-recovery transaction.

No modules are added by `ruvoraq new`. Database, auth, AI, dependency injection
and automatic service construction remain future work.

## Typed APIs (Experiment 003)

Keep handlers and models in `main.rs`; startup stays in `settings.rs`:

```rust
use ruvoraq::prelude::*;

#[derive(Deserialize)]
struct CreateUser {
    name: String,
}

#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

#[post("/users")]
async fn create_user(Json(input): Json<CreateUser>) -> Reply<User> {
    created(User { id: 1, name: input.name })
}

#[get("/users/{id}")]
async fn get_user(Path(id): Path<u64>) -> User {
    User { id, name: "Ada".into() }
}
```

Route attributes automatically serialize returned models, collections and
`json!()` values to JSON with 200 OK. Return a model directly; no `Json(...)`
response wrapper is needed. `Result<Model>` returns the successful model as JSON
and uses the standard envelope for errors.

Common responses have simple names:

| Desired response | Handler code |
| --- | --- |
| 200 JSON | Return a model or `json!({...})` directly |
| 201 JSON | `created(value)` |
| 202 JSON | `accepted(value)` |
| 204 empty body | `no_content()` |
| 400 error | `Err(bad_request("Bad input"))` |
| 404 error | `Err(not_found("User not found"))` |
| 422 field error | `Err(invalid("name", "must not be blank"))` |

Use `-> Reply<User>` for a named JSON response, `-> Result<User>` for a model
that can fail, or `-> Result<Reply<User>>` when both apply.
For example:

```rust
#[get("/profile/{id}")]
async fn profile(Path(id): Path<u64>) -> Result<User> {
    if id == 0 {
        return Err(not_found("User not found"));
    }
    Ok(User { id, name: "Ada".into() })
}
```

`Json<T>` remains the typed JSON **input** extractor and an optional explicit
response wrapper. Strings stay plain text; explicit `IntoResponse` types,
status tuples and custom responses retain their behavior. Automatic model
conversion is provided by the route attributes; explicit `App` builder routes
can use `ok(value)`, `created(value)` or `Json(value)`.
Serialization errors from named JSON responses retain HTTP 500.

Model derives, helpers, `json!`, `Value`, `StatusCode` and the extractors are
available through the prelude.
New projects include Serde with its derive feature. Existing Experiment 002
projects using model derives should add
`serde = { version = "1", features = ["derive"] }` to their dependencies.

`Path<T>` supports a single value, tuples, or a deserializable struct matching
named parameters. `Query<T>` deserializes URL query fields; use Serde defaults
or `Option<T>` for optional fields. A `HeaderMap` handler argument reads headers.
Use a named helper for common response statuses. For a custom status,
`(StatusCode, Json(value))` and explicit `Result<Json<T>>` remain supported.

JSON schema checking follows Serde: required fields and field types are checked,
and `#[serde(deny_unknown_fields)]` can reject extra fields.
For application rules, implement `Validate` and use `ValidatedJson<T>`:

```rust
impl Validate for CreateUser {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(invalid("name", "must not be blank"));
        }
        Ok(())
    }
}
```

Replace `Json(input): Json<CreateUser>` with
`ValidatedJson(input): ValidatedJson<CreateUser>` in the handler.
A JSON body extractor must be the **last handler argument**.
Validation is explicit; there is no validation derive or generated API schema yet.

Put the typed handlers above in your generated application, then run:

```sh
ruvoraq dev
# In another terminal:
curl http://127.0.0.1:8000/
curl -i -H 'Content-Type: application/json' -d '{"name":"Ada"}' http://127.0.0.1:8000/users
curl http://127.0.0.1:8000/users/42
curl -i -H 'Content-Type: application/json' -d '{"name":" "}' http://127.0.0.1:8000/users
```

Generated applications use port 8000; change settings if the port is occupied.
These handlers return illustrative users and do not store records.
For a JSON greeting, change the root handler to return `Value` and
`json!({"Hello": "World"})`. The default scaffold still returns plain `Hello`.

Built-in extraction errors, Ruvoraq handler errors, missing routes and unsupported
methods share this envelope:

```json
{"error":{"code":"validation_error","message":"must not be blank","details":{"name":"must not be blank"}}}
```

| HTTP status | Error code / cause |
| --- | --- |
| 400 | `invalid_json`, `invalid_path`, `invalid_query`, or explicit `bad_request` |
| 404 | `not_found` |
| 405 | `method_not_allowed`; the `Allow` header is preserved |
| 413 | `payload_too_large`; JSON bodies default to a 2 MiB limit |
| 415 | `unsupported_media_type`; JSON content type required |
| 422 | `validation_error`; schema mismatch or application validation |
| 500 | `internal_error`; internal errors, request panics or JSON serialization failures |

All Ruvoraq 5xx error responses replace messages, codes and details with generic
values. Panics are caught when unwinding is enabled; Rust's panic hook can still
log to stderr. Custom raw responses or backend-specific extractors control their
own response format. `Error::new` supports custom client error codes and statuses;
non-error statuses become 500.

## Web core and architecture

The workspace contains four crates. Generated apps such as `examples/app` are independent workspaces:

| Crate | Responsibility |
| --- | --- |
| `ruvoraq` | Public facade, prelude, and async runtime entry point |
| `ruvoraq-macros` | Route attributes and settings bootstrap |
| `ruvoraq-web` | HTTP adapter, typed extraction, errors, routes and server lifecycle |
| `ruvoraq-cli` | Project generation and Cargo development launcher |

`App` supports `.get()`, `.post()`, `.put()`, `.patch()`, and `.delete()`.
Methods can share a route path. Axum supplies handler traits, response conversion
and HEAD for GET routes; Ruvoraq adds JSON fallbacks and panic handling through
`App::into_router()` and server startup.

Attribute paths start with `/` and support complete `{name}` segments.
Parameter names must be unique identifiers within a path, and methods sharing
a route pattern must use identical parameter names. Invalid attributes fail at
compile time; duplicate registrations and conflicting names for shared patterns
fail before binding. Query strings, fragments, wildcards and legacy `:name`
syntax are rejected in attributes. Explicit `App` builder routes use Axum's
registration rules and can panic for invalid paths.
Handler attributes require safe, non-generic async functions.
A backend-independent handler API remains future work.

The server drains active requests on Ctrl+C (Unix/Windows) or SIGTERM (Unix).
`App::serve` accepts an already-bound Tokio listener and a custom shutdown future
for tests and lifecycle integrations. A stuck handler can delay shutdown; a
forced-shutdown timeout is deferred.

There are no database, auth, AI, dependency-injection, OpenAPI,
validation-derive or hot-reload features in this experiment.

## Verification

```sh
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

CLI tests verify the exact initial scaffold, overwrite protection, project
recognition, launcher errors and exit codes, generated-project compilation,
optional module scaffolds, multiple apps, source preservation, invalid names,
custom-wiring refusals, symlink protection and rollback,
route-only source, model derives, typed handler compilation, useful compile errors,
duplicate/missing/conflicting routes, and automatic registration across modules
and conditional handlers. Typed API tests exercise JSON, path/query fields, headers,
named statuses, direct model/result responses, custom response compatibility,
validation, body limits, JSON 404/405, and suppression of internal error details. They cover router requests and typed requests over real HTTP.
On Unix, a generated application is launched through `ruvoraq dev`, queried over
HTTP (including an added module route), and stopped with both SIGINT and SIGTERM. Web tests use real local sockets
to check all five methods, responses, 404/405, bind errors, and draining an active
request. Generated-project checks run offline after workspace dependencies have
been fetched, sharing ignored build artifacts under
`target/generated-project-tests/`.

Licensed under Apache-2.0; see [LICENSE](LICENSE).
