# Ruvoraq

### Rust APIs. Simple handlers. Explicit configuration.

Ruvoraq is a modular backend framework for Rust. Write asynchronous handlers,
register routes with attributes, and keep application startup in `settings.rs`.
Typed requests, shared services, OpenAPI documentation and optional databases
provide a foundation that grows with your application.

```rust
use ruvoraq::prelude::*;

#[get("/")]
async fn hello() -> &'static str {
    "Hello"
}
```

[crates.io](https://crates.io/crates/ruvoraq) · [Get started](#installation) · [User guide](doc/user_guid.md) ·
[API example](#a-typed-api-in-a-few-lines) · [Testing](doc/testing_guid.md) ·
[Developer guide](doc/developer_guid.md) · [Roadmap](project.md)

**Project status:** experimental, version `0.2.0`. Development has reached
Experiment 011. All six packages are published on crates.io as `0.2.0`.
Install the CLI from the registry or use the checkout for framework development. Production readiness remains a roadmap objective.

<a id="why-ruvoraq"></a>

## ✨ Why Ruvoraq

| Capability | What you get |
| --- | --- |
| Small starting point | Exactly three generated files; add modules when needed. |
| Declarative routing | `#[get]`, `#[post]`, `#[put]`, `#[patch]` and `#[delete]` register handlers automatically. |
| Typed APIs | Path/query extraction, JSON models, explicit validation and consistent errors. |
| Clear responses | `ok`, `created`, `accepted` and `no_content` helpers. |
| Shared services | Typed `Inject<T>` with provider validation before the server starts. |
| Interactive documentation | OpenAPI 3.1 and locally bundled Swagger UI. |
| Explicit configuration | Rust defaults, optional `.env` and typed environment values. |
| Optional persistence | SQLite or PostgreSQL through SQLx, transactions and versioned migrations. |
| Middleware | Validated request IDs, structured tracing, explicit CORS and handler timeouts. |
| Server lifecycle | Graceful Ctrl+C and Unix SIGTERM shutdown. |

<a id="installation"></a>

## 🛠️ Installation

The verified development environment is Linux/WSL2 with Rust and Cargo.
The workspace uses Rust edition 2024 and declares Rust 1.85 as its minimum;
verification currently uses Rust 1.99.0. The all-feature workspace also passes a Rust 1.85.0 build.

For operating-system prerequisites and Rust installation, follow the
[installation guide](doc/user_guid.md#installation-and-environment-setup).
With Rust and Cargo available:

```bash
cargo install ruvoraq-cli --version 0.2.0 --locked
ruvoraq --version
```

For PostgreSQL migration commands, install the optional CLI feature:

```bash
cargo install ruvoraq-cli --version 0.2.0 --features postgres --locked --force
```

Generated applications use `ruvoraq = "0.2.0"` from crates.io, so a framework
checkout is unnecessary. Application database features and CLI database features
are enabled separately. See [updating your installation](doc/user_guid.md#updating-your-installation).

For source development and the repository example:

```bash
cd "$HOME"
git clone https://github.com/babar-xagi/Ruvoraq.git
cd Ruvoraq
cargo install --path crates/ruvoraq-cli --locked --force
RUVORAQ_FRAMEWORK_PATH="$HOME/Ruvoraq/crates/ruvoraq" ruvoraq new local_api
```

The explicit override adds a local dependency path for unpublished framework
changes. Keep that checkout available for projects generated with the override.

<a id="your-first-application"></a>

## 🚀 Your first application

Run these commands from a directory where you keep application projects:

```bash
ruvoraq new hello_api
cd hello_api
ruvoraq dev
```

| Endpoint | Purpose |
| --- | --- |
| [http://127.0.0.1:8000/](http://127.0.0.1:8000/) | Greeting response. |
| [http://127.0.0.1:8000/docs](http://127.0.0.1:8000/docs) | Swagger UI with **Try it out**. |
| [http://127.0.0.1:8000/openapi.json](http://127.0.0.1:8000/openapi.json) | Generated OpenAPI document. |

Press Ctrl+C to stop. `ruvoraq dev` builds and runs the application; restart it
after edits because file watching is not implemented yet.

```text
hello_api/
├── Cargo.toml
└── src/
    ├── main.rs       # Route handlers
    └── settings.rs   # Configuration and startup
```

Cargo selects `settings.rs` as the binary entry point.
`ruvoraq::bootstrap!();` loads `main.rs` and starts the application.
Cargo creates `Cargo.lock` and `target/` during builds.

<a id="a-typed-api-in-a-few-lines"></a>

## 🧩 A typed API in a few lines

Add the following below the greeting in `src/main.rs`:

```rust
#[schema]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateUser {
    name: String,
}

impl Validate for CreateUser {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(invalid("name", "Name is required"));
        }
        Ok(())
    }
}

#[schema]
#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

/// Create a demonstration user.
#[post("/users", status = 201)]
async fn create_user(ValidatedJson(input): ValidatedJson<CreateUser>) -> Reply<User> {
    created(User {
        id: 1,
        name: input.name.trim().to_owned(),
    })
}
```

Restart the server and send a request:

```bash
curl -i http://127.0.0.1:8000/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Ada"}'
```

The response is HTTP `201` with `{"id":1,"name":"Ada"}`. A blank name returns
`422`. This example returns a model without storing it; the comprehensive
example demonstrates persistence. The route's `status` describes OpenAPI;
the `created` helper sets the actual HTTP status.

<a id="interactive-api-documentation"></a>

## 📚 Interactive API documentation

Request and response models annotated with `#[schema]` appear in OpenAPI.
Handler doc comments supply descriptions. Swagger UI serves its JavaScript
and CSS locally and supports sending requests directly from the browser.

![Ruvoraq Swagger UI with a successful API response](docs/images/swagger-ui.jpg)

Disable documentation endpoints with `RUVORAQ_DOCS=false`.
See [API documentation](doc/user_guid.md#openapi-and-swagger-ui) for metadata
behavior and supported schemas.

<a id="configuration-and-application-modules"></a>

## ⚙️ Configuration and application modules

Create an optional `.env` in your application root:

```dotenv
RUVORAQ_APP_NAME=hello_api
RUVORAQ_HOST=127.0.0.1
RUVORAQ_PORT=8000
RUVORAQ_DOCS=true
```

Precedence is **process environment → `.env` → Rust defaults**.
Configuration is read at startup. Custom values use typed `Env` getters.

```bash
ruvoraq add app school
RUVORAQ_PORT=8010 ruvoraq dev
```

The module command generates models, routes and services under `src/apps/school`
and connects the module to startup. Providers are constructed explicitly in
a configure hook and accessed through `Inject<T>`.

<a id="request-middleware-and-observability"></a>

## 📘 Request middleware and observability

Keep handlers small and configure HTTP policies in settings.rs:

```rust
fn configure(app: ruvoraq::App) -> std::io::Result<ruvoraq::App> {
    Ok(app
        .request_logging(true)
        .request_timeout(std::time::Duration::from_secs(5))
        .cors(ruvoraq::Cors::new(["http://localhost:3000"])?))
}

ruvoraq::bootstrap!(configure);
```

Replace the existing bootstrap invocation, or extend your existing configure
hook. Request IDs are enabled by default; logs, CORS and deadlines are opt-in.
Use RequestId in a handler to read its correlation ID. Browser preflight and
timeout responses retain the appropriate headers.

The [middleware guide](doc/middleware_guid.md) explains the pipeline, safe log
fields, environment settings, CORS concepts and cancellation boundaries.

<a id="cli-reference"></a>

## 💻 CLI reference

| Command | Purpose |
| --- | --- |
| `ruvoraq --help` | Display available commands. |
| `ruvoraq --version` | Display the installed version. |
| `ruvoraq new <project-name>` | Generate a protected three-file application. |
| `ruvoraq add app <module-name>` | Generate and connect an application module. |
| `ruvoraq dev` | Build and run from the application root. |
| `ruvoraq migrate` | Apply pending SQL migrations. |
| `ruvoraq migrate --status` | Inspect pending/applied versions. |

The generator validates package names and refuses non-empty directories,
existing files and symlink targets. Database migrations require `DATABASE_URL`;
`MIGRATIONS_DIR` defaults to `migrations` in the CLI.

<a id="explore-the-complete-example"></a>

## 📦 Explore the complete example

From a source checkout created with the development commands above:

The [comprehensive application](examples/app/README.md) combines school/billing
services with validated notes CRUD, configuration, documentation and migrations.
School and billing state reset on restart; notes persist in the database.

```bash
cd "$HOME/Ruvoraq/examples/app"
cp -n .env.example .env
ruvoraq migrate --status
ruvoraq migrate
ruvoraq dev
```

SQLite is the example default. PostgreSQL uses a separate feature and SQL folder.
See the [database walkthrough](doc/user_guid.md#optional-postgresql) and
[complete testing guide](doc/testing_guid.md) for both backends and manual requests.

<a id="development-and-verification"></a>

## 🧪 Development and verification

From the repository root:

```bash
cargo fmt --all --check
cargo check --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Full live verification requires Python 3.11+, Docker and a PostgreSQL-enabled CLI:

```bash
cargo install --path crates/ruvoraq-cli --features postgres --locked --force
python3 examples/app/tests/full_test.py --postgres
```

The new middleware module has a measured 100% line/function/region coverage gate.
Whole-workspace coverage is reported separately in the testing guide.

The runner uses temporary application copies and an owned disposable PostgreSQL
container. Current evidence includes **92 regular tests per feature configuration,
146 live checks and five real PostgreSQL tests**. Verification details and
remaining limits are recorded in the [testing guide](doc/testing_guid.md).

<a id="documentation-and-roadmap"></a>

## 🗺️ Documentation and roadmap

| Resource | Contents |
| --- | --- |
| [User guide](doc/user_guid.md) | Installation, first app, APIs, services, configuration and databases. |
| [Middleware guide](doc/middleware_guid.md) | Request IDs, tracing, CORS, timeouts and their limits. |
| [Testing guide](doc/testing_guid.md) | Repeatable checks, curl commands and expected responses. |
| [Developer guide](doc/developer_guid.md) | Architecture, crate boundaries and contribution workflow. |
| [Project tracker](project.md) | Completed experiments, limitations and planned work. |
| [Publishing guide](doc/publishing_guid.md) | Registry authentication, package checks and upload workflow. |

Authentication, jobs, higher-level database tooling
and AI capabilities remain future work. Successful certificate-verified PostgreSQL TLS tests remain pending.
The declared Rust 1.85 minimum now passes the all-feature build check.

<a id="license"></a>

## 📄 License

Licensed under [Apache-2.0](LICENSE).
