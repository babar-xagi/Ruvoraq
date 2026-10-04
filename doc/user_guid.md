# 📘 Ruvoraq user guide

Ruvoraq is an experimental Rust backend framework built around small applications,
route attributes, and optional features. This guide describes the implementation
through Experiment 010. The framework crates are currently used through local
path dependencies; they have not been published to crates.io.

## 🧭 Contents

- [Start an application](#-start-an-application)
- [Routes and responses](#-routes-and-responses)
- [Typed requests and validation](#-typed-requests-and-validation)
- [Configuration](#-configuration)
- [Shared services](#-shared-services)
- [Application modules](#-application-modules)
- [OpenAPI and Swagger UI](#-openapi-and-swagger-ui)
- [SQLite persistence](#-sqlite-persistence)
- [Versioned SQLite migrations](#-versioned-sqlite-migrations)
- [Optional PostgreSQL](#-optional-postgresql)
- [Examples and troubleshooting](#-examples-and-troubleshooting)

## 🚀 Start an application

The workspace declares Rust 1.85 or newer and uses edition 2024. You need Rust,
Cargo, and a working native build toolchain. The existing examples and their live
test scripts have been exercised in Linux/WSL2.

Install the CLI from your checkout:

```sh
cd /home/xagi/Ruvoraq
cargo install --path crates/ruvoraq-cli --locked --force
ruvoraq --help
ruvoraq --version
ruvoraq new hello-api
cd hello-api
ruvoraq dev
```

Visit http://127.0.0.1:8000/ for the greeting and
http://127.0.0.1:8000/docs for interactive API testing. Press Ctrl+C to stop.

After updating the CLI source, repeat the installation command. Building the
workspace does not replace the executable installed in ~/.cargo/bin.

### What is generated?

Immediately after `new`, the project contains exactly three files:

```text
hello-api/
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
dependency points to the local checkout; moving or sharing the project requires
updating that path.

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

## 🛣️ Routes and responses

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

## 🧩 Typed requests and validation

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

## ⚙️ Configuration

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
RUVORAQ_APP_NAME=hello-api
RUVORAQ_HOST=127.0.0.1
RUVORAQ_PORT=8000
RUVORAQ_DOCS=true
SCHOOL_GREETING=Welcome to school
```

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

## 🤝 Shared services

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

## 📦 Application modules

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

## 📚 OpenAPI and Swagger UI

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

## 🗄️ SQLite persistence

SQLite support is optional. Add the feature to your application's existing
Ruvoraq dependency; keep its generated path:

```toml
[dependencies.ruvoraq]
path = "/home/xagi/Ruvoraq/crates/ruvoraq"
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

## 🔄 Versioned SQLite migrations

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
are unsupported. Run one migrator at a time. There is no cross-process migration
coordinator in this phase.

Startup loads files relative to the current working directory. Ship migrations/
with the application and run from the intended project root. A missing or empty
directory is an error. The CLI requires DATABASE_URL explicitly, even if an
application hook has its own fallback.

The notes example's first migration uses CREATE TABLE IF NOT EXISTS to preserve
the known Experiment 008 notes schema. This is not general schema detection or
an automatic baseline for arbitrary legacy databases.

## 🐘 Optional PostgreSQL

Experiment 010 adds an explicit PostgreSQL adapter. Enable it in your
application's existing local Ruvoraq dependency:

```toml
[dependencies.ruvoraq]
path = "/home/xagi/Ruvoraq/crates/ruvoraq"
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

## 🧪 Examples and troubleshooting

One comprehensive application now combines the school/billing API and notes
database examples. SQLite is the default; PostgreSQL is an explicit feature.
Use [the complete testing guide](testing_guid.md) for a fresh project, manual
HTTP commands, expected responses, configuration and both database backends.

```sh
cd /home/xagi/Ruvoraq/examples/app
cp -n .env.example .env
ruvoraq migrate --status
ruvoraq migrate
ruvoraq dev
# From the framework root, run all automated live checks:
cd /home/xagi/Ruvoraq
python3 examples/app/tests/full_test.py --postgres
```

MIGRATIONS_DIR selects the migration folder. The CLI defaults to migrations;
this example's .env selects migrations/sqlite. PostgreSQL uses
migrations/postgres and `cargo run --no-default-features --features postgres`.
Process environment variables override .env for this setting too.

| Issue | What to check |
| --- | --- |
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

For contributor workflows, see the [developer guide](developer_guid.md).
For completed experiments and planned work, see [project tracking](../project.md).
