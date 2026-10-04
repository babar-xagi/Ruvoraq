# Ruvoraq user guide

Ruvoraq is an experimental Rust backend framework built around small applications,
route attributes, and optional features. This guide describes the implementation
through Experiment 010. Examples build on the generated scaffold; when combining
configuration, services and databases, extend one configure hook and retain one
bootstrap invocation. All six packages are available on crates.io as version 0.1.0. A framework
checkout is optional for application development. See the
[publishing guide](publishing_guid.md) for the verified release workflow.

## Contents

1. [Installation and environment setup](#installation-and-environment-setup)
2. [Start an application](#start-an-application)
3. [Routes and responses](#routes-and-responses)
4. [Typed requests and validation](#typed-requests-and-validation)
5. [Configuration](#configuration)
6. [Shared services](#shared-services)
7. [Application modules](#application-modules)
8. [OpenAPI and Swagger UI](#openapi-and-swagger-ui)
9. [SQLite persistence](#sqlite-persistence)
10. [Versioned SQLite migrations](#versioned-sqlite-migrations)
11. [Optional PostgreSQL](#optional-postgresql)
12. [Examples and troubleshooting](#examples-and-troubleshooting)
13. [Development workflow and release builds](#development-workflow-and-release-builds)
14. [Current limitations](#current-limitations)

## Installation and environment setup

### Requirements

| Component | Required for |
| --- | --- |
| Rust and Cargo | Compiling the framework and your application. |
| Git | Cloning and updating the framework checkout. |
| Native build tools | Linking Rust binaries and compiling native dependencies such as bundled SQLite. |
| curl | Installing rustup and sending manual HTTP requests. |
| Python 3.11+ | Running the comprehensive example test runner. |
| Docker | The disposable PostgreSQL environment used by the full live suite. |
| PostgreSQL server | Running your own PostgreSQL application; SQLite needs no server. |

Linux/Ubuntu in WSL2 is the verified development environment. Native Windows
and macOS have not received equivalent end-to-end verification. The live scripts
use Unix process signals and should be run in Linux/WSL2.

The workspace uses Rust edition 2024 and declares Rust 1.85 as its minimum.
Current checks used Rust/Cargo 1.99.0. The complete all-feature workspace has also passed Rust 1.85.0; use a current stable toolchain for this walkthrough.

### Windows: prepare WSL2

If you already use Ubuntu in WSL2, skip to the Ubuntu setup below.
Otherwise, open **PowerShell as Administrator**:

```powershell
wsl --install -d Ubuntu
```

Restart Windows if prompted. Open Ubuntu, finish its first-run Linux user setup,
then verify the distribution from PowerShell:

```powershell
wsl --list --verbose
```

The Ubuntu entry should show version `2`. If your existing Ubuntu uses WSL1,
convert that distribution with `wsl --set-version Ubuntu 2`.
These steps follow [Microsoft's WSL installation guide](https://learn.microsoft.com/en-us/windows/wsl/install).
All remaining Bash commands belong in the Ubuntu/WSL terminal.

### Ubuntu / Debian: install build tools

```bash
sudo apt update
sudo apt install -y build-essential pkg-config git curl ca-certificates
```

For the optional Python-based test suites:

```bash
sudo apt install -y python3
python3 --version
```

The full runner uses Python 3.11 or newer. Your distribution's Python version
must meet that requirement. SQLite support compiles the bundled SQLite library;
you do not need to provision a separate SQLite service.

### Install Rust

Install Rust inside WSL rather than relying on a Windows Rust installation.
The [official Rust installation page](https://rust-lang.org/tools/install/)
provides the rustup command:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

Follow the installer prompts. For an existing rustup installation, update the
stable toolchain when appropriate with `rustup update stable`.
Formatting and lint tools used during development can be installed with:

```bash
rustup component add rustfmt clippy
```

If a command is unavailable after installation, reopen the terminal or source
`$HOME/.cargo/env`. Rust and installed Cargo binaries normally live in
`$HOME/.cargo/bin`.

### Install the CLI from crates.io

```bash
cargo install ruvoraq-cli --version 0.1.0 --locked
ruvoraq --help
ruvoraq --version
```

No Git checkout is needed to generate and run an application. For PostgreSQL
migration commands, use:

```bash
cargo install ruvoraq-cli --version 0.1.0 --features postgres --locked --force
```

This enables PostgreSQL in the migration CLI; application dependencies still
need their own database feature. Packages: [framework](https://crates.io/crates/ruvoraq)
and [CLI](https://crates.io/crates/ruvoraq-cli).

### Source development installation

Keep the framework checkout in your Linux home directory:

```bash
cd "$HOME"
git clone https://github.com/babar-xagi/Ruvoraq.git
cd Ruvoraq
cargo install --path crates/ruvoraq-cli --locked
ruvoraq --help
ruvoraq --version
```

If you already cloned the repository, enter that checkout instead of cloning
over it. The current version output is `ruvoraq 0.1.0`.
The source installation is for contributors and local framework changes.
By default this CLI also generates registry dependencies. To use your checkout,
set RUVORAQ_FRAMEWORK_PATH to its crates/ruvoraq directory while generating.

For PostgreSQL CLI migration support:

```bash
cargo install --path crates/ruvoraq-cli --features postgres --locked --force
```

The CLI feature enables PostgreSQL **migration commands**. Application database
dependencies still need their own `sqlite` or `postgres` feature.

### Updating your installation

For the current published release, reinstall the registry CLI with `--force`:

```bash
cargo install ruvoraq-cli --version 0.1.0 --locked --force
# Or retain PostgreSQL support:
cargo install ruvoraq-cli --version 0.1.0 --features postgres --locked --force
```

Choose one command. Future releases require choosing their published version.
Existing applications retain the dependency version recorded in Cargo.toml and
Cargo.lock; reinstalling the CLI does not rewrite their manifests.

For a source installation, follow this separate workflow:

From your framework checkout, update with a fast-forward pull and reinstall
the CLI you use:

```bash
git pull --ff-only
cargo install --path crates/ruvoraq-cli --locked --force
# Or keep PostgreSQL CLI support:
cargo install --path crates/ruvoraq-cli --features postgres --locked --force
```

Choose one installation command. Reinstalling without `--features postgres`
replaces the binary with the default SQLite-only migration CLI. If you have
local framework changes, resolve them before updating the checkout.

Building the workspace does not replace the installed CLI. `--force` replaces
an existing installation; it does not overwrite generated application projects.
Projects generated with RUVORAQ_FRAMEWORK_PATH use this checkout, so source
updates affect those projects when they next build. Registry projects resolve
their declared versions instead. Version `0.1.0` alone does not identify
the exact installed experiment; reinstall from the intended Git revision.

## Start an application

With the CLI installed, create an application in a directory you own:

```bash
mkdir -p "$HOME/projects"
cd "$HOME/projects"
ruvoraq new hello_api
cd hello_api
cargo check
ruvoraq dev
```

The first build downloads and compiles dependencies. In a second terminal,
`curl -i http://127.0.0.1:8000/` returns HTTP 200 and `Hello`.
Use port 8000 unless you override it with RUVORAQ_PORT.

Visit http://127.0.0.1:8000/docs for interactive API testing. Press Ctrl+C
to stop the server before restarting after source changes. For installation
updates, use [the CLI update workflow](#updating-your-installation).

### What is generated?

Immediately after `new`, the project contains exactly three files:

```text
hello_api/
├── Cargo.toml
└── src/
    ├── main.rs
    └── settings.rs
```

Cargo creates Cargo.lock and target/ when you build. Additional models, modules,
environment files, and database files are added only when you need them.

The generated main.rs is deliberately small:

```rust
use ruvoraq::prelude::*;

#[get("/")]
async fn hello() -> &'static str {
    "Hello"
}
```

The generated settings.rs contains the application name, listening address,
settings function, and `ruvoraq::bootstrap!();`.

Cargo uses settings.rs as the binary entry point. The bootstrap macro supplies
the Rust main function and includes main.rs as the route module. Do not add a
second main function or bootstrap invocation.

The manifest marks the project with `[package.metadata.ruvoraq]` and
`project = true`. It also contains an independent `[workspace]` declaration,
so generated applications can live inside the framework checkout. Its Ruvoraq
dependency uses the CLI version from crates.io. Source developers can set
RUVORAQ_FRAMEWORK_PATH to add an explicit local dependency path. Moving an app
with a path dependency requires updating it; normal registry projects are portable.

### Available commands

| Command | Behavior |
| --- | --- |
| `ruvoraq --help` | Show supported commands. |
| `ruvoraq --version` | Show the installed CLI version. |
| `ruvoraq new <project-name>` | Create the protected three-file application. |
| `ruvoraq migrate [--status]` | Apply or inspect migrations; PostgreSQL requires the CLI postgres feature. |
| `ruvoraq dev` | Build and run the current marked application. |
| `ruvoraq add app <module-name>` | Add and wire an optional application module. |

Run `dev` and `add app` from the application's root. Development mode uses
Cargo, inherits terminal output, and propagates its exit status. It does not
watch files or automatically restart the server. The CARGO environment variable
can select a different Cargo executable.

Project names are 1–64 ASCII characters: letters, digits, hyphens, and
underscores, beginning with a letter or underscore. Reserved Rust keywords,
Cargo artifact names, and Windows device names are rejected. A project name
cannot be a filesystem path.

An existing empty directory is accepted. An existing non-empty directory,
including hidden entries, is refused; files and symlink targets are refused.
Errors appear on stderr with a nonzero exit status.

## Routes and responses

The attributes `#[get]`, `#[post]`, `#[put]`, `#[patch]`, and `#[delete]`
register async handlers automatically. Paths start with / and use whole-segment
captures such as `/users/{id}`.

Add these handlers to main.rs alongside the initial greeting:

```rust
#[get("/health")]
async fn health() -> Value {
    json!({"status": "ready"})
}

#[post("/requests", status = 202)]
async fn submit() -> Reply<Value> {
    accepted(json!({"state": "accepted"}))
}

#[delete("/demo", status = 204)]
async fn remove_demo() -> Response {
    no_content()
}
```

The submission example only returns a response; it does not enqueue work.
A plain string is a text response. Serializable models and JSON values returned
through attributed handlers become JSON responses. Native response types and
custom IntoResponse implementations retain their own behavior.

| Helper | HTTP status | Typical use |
| --- | --- | --- |
| `ok(value)` | 200 | Explicit successful JSON response. |
| `created(value)` | 201 | A resource was created. |
| `accepted(value)` | 202 | A request was accepted. |
| `no_content()` | 204 | Success with no response body. |
| `bad_request(message)` | 400 | Application-level invalid request. |
| `not_found(message)` | 404 | A requested resource is missing. |
| `invalid(field, message)` | 422 | A field failed application validation. |

Use `Result<Model>` when a JSON model can fail, or
`Result<Reply<Model>>` when it can fail and needs a named success status:

```rust
#[get("/items/{id}")]
async fn item(Path(id): Path<u64>) -> Result<Value> {
    if id != 1 {
        return Err(not_found("item not found"));
    }
    Ok(json!({"id": id, "name": "Example"}))
}
```

**The attribute's status argument documents the response; it does not change
the runtime status.** Pair `status = 201` with `created(...)`, for example.

For manual App route registration, return an explicit response wrapper such as
Json or Reply for arbitrary models. Automatic model conversion belongs to the
route attributes.

## Typed requests and validation

Use Path for URL captures, Query for query parameters, Json for deserialization,
and ValidatedJson when a model implements Validate. Put a JSON body extractor
last in the handler's arguments.

To keep main.rs focused on handlers, add src/models.rs when models are needed
and add `mod models;` in settings.rs. This is an intentional extension beyond
the initial three-file scaffold.

Example models.rs:

```rust
use ruvoraq::prelude::*;

#[schema]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUser {
    pub name: String,
}

impl Validate for CreateUser {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(invalid("name", "name is required"));
        }
        Ok(())
    }
}

#[schema]
#[derive(Serialize)]
pub struct User {
    pub id: u64,
    pub name: String,
}
```

Add this import and route in main.rs:

```rust
use super::models::{CreateUser, User};

#[post("/users", status = 201)]
async fn create_user(ValidatedJson(input): ValidatedJson<CreateUser>) -> Reply<User> {
    created(User {
        id: 1,
        name: input.name.trim().to_owned(),
    })
}
```

This teaching example returns a model without storing it. The SQLite example
below demonstrates persistence.

Try it:

```sh
curl -i http://127.0.0.1:8000/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Ada"}'
```

The response is HTTP 201 with `{"id":1,"name":"Ada"}`. An empty name returns
422. Plain Json checks JSON syntax and Serde deserialization, but does not call
Validate. Use Serde defaults and Option fields for optional query parameters;
use deny_unknown_fields when extra JSON fields should be rejected.

HeaderMap exposes incoming headers. The school example demonstrates reading
x-request-id and adding it to a response.

### Path parameters, query defaults and headers

Add these handlers to a generated `src/main.rs`, keeping its existing
`use ruvoraq::prelude::*;` import:

```rust
#[get("/products/{id}")]
async fn product(Path(id): Path<u64>) -> Value {
    json!({"id": id})
}

#[schema]
#[derive(Deserialize)]
struct Search {
    q: Option<String>,
    #[serde(default = "default_limit")]
    limit: u16,
}

fn default_limit() -> u16 {
    10
}

#[get("/search")]
async fn search(Query(input): Query<Search>) -> Result<Value> {
    if !(1..=100).contains(&input.limit) {
        return Err(invalid("limit", "Limit must be between 1 and 100"));
    }
    Ok(json!({"query": input.q, "limit": input.limit}))
}

#[get("/request-info")]
async fn request_info(headers: HeaderMap) -> Value {
    let request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok());
    json!({"request_id": request_id})
}
```

Restart the server and try these in a second terminal:

```bash
curl -i http://127.0.0.1:8000/products/42
# 200: {"id":42}
curl -i http://127.0.0.1:8000/products/invalid
# 400: invalid_path
curl -i 'http://127.0.0.1:8000/search?q=rust&limit=5'
# 200: {"query":"rust","limit":5}
curl -i http://127.0.0.1:8000/search
# 200: {"query":null,"limit":10}
curl -i 'http://127.0.0.1:8000/search?limit=0'
# 422: validation_error
curl -i http://127.0.0.1:8000/request-info -H 'x-request-id: example-123'
# 200: {"request_id":"example-123"}
```

Query deserialization supplies the default; application code checks the allowed
range. Reading a header through HeaderMap does not add that header to OpenAPI
automatically.

### Error responses

Framework errors use a consistent envelope:

```json
{
  "error": {
    "code": "validation_error",
    "message": "name is required",
    "details": {
      "name": "name is required"
    }
  }
}
```

| Situation | Status |
| --- | --- |
| Malformed JSON, path, or query value | 400 |
| Missing route or resource | 404 |
| Unsupported method | 405, with an Allow header |
| JSON body over the default 2 MiB limit | 413 |
| Unsupported JSON content type | 415 |
| JSON field/type mismatch or application validation failure | 422 |
| Internal failure | 500 |

Framework 5xx errors return a generic internal error without private details.
Custom raw responses and third-party extractors may have their own error format.
Validation rules are explicit Rust code; no validation derive is implemented.

## Configuration

Keep startup and configuration in settings.rs. The generated defaults use
localhost and port 8000. Bootstrap loads an optional .env file from the current
working directory and applies these variables:

| Variable | Meaning |
| --- | --- |
| RUVORAQ_APP_NAME | Non-empty application name without control characters. |
| RUVORAQ_HOST | IPv4 or IPv6 address; hostname lookup is not supported. |
| RUVORAQ_PORT | Integer from 0 to 65535; 0 chooses an available port. |
| RUVORAQ_DOCS | Strict true or false, controlling documentation endpoints. |

Example .env:

```dotenv
RUVORAQ_APP_NAME=hello_api
RUVORAQ_HOST=127.0.0.1
RUVORAQ_PORT=8000
RUVORAQ_DOCS=true
SCHOOL_GREETING=Welcome to school
```

Database workflows also use these application/CLI keys:

| Variable | Behavior |
| --- | --- |
| DATABASE_URL | Required by migrate; application configure hooks choose whether a fallback is appropriate. |
| MIGRATIONS_DIR | Migration CLI defaults to migrations; a startup hook must read the same key to share its folder choice. |

These keys do not enable a database automatically. Enable a database feature
and initialize/register its provider in the configure hook.

Process environment values take precedence over file values. A missing optional
.env file is accepted, but malformed content and duplicate file keys are errors.
The snapshot does not mutate the process environment and does not reload live.
The framework checkout ignores .env files; check your application's own Git
ignore rules before saving secrets. Commit an .env.example containing safe
placeholders if useful.

For application-specific configuration, replace the existing bootstrap line
with `ruvoraq::bootstrap!(configure);` and define a hook:

```rust
fn configure(app: ruvoraq::App) -> std::io::Result<ruvoraq::App> {
    let greeting = app.env().get_or("SCHOOL_GREETING", "School API".to_owned())?;
    Ok(app.provide(greeting))
}
```

Env offers `get::<T>` for required values, `optional::<T>` for optional values,
and `get_or::<T>` for defaults when a key is absent. Invalid present values
produce errors instead of silently using the default. Parsing diagnostics omit
raw values. Strings retain their original contents, including empty strings.

The configure hook runs after built-in environment settings are applied. A
manual App builder can opt in with `app.environment(Env::load()?)?`.

## Shared services

Register a service in the settings configure hook with
`app.provide(service)`, or `app.provide_shared(arc)` when an Arc already exists.
Use Inject<Service> in a route to receive shared access.

For the String provider above:

```rust
#[get("/greeting")]
async fn greeting(message: Inject<String>) -> String {
    message.as_str().to_owned()
}
```

Services must be Send + Sync + 'static, but need not implement Clone. The
registry belongs to each App instance. Registering the same concrete type again
replaces its previous value, so use distinct service types for distinct roles.

Attributed handlers' required services are checked before the server binds.
Missing registration produces a startup error. There is no automatic
constructor graph or request-scoped dependency system yet.

For mutable shared state, use a suitable synchronization primitive. The school
example uses a Mutex for student records and an atomic counter for visits.
Avoid holding a standard mutex guard across an await.

## Application modules

From a generated application's root:

```sh
ruvoraq add app school
```

The generator creates and connects:

```text
src/apps/
├── mod.rs
└── school/
    ├── mod.rs
    ├── models.rs
    ├── routes.rs
    └── services.rs
```

It adds the module declaration in settings.rs. Route attributes in routes.rs
participate in automatic registration, and the generated /school endpoint
returns application information. Module names use lowercase ASCII letters,
digits, and underscores, starting with a letter or underscore.

Existing modules are refused. The generator conservatively rejects ambiguous
custom wiring, such as inline or conditional apps modules. It does not rewrite
your main.rs or add database behavior.

The generated services module starts private. Expose it with
`pub mod services;` in the module's mod.rs when settings.rs must construct its
service.

## OpenAPI and Swagger UI

Documentation is available by default:

- /docs: Swagger UI for viewing and sending API requests.
- /openapi.json: the generated OpenAPI 3.1 document.
- Swagger JavaScript and CSS: served locally, without a CDN.

Add `#[schema]` to request, response, and query models. Rust doc comments on
handlers supply descriptions. Routes in application modules are grouped by
module. Known Path, Query, Json, ValidatedJson, and response wrappers contribute
parameters, bodies, and response schemas.

The docs cannot infer arbitrary custom extractors, HeaderMap keys, every type
alias, or custom validation rules. Dynamic response statuses need explicit
documentation metadata. Explicit App builder routes are not automatically
documented.

The /docs and /openapi.json paths, including Swagger asset paths, are reserved.
A conflict fails startup. Disable these endpoints with RUVORAQ_DOCS=false or
App::docs(false) when you need those paths.

![Swagger UI showing the school API](../docs/images/swagger-ui.jpg)

## SQLite persistence

SQLite support is optional. Add the feature to your application's existing
Ruvoraq dependency; preserve its existing version or source-development path:

```toml
[dependencies.ruvoraq]
version = "0.1.0"
features = ["sqlite"]
```

The feature exports Database and sqlx. Replace the bootstrap invocation with
`ruvoraq::bootstrap!(async configure);` for asynchronous initialization:

```rust
async fn configure(app: ruvoraq::App) -> std::io::Result<ruvoraq::App> {
    let url = app.env().get_or("DATABASE_URL", "sqlite://notes.sqlite".to_owned())?;
    let database = ruvoraq::Database::connect(&url).await?;
    Ok(app.provide(database))
}
```

The parent directory must already exist. The database file is created when
needed. sqlite::memory: is useful for isolated tests. The current pool uses one
connection, enables foreign keys, and uses five-second busy/acquire timeouts.

Handlers receive `Inject<Database>` and execute parameterized SQL:

```rust
let row: Option<(i64, String)> =
    sqlx::query_as("SELECT id, title FROM notes WHERE id = ?")
        .bind(id)
        .fetch_optional(database.pool())
        .await
        .map_err(|_| Error::internal())?;
```

This fragment assumes the notes table has already been initialized. Never build
SQL by interpolating request values into the query string. Use bind for values.

Database::begin creates a transaction; commit saves it, rollback cancels it,
and dropping an uncommitted transaction rolls it back. Database clones share
the pool, so closing one closes the shared pool.

The [complete SQLite example](../examples/app/README.md) contains schema
initialization, validated CRUD routes, and persistence tests. It uses numbered SQL files and
startup migration checks. PostgreSQL is available through a separate opt-in
adapter below. An ORM, reversible migrations, migration-file generators, and
database scaffolding remain future work.

### Complete SQLite setup in a new application

This walkthrough creates a separate `notes_api` application. Use a new project
name if that directory already exists:

```bash
mkdir -p "$HOME/projects"
cd "$HOME/projects"
ruvoraq new notes_api
cd notes_api
```

Add `features = ["sqlite"]` under the existing `[dependencies.ruvoraq]` section
in `Cargo.toml`; preserve its generated version. For source development, keep
the local path as well. Then create the first SQL
migration and an optional environment file:

```bash
mkdir migrations
cat > migrations/0001_create_notes.sql <<'SQL'
CREATE TABLE notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 120)
);
SQL
cat > .env <<'ENV'
DATABASE_URL=sqlite://notes.sqlite
MIGRATIONS_DIR=migrations
RUVORAQ_PORT=8000
ENV
```

Replace `src/settings.rs` with this complete startup file:

```rust
use std::net::Ipv4Addr;
use ruvoraq::{App, Database, Settings};

pub const APP_NAME: &str = "notes_api";
pub const HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const PORT: u16 = 8000;

pub fn settings() -> Settings {
    Settings {
        app_name: APP_NAME.to_owned(),
        address: (HOST, PORT).into(),
    }
}

async fn configure(app: App) -> std::io::Result<App> {
    let url: String = app.env().get("DATABASE_URL")?;
    let directory = app.env().get_or("MIGRATIONS_DIR", "migrations".to_owned())?;
    let database = Database::connect(&url).await?;
    database.migrate(directory).await?;
    Ok(app.provide(database))
}

ruvoraq::bootstrap!(async configure);
```

Replace `src/main.rs` with this complete minimal notes API:

```rust
use ruvoraq::prelude::*;

#[schema]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateNote {
    title: String,
}

impl Validate for CreateNote {
    fn validate(&self) -> Result<()> {
        let length = self.title.trim().chars().count();
        if !(1..=120).contains(&length) {
            return Err(invalid("title", "Title must contain 1–120 characters"));
        }
        Ok(())
    }
}

#[schema]
#[derive(Serialize)]
struct Note {
    id: i64,
    title: String,
}

#[get("/notes")]
async fn notes(database: Inject<Database>) -> Result<Vec<Note>> {
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, title FROM notes ORDER BY id")
        .fetch_all(database.pool())
        .await
        .map_err(|_| Error::internal())?;
    Ok(rows.into_iter().map(|(id, title)| Note { id, title }).collect())
}

#[post("/notes", status = 201)]
async fn create_note(
    database: Inject<Database>,
    ValidatedJson(input): ValidatedJson<CreateNote>,
) -> Result<Reply<Note>> {
    let (id, title): (i64, String) =
        sqlx::query_as("INSERT INTO notes (title) VALUES (?) RETURNING id, title")
            .bind(input.title.trim())
            .fetch_one(database.pool())
            .await
            .map_err(|_| Error::internal())?;
    Ok(created(Note { id, title }))
}
```

Check, apply and start:

```bash
cargo check
ruvoraq migrate --status
ruvoraq migrate
ruvoraq migrate
ruvoraq dev
```

The first apply reports one migration; the repeat reports zero. In a second
terminal:

```bash
curl -i http://127.0.0.1:8000/notes
# 200: [] on a fresh database
curl -i http://127.0.0.1:8000/notes \
  -H 'Content-Type: application/json' -d '{"title":"My first note"}'
# 201: {"id":1,"title":"My first note"}
curl -i http://127.0.0.1:8000/notes
```

Stop and restart the server; the note remains. Open `/docs` to send the same
requests through Swagger UI. Read/update/delete handlers and full migration
failure tests are available in the [comprehensive example](../examples/app/README.md).

## Versioned SQLite migrations

Experiment 009 adds forward-only migrations. Create a migrations/ directory in
your application's root and add files with positive numeric versions:

```text
migrations/
├── 0001_create_notes.sql
└── 0002_notes_title_index.sql
```

Descriptions use lowercase ASCII letters, digits, and underscores. Versions
must be unique; files run in numeric order, not alphabetical order.
A first migration might contain:

```sql
CREATE TABLE notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL
);
```

A later file can add an index:

```sql
CREATE INDEX notes_title_index ON notes(title);
```

Set DATABASE_URL in the process environment or the application's .env:

```dotenv
DATABASE_URL=sqlite://notes.sqlite
```

From the marked application root:

```sh
ruvoraq migrate --status
ruvoraq migrate
ruvoraq migrate --status
```

Status lists each version as pending or applied. It validates history without
running migration SQL or creating the history table. Opening the SQLite
connection may create a missing database file. The CLI uses its own SQLite
adapter and does not compile your application or run its configure hook.

For automatic startup migrations, add this line after connecting Database in
your existing async configure hook:

```rust
database.migrate("migrations").await?;
```

Do not add a second bootstrap invocation. Enable the sqlite application feature
when using this API. The three-file generator remains unchanged; migrations/
is created deliberately when persistence needs it.

SQLx stores versions and checksums in _sqlx_migrations. Repeated runs skip applied
files. Changed or missing applied files, duplicate versions, malformed SQL file
names, and newly inserted older versions fail before pending SQL is applied.
Non-SQL files are ignored. SQL symlink entries, empty files, and reversible
.up.sql/.down.sql files are refused.

Each file runs in its own transaction. If a file fails, its ordinary transactional
SQL changes roll back while earlier successful files remain committed. Fix the
unapplied file and retry. Keep applied files unchanged; put subsequent changes
in a new, higher version. Do not edit database history to bypass checks.

The runner owns transaction boundaries. Scripts must not contain BEGIN, COMMIT,
ROLLBACK, or other transaction-control statements; non-transactional directives
are unsupported. Run one SQLite migrator at a time; the SQLite adapter has no
cross-process migration coordinator. PostgreSQL uses advisory locking as
described below.

The CLI reads MIGRATIONS_DIR from the process environment or .env and defaults
to migrations. For startup to use the same folder, read that key in the configure
hook as shown in the complete SQLite example. The comprehensive example selects
backend-specific migrations/sqlite or migrations/postgres folders.

Startup loads files relative to the current working directory. Ship migrations/
with the application and run from the intended project root. A missing or empty
directory is an error. The CLI requires DATABASE_URL explicitly, even if an
application hook has its own fallback.

The notes example's first migration uses CREATE TABLE IF NOT EXISTS to preserve
the known Experiment 008 notes schema. This is not general schema detection or
an automatic baseline for arbitrary legacy databases.

## Optional PostgreSQL

Experiment 010 adds an explicit PostgreSQL adapter. Enable it in your
application's existing Ruvoraq dependency:

```toml
[dependencies.ruvoraq]
version = "0.1.0"
features = ["postgres"]
```

Use PostgresDatabase for PostgreSQL and Database for SQLite. Both provide
pool(), begin(), close(), migrate(), and migration_status(). Either can be
registered with App::provide and extracted through Inject<T>.

In your existing async configure hook:

```rust
async fn configure(app: ruvoraq::App) -> std::io::Result<ruvoraq::App> {
    let url: String = app.env().get("DATABASE_URL")?;
    let database = ruvoraq::PostgresDatabase::connect(&url).await?;
    database.migrate("migrations").await?;
    Ok(app.provide(database))
}
```

Replace the existing bootstrap invocation with
`ruvoraq::bootstrap!(async configure);`. Provision the database separately and
set DATABASE_URL to a postgres:// or postgresql:// URL. The adapter does not
create a PostgreSQL database or account.

PostgreSQL uses its own SQL syntax. For a bound query:

```rust
let row: Option<(i64, String)> =
    sqlx::query_as("SELECT id, title FROM notes WHERE id = $1")
        .bind(id)
        .fetch_optional(database.pool())
        .await
        .map_err(|_| Error::internal())?;
```

This fragment assumes an Inject<PostgresDatabase>, an id, and an initialized
notes table. Keep PostgreSQL migrations in the PostgreSQL application's
migrations/ folder; the SQLite examples' SQL is not converted automatically.

To use the same migrate commands with PostgreSQL, install the optional CLI
feature from the framework root:

```sh
cargo install --path crates/ruvoraq-cli --features postgres --locked --force
```

Then, from the PostgreSQL application root:

```sh
ruvoraq migrate --status
ruvoraq migrate
ruvoraq dev
```

The URL scheme selects the CLI backend. An installed CLI without the postgres
feature gives a clear reinstall instruction without exposing the URL.

The PostgreSQL pool permits five connections, with five-second acquisition
and PostgreSQL lock timeouts. Migration runs hold a PostgreSQL advisory lock
across history validation and per-file transactions. The migration connection
is closed after success or failure to release session locks.

Connection diagnostics omit the URL and underlying driver details. Native
SQLx query errors still need mapping to Error::internal in HTTP handlers.

TLS support is provided by SQLx/Rustls, with policy selected through connection
options. Testing used a local PostgreSQL 17 server without TLS and verified that
requiring TLS fails against it. Certificate-verified TLS connections remain
unverified in this phase.

See the [complete PostgreSQL example](../examples/app/README.md) for
local database setup, CRUD, migrations, and its 27-check live suite. The
consolidated example defaults to port 8000. PostgreSQL support remains optional; a default generated application
has no SQLx dependency.

### Provision a local development database with Docker

If Docker is already available in WSL, check `docker info` before proceeding.
This creates a separate PostgreSQL development container. From your application
root, create `.postgres.env` (ignore it as shown in the Git workflow section):

```dotenv
POSTGRES_USER=ruvoraq_dev
POSTGRES_DB=ruvoraq_dev
POSTGRES_PASSWORD=change-this-local-password
```

Choose your own development password, then run:

```bash
chmod 600 .postgres.env
docker run -d --name ruvoraq-guide-postgres \
  --env-file .postgres.env \
  -p 127.0.0.1:5433:5432 \
  postgres:17-alpine
docker exec ruvoraq-guide-postgres \
  pg_isready -h 127.0.0.1 -U ruvoraq_dev -d ruvoraq_dev
```

Wait until the TCP readiness check reports accepting connections. Port 5433
avoids the usual local PostgreSQL port. The container must have a new name;
reuse or stop your existing development container deliberately if one exists.

For the repository's comprehensive example, select PostgreSQL explicitly:

```bash
cd "$HOME/Ruvoraq/examples/app"
export DATABASE_URL='postgres://ruvoraq_dev:change-this-local-password@127.0.0.1:5433/ruvoraq_dev'
export MIGRATIONS_DIR=migrations/postgres
ruvoraq migrate --status
ruvoraq migrate
cargo run --no-default-features --features postgres
```

Use the actual password from your environment file. URL-encode reserved
characters if they appear in a connection URL. The example's `.env` may select
SQLite; these process exports override it. Its application-level `postgres`
feature selects PostgreSQL handlers. In a new application that directly enables
the facade dependency's `postgres` feature, the earlier `ruvoraq dev` command
still works without this example-specific feature selector.

The notes endpoints and Swagger UI use the same URLs as the SQLite example.
PostgreSQL has separate records and migration history. After stopping the app,
clear the overrides before returning to SQLite:

```bash
unset DATABASE_URL MIGRATIONS_DIR
```

To pause this development database, use `docker stop ruvoraq-guide-postgres`;
resume it with `docker start ruvoraq-guide-postgres`. When you are finished and
want to discard its container-local database, run
`docker rm -f ruvoraq-guide-postgres`. This example does not configure a named
data volume. For disposable automated testing, use the full runner instead of
maintaining this manual container.

## Examples and troubleshooting

One comprehensive application now combines the school/billing API and notes
database examples. SQLite is the default; PostgreSQL is an explicit feature.
Use [the complete testing guide](testing_guid.md) for a fresh project, manual
HTTP commands, expected responses, configuration and both database backends.

```sh
cd "$HOME/Ruvoraq/examples/app"
cp -n .env.example .env
ruvoraq migrate --status
ruvoraq migrate
ruvoraq dev
# From the framework root, run all automated live checks:
cd "$HOME/Ruvoraq"
python3 examples/app/tests/full_test.py --postgres
```

MIGRATIONS_DIR selects the migration folder. The CLI defaults to migrations;
this example's .env selects migrations/sqlite. PostgreSQL uses
migrations/postgres and `cargo run --no-default-features --features postgres`.
Process environment variables override .env for this setting too.

| Issue | What to check |
| --- | --- |
| Cargo or ruvoraq is not found | Source "$HOME/.cargo/env" or reopen the WSL terminal; verify the CLI installation. |
| Linker cc is not found | Install the build-essential package inside WSL/Ubuntu. |
| CLI still generates an old template | Reinstall crates/ruvoraq-cli with --locked --force. |
| Port is already in use | Stop the other server or set RUVORAQ_PORT. |
| Project marker is missing | Run dev inside a generated application's root. |
| Missing service at startup | Register the exact Inject<T> type in configure. |
| JSON request is rejected | Check Content-Type, field types, body size, and validation. |
| Documentation status differs from HTTP | Match attribute metadata to the actual response helper. |
| PostgreSQL CLI support is unavailable | Reinstall crates/ruvoraq-cli with --features postgres. |
| Migration CLI reports missing DATABASE_URL | Set it in the environment or project .env; the CLI does not run configure. |
| Applied migration has changed | Restore the original file; append a new version for changes. |
| Database connection fails | Check the sqlite: URL, parent directory, and permissions. |
| Generated dependency path is missing | Update the local Ruvoraq path after moving the checkout. |
| Copied path uses another username | Preserve the generated Cargo path or replace /home/xagi with your actual checkout path. |
| Docker live tests cannot start | Verify docker info in WSL and Python 3.11+ before running the full suite. |

For contributor workflows, see the [developer guide](developer_guid.md).
For completed experiments and planned work, see [project tracking](../project.md).

## Development workflow and release builds

### Working on your application

Run these commands from the application root:

```bash
cargo check
cargo fmt
cargo clippy --all-targets -- -D warnings
ruvoraq dev
```

`ruvoraq dev` uses Cargo's development profile. Stop the server before restarting
after edits. The generated route module is loaded by bootstrap expansion;
format it directly when necessary:

```bash
rustfmt --edition 2024 src/main.rs
```

### Keep local artifacts out of Git

The generator deliberately creates only three files. In an independently
version-controlled application, create your own `.gitignore`:

```gitignore
target/
.env
.env.*
!.env.example
.postgres.env
*.sqlite
*.sqlite-shm
*.sqlite-wal
*.sqlite-journal
__pycache__/
*.pyc
```

Commit `Cargo.lock` for an application so dependency versions can be reproduced.
Keep migration SQL in version control and preserve applied versions unchanged.
Keep a placeholder `.env.example` for setup instructions.

### Build and run an optimized binary

After the application has a lockfile:

```bash
cargo build --release --locked
./target/release/hello_api
```

The executable name follows `[[bin]].name` in the generated manifest. For the
comprehensive example it is `app`. Custom `CARGO_TARGET_DIR` values change its
output location.

Run the binary from the application root when it uses relative configuration
or migration paths. Bootstrap reads `.env` from the working directory, not from
the executable's directory. Distributing only a binary is insufficient for an
application that loads SQL migrations at startup; ship the selected migration
folder as well.

Configuration also works with a release binary:

```bash
RUVORAQ_HOST=0.0.0.0 RUVORAQ_PORT=8080 RUVORAQ_DOCS=false \
  ./target/release/hello_api
```

`0.0.0.0` listens on all IPv4 interfaces; `127.0.0.1` listens on loopback.
Unix SIGTERM and Ctrl+C request graceful shutdown. There is currently no forced
shutdown deadline. These are build/runtime instructions; the framework remains
experimental and has not completed a production-readiness review.

For repeatable framework and HTTP verification, follow the
[testing guide](testing_guid.md). It explains default/all-feature Cargo checks,
the single comprehensive application, isolated test databases and expected
manual HTTP responses.

## Current limitations

| Area | Current boundary |
| --- | --- |
| Distribution | Version 0.1.0 is published on crates.io; source paths are optional development overrides. |
| Development server | Build/run only; no automatic file watching. |
| Validation | Explicit Validate implementation; no validation derive. |
| Dependency injection | Explicit per-App providers; no automatic construction or request-scoped graph. |
| Database tooling | SQLx queries and forward-only migrations; no ORM, add-db command or reversible migration generator. |
| API metadata | Known extractors/models are supported; custom extractors and dynamic statuses need additional metadata. |
| Platform verification | Linux/WSL2 verified; equivalent native Windows/macOS live runs are pending. |
| Deployment | Authentication, jobs and broader operations tooling remain planned. |
| Compatibility | The all-feature workspace passes Rust 1.85.0; continue checking the minimum toolchain for future releases. |
| PostgreSQL TLS | Rustls support is enabled; successful certificate-verified TLS tests remain pending. |

The [project tracker](../project.md) records delivered behavior and future work.
