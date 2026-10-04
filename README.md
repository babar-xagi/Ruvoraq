# Ruvoraq

Ruvoraq is being developed through small experiments. Experiment 001 delivered
a protected three-file project generator. Experiment 002 added route attributes,
automatic registration, server startup, settings, graceful shutdown and a development
command. Experiment 003 added typed APIs and simple response helpers.
Experiment 004 added optional app modules with `ruvoraq add app <name>`.
Experiment 005 added shared services and typed injection with `Inject<T>`.
Experiment 006 added automatic OpenAPI and interactive /docs.
Experiment 007 added typed environment configuration and optional .env loading.
**Experiment 008 adds optional SQLite persistence and async configuration hooks.**
Small projects still start with exactly three files.

## Documentation

- 📘 [User guide](doc/user_guid.md): setup, routes, configuration, services, API docs, and SQLite.
- 🛠️ [Developer guide](doc/developer_guid.md): architecture, contribution workflow, and verification.
- 🧭 [Project tracker](project.md): completed experiments, local work, remaining features, and future goals.

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

## SQLite persistence (Experiment 008)

Database support is opt-in. The default three-file app and examples/app do not
enable SQLite. Enable it only in an application's Cargo.toml:

```toml
[dependencies.ruvoraq]
path = "/home/xagi/Ruvoraq/crates/ruvoraq"
features = ["sqlite"]
```

Database and the SQLx adapter are then available from the prelude. Construct
and inject the pool in an async settings hook:

```rust
use ruvoraq::{App, Database};

async fn configure(app: App) -> std::io::Result<App> {
    let url: String = app.env().get("DATABASE_URL")?;
    let db = Database::connect(&url).await?;
    Ok(app.provide(db))
}
ruvoraq::bootstrap!(async configure);
```

Existing synchronous and fallible hooks stay compatible. Configuration and
database initialization finish before the server binds. A file URL such as
sqlite://notes.sqlite creates a missing file; its parent directory must exist.
Use sqlite::memory: for an ephemeral database.

Handlers use Inject<Database> and parameterized queries with bind():

```rust
use ruvoraq::prelude::*;

#[get("/count")]
async fn count(db: ruvoraq::Inject<ruvoraq::Database>) -> ruvoraq::Result<ruvoraq::Value> {
    let (count,): (i64,) = ruvoraq::sqlx::query_as("SELECT COUNT(*) FROM notes")
        .fetch_one(db.pool()).await.map_err(|_| ruvoraq::Error::internal())?;
    Ok(ruvoraq::json!({"count": count}))
}
```

The example initializes its notes table in settings.rs. Do not concatenate
request values into SQL. The initial SQLite pool uses one connection, so memory
databases remain consistent and concurrent writes queue through the same pool.
Foreign keys are enabled, and connection acquisition / busy waits are bounded.
Database::begin() returns a SQLx transaction with commit(), rollback(), and
rollback on drop. Database::close() closes the shared pool; cloning a Database
shares the existing pool.

The separate examples/sqlite-api example provides persistent note CRUD, schema
documentation and an async configure hook. Data survives server restarts.
Its CREATE TABLE IF NOT EXISTS is initial schema setup, not a versioned migration
system. PostgreSQL, an ORM, model generators, migrations and automatic database
shutdown hooks are deferred. SQLx 0.8.6 is used for compatibility with this
workspace's Rust version; driver APIs remain available as an explicit escape hatch.
Connection errors redact URLs; map query errors to Error::internal() before
returning them from HTTP handlers, as the example does.

```sh
cd examples/sqlite-api
ruvoraq dev
# Test in the browser at http://127.0.0.1:8000/docs
# Or run the repeatable test:
python3 tests/smoke.py
```

The live test reuses HTTP/process helpers from examples/app/tests/smoke.py.
SQLite files and journals are ignored by Git. No database is created by new,
and no existing example has been replaced.

## Configuration (Experiment 007)

The bootstrap loads configuration before startup. Precedence is:

```text
process environment > local .env > settings.rs defaults
```

No changes to main.rs are needed. The generator still creates exactly three
files; .env is optional and is never generated automatically.

| Variable | Type | Default |
| --- | --- | --- |
| RUVORAQ_APP_NAME | Non-empty text without control characters | APP_NAME in settings.rs |
| RUVORAQ_HOST | IPv4 or IPv6 address, without a port | HOST in settings.rs |
| RUVORAQ_PORT | Integer 0–65535 | PORT in settings.rs |
| RUVORAQ_DOCS | true or false | true |

For example:

```sh
RUVORAQ_PORT=9000 ruvoraq dev
RUVORAQ_DOCS=false ruvoraq dev
```

Or create .env in the application directory:

```dotenv
RUVORAQ_APP_NAME="My API"
RUVORAQ_HOST=127.0.0.1
RUVORAQ_PORT=9000
RUVORAQ_DOCS=true
```

Only the current directory's .env is loaded; parent directories are not searched.
Missing files are allowed. Invalid syntax, duplicate names and unreadable files
fail before the server starts. Parsing uses dotenvy for comments, quoting, export
and interpolation; substitution follows dotenvy's rules at load time.
Loading does not call set_var or change the global process environment.
Boolean and numeric parsing is strict; invalid or empty typed values never
silently fall back to defaults. Port 0 still requests an available local port.

Use the same snapshot for custom configuration in settings.rs:

```rust
fn configure(app: ruvoraq::App) -> std::io::Result<ruvoraq::App> {
    let greeting = app.env().get_or("SCHOOL_GREETING", "School API".to_owned())?;
    // Construct this service as usual in your optional school module.
    Ok(app.provide(apps::school::services::SchoolService::with_greeting(greeting)))
}
ruvoraq::bootstrap!(configure);
```

The example application's SchoolService implements with_greeting(). The configure
hook can return either App (existing code stays compatible) or io::Result<App>.
It runs after environment overrides; explicit changes in the hook are final.

The prelude exports Env with typed methods:

```rust
let env = ruvoraq::Env::load()?;
let required: String = env.get("DATABASE_URL")?;
let optional: Option<u16> = env.optional("CUSTOM_PORT")?;
let retries: usize = env.get_or("RETRIES", 3)?;
```

These variables are examples of custom configuration, not database integration.
A bootstrapped app also provides Inject<Env> automatically when handlers need
configuration. Prefer constructing services in settings.rs for application logic.
Env is a snapshot: changing a file or process variable later does not update an
already running app.

Errors report the variable name and expected type without printing the supplied
value or parser input. Env's Debug output contains only entry counts; it does
not print variable names or values. Applications can still deliberately log values.

Explicit App builders opt in with
`App::new().settings(defaults).environment(Env::load()?)?`.
Settings::with_env(&env) applies just the identity/address overrides.
Env::from_file(path) requires a named file, load_from(directory) reads an optional
.env in that exact directory, and from_values(...) creates an isolated snapshot
for tests. None of these methods mutates process variables.

In the repository example, copy examples/app/.env.example to examples/app/.env
only when you want file-based settings. Actual .env files are ignored by Git;
.env.example files can be tracked. New independent projects should configure
their own ignore rules. Profiles, automatic config-struct derives, live reloading
and database integration are deferred.

## Interactive API docs (Experiment 006)

Every attribute-based app serves **/docs** (Swagger UI with "Try it out") and
**/openapi.json** (OpenAPI 3.1). Start `ruvoraq dev`, then open
http://127.0.0.1:8000/docs. JavaScript and CSS are bundled locally, so the page
works without a CDN or an online schema validator.

![Swagger UI showing a successful 201 Created API response](docs/images/swagger-ui.jpg)

A student created through Swagger UI using **Try it out**, with the live
201 response and JSON body shown above.

Routes, methods, path captures, supported typed input/output and module groups
are collected automatically. Rust doc comments become operation summaries.
Keep main.rs route-only; documentation startup stays inside the framework.

Add `#[schema]` to request, response and query models for field schemas:

```rust
use ruvoraq::prelude::*;

#[schema]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateUser {
    name: String,
}

#[schema]
#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

/// Create a user.
#[post("/users", status = 201)]
async fn create_user(Json(input): Json<CreateUser>) -> Reply<User> {
    created(User { id: 1, name: input.name })
}
```

No additional application dependency is needed for `#[schema]`. Serde names,
optional fields, defaults, nested models and recursive models are supported.
Custom `Validate` logic still runs normally; document constraints separately
with Schemars field attributes such as `#[schemars(length(min = 1, max = 80))]`
when appropriate. Schemas describe fields; they do not replace validation.

The optional `status = 201` argument is documentation metadata: keep it aligned
with `created()`, `accepted()`, `no_content()` or the actual returned status.
It does not override response/error handling. Plain JSON/text responses default
to documented 200. Reply, opaque/custom dynamic responses and status tuples
use a generic success response unless a status is declared.

Models without `#[schema]` keep working, with an explicitly unavailable schema.
This first version recognizes the standard Path/Query/Json/ValidatedJson and
Result/Json/Reply names in signatures; custom extractors and aliases do not yet
provide full field metadata. HeaderMap keys cannot be inferred automatically.
Explicit `App::get(...)` builder routes are served but are not documented;
use route attributes for automatic docs.

Disable the built-in endpoints in settings.rs when needed:

```rust
fn configure(app: ruvoraq::App) -> ruvoraq::App {
    app.docs(false)
}
ruvoraq::bootstrap!(configure);
```

The /docs, /openapi.json and two asset paths are reserved while docs are enabled.
Startup reports conflicts clearly. Vendored Swagger UI 5.11.0 retains its Apache
license in crates/ruvoraq-web/src/swagger/LICENSE. To update it, replace both
assets and the matching license from the same swagger-ui-dist release, then
rerun the documentation and browser tests.

## Shared services (Experiment 005)

Create a service once in settings.rs and receive it in handlers with `Inject<T>`.
Requests share the same instance; the service itself does not need `Clone`.
This supports shared configuration, counters, clients and application services.

Here is an opt-in example using the `school` module from `ruvoraq add app school`.
In src/apps/school/services.rs:

```rust
pub struct SchoolService {
    pub message: &'static str,
}
```

Expose the service to settings.rs by changing the services declaration in
src/apps/school/mod.rs to `pub mod services;` (keep models/routes declarations).

In src/apps/school/routes.rs:

```rust
use ruvoraq::prelude::*;
use super::{models::AppInfo, services::SchoolService};

#[get("/school")]
async fn index(service: Inject<SchoolService>) -> AppInfo {
    AppInfo {
        name: "school",
        message: service.message,
    }
}
```

In settings.rs, keep the existing settings function and `mod apps;`.
Add this configure function and **replace** the existing bootstrap invocation:

```rust
fn configure(app: ruvoraq::App) -> ruvoraq::App {
    app.provide(apps::school::services::SchoolService {
        message: "Hello from shared service",
    })
}

ruvoraq::bootstrap!(configure);
```

Start with `ruvoraq dev`, then request `http://127.0.0.1:8000/school`.
main.rs needs no change. Existing apps using `ruvoraq::bootstrap!();` continue
working; the configure hook is optional.

Services can have synchronous or async methods; `Inject<T>` dereferences to
`T`, so handlers call them normally: `service.method().await`.
For JSON input, put `Inject<T>` before the body extractor.

`App::provide(value)` owns and shares a concrete `Send + Sync + 'static` type.
Use `App::provide_shared(arc)` to share an existing `Arc<T>`, such as test state.
Registering the same type again replaces its previous provider. Separate apps
have separate registries. Use interior mutability (for example, atomics or an
appropriate mutex) for mutable state; there is no global service registry.

Route attributes record required service types, including aliases and qualified
`Inject<T>` paths. Startup validates them before `App::run` binds a socket.
A missing provider names the service and route and tells you to register it in
settings.rs. `App::check()` runs the same validation without starting a server.

Explicit `App::get(...)` builder routes and standalone `into_router()` use
runtime extraction. A missing service produces the generic JSON 500 envelope;
call `check()` first when using a standalone router with attribute registrations.
The normal bootstrap/run/serve path checks attribute dependencies automatically.

Providers are explicitly constructed, and the configure hook is synchronous.
Automatic constructor graphs, request-scoped providers, async provider factories
and trait-based service resolution are deferred. Database, auth and AI are not
part of this experiment.

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

No modules are added by `ruvoraq new`. Database, auth, AI and automatic service
construction remain future work. Typed shared services are opt-in as described above.

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
Validation is explicit; there is no validation derive. Add #[schema] for generated
API field schemas as described in Experiment 006.

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

The workspace contains six crates. Generated apps such as `examples/app` are independent workspaces:

| Crate | Responsibility |
| --- | --- |
| `ruvoraq` | Public facade, prelude, and async runtime entry point |
| `ruvoraq-macros` | Route attributes and settings bootstrap |
| `ruvoraq-web` | HTTP adapter, typed extraction, errors, routes and server lifecycle |
| `ruvoraq-cli` | Project generation and Cargo development launcher |
| `ruvoraq-config` | Typed configuration snapshots and optional dotenv parsing |
| `ruvoraq-db` | Optional SQLite pool and SQLx adapter |

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

Authentication, AI, automatic dependency construction, validation derives, and
hot reload remain future work. OpenAPI and optional SQLite persistence are now
implemented; see the documentation guides and project tracker.

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
validation, body limits, JSON 404/405, and suppression of internal error details.
Shared-state tests verify concurrent requests, app isolation, provider replacement,
non-Clone services, type aliases, async service methods and missing dependencies.
They cover router requests and typed requests over real HTTP.
On Unix, a generated application is launched through `ruvoraq dev`, queried over
HTTP (including module and injected-service routes), and stopped with both SIGINT and SIGTERM. Web tests use real local sockets
to check all five methods, responses, 404/405, bind errors, and draining an active
request. Generated-project checks run offline after workspace dependencies have
been fetched, sharing ignored build artifacts under
`target/generated-project-tests/`.

Licensed under Apache-2.0; see [LICENSE](LICENSE).
