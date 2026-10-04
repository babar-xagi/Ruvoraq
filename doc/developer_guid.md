# 🛠️ Ruvoraq developer guide

This guide is for contributors to the framework. Application authors should
start with the [user guide](user_guid.md). The current architecture covers
Experiments 001–009; implementation details may change before a public release.

## 🧭 Repository map

| Crate | Responsibility |
| --- | --- |
| [ruvoraq](../crates/ruvoraq/src/lib.rs) | Public facade, prelude, runtime startup, feature exports. |
| [ruvoraq-macros](../crates/ruvoraq-macros/src/lib.rs) | Route attributes, schema attribute, bootstrap expansion. |
| [ruvoraq-web](../crates/ruvoraq-web/src/lib.rs) | HTTP adapter, extraction, responses, services, OpenAPI, lifecycle. |
| [ruvoraq-cli](../crates/ruvoraq-cli/src/main.rs) | Command parsing, project generation, module wiring, Cargo launcher. |
| [ruvoraq-config](../crates/ruvoraq-config/src/lib.rs) | Typed environment snapshots and optional dotenv loading. |
| [ruvoraq-db](../crates/ruvoraq-db/src/lib.rs) | Optional SQLx SQLite connection and transaction adapter. |

The public facade is the normal application dependency. The web crate builds on
Axum and Tokio. Database support is behind the facade's sqlite feature and uses
SQLx 0.8.6 with its SQLite/runtime features. A workspace build includes the
database member, while a default application dependency does not pull in SQLx.

[examples/app](../examples/app/src/settings.rs) demonstrates the general HTTP
features. [examples/sqlite-api](../examples/sqlite-api/src/settings.rs)
demonstrates asynchronous setup and durable data. The [migration-demo](../examples/migration-demo/README.md) provides a fresh
task API with a migration-first workflow. All three examples are independent
Cargo workspaces with their own lockfiles.

## 🏗️ Application startup

The generated manifest disables automatic binary discovery and selects
src/settings.rs as the binary entry point. Its bootstrap macro expands startup
and includes src/main.rs as the route module.

The lifecycle is:

1. Compile handlers, models, and ordinary Rust application modules.
2. Route attributes contribute registrations through inventory.
3. Bootstrap loads the current-directory environment snapshot.
4. App::auto gathers routes and validates route registrations.
5. Built-in environment settings are applied.
6. The optional configure hook supplies services or changes the App.
7. Startup checks required services and reserved documentation paths.
8. The server binds, reports its actual address, and serves requests.
9. Ctrl+C or Unix SIGTERM initiates graceful shutdown.

Bootstrap supports three forms:

```rust
ruvoraq::bootstrap!();
ruvoraq::bootstrap!(configure);
ruvoraq::bootstrap!(async configure);
```

Use exactly one per application. Hooks return App or std::io::Result<App>.
The async form awaits initialization before service checks and binding.
The hidden ConfiguredApp trait normalizes those return types.

There is no runtime source-file scanning, implicit constructor graph, or
generated build.rs. Keep application main.rs focused on routes and settings.rs
focused on composition.

## 🛣️ Route macro responsibilities

Attributed handlers must be safe, non-generic async free functions. The original
function remains callable. Expansion builds the adapter and registration
metadata, including method, path, module identity, description, schema probes,
and required service probes.

Path validation rejects malformed captures, duplicate parameter names,
wildcards, query strings, fragments, and legacy colon parameters. Captures use
whole segments, such as /students/{id}. Registration rejects duplicate
method/path pairs and conflicting parameter naming for the same route shape.
Different methods may share a path. Disabled cfg registrations stay disabled.

GET also serves HEAD through the HTTP adapter; it is not a separate documented
handler. Stacked method attributes are supported.

Some checks rely on recognizable syntax, including placing Json or
ValidatedJson last. Rust/Axum trait checking remains the final authority for
unusual aliases and custom extractors. Required-service probes support qualified
and aliased Inject types.

### Response conversion

Attribute adapters use trait probes to select response behavior:

- Existing IntoResponse behavior takes priority.
- Result values serialize successful models and preserve errors' responses.
- Other supported Serialize values become JSON.

This allows a model return type without forcing Json<Model> in every route.
Manual App builder handlers do not get the attribute adapter; use explicit
Json or Reply wrappers there.

The status attribute is OpenAPI metadata only. It accepts supported success
statuses and must not silently rewrite a handler's runtime response. Test both
the generated document and actual HTTP response when changing status behavior.

### Error behavior

Request extraction failures use framework envelopes with consistent HTTP
statuses. Validation is performed by Validate through ValidatedJson; plain Json
does not call it. The default JSON body limit is 2 MiB.

All framework 5xx errors redact private details. Serialization failure must stay
a server error even when an outer helper requests a success status. The panic
catch layer handles unwinding panics; panic hooks can still write diagnostics.
Custom responses and third-party extractors may define different formats.

Tests should exercise malformed bodies, unsupported content types, extraction
failures, missing resources, method errors, validation, serialization failures,
and redaction without asserting private values into public output.

## 🤝 Services and application state

App owns a registry indexed by concrete TypeId, storing shared Arc values.
provide(T) and provide_shared(Arc<T>) require Send + Sync + 'static. Registering
the same type replaces its previous value.

Inject<T> wraps Arc<T>, implements Clone without requiring T: Clone, and offers
Deref and into_inner. Attribute metadata records required services so a missing
provider is caught before binding. Explicit/manual handlers can still fail
during extraction if they bypass those checks.

Keep registries isolated between App instances. Avoid global mutable registries,
holding synchronous locks across await, and automatic construction hidden from
the settings hook. Future dependency features should preserve explicit startup
and clear missing-provider errors.

App::check performs startup validation. App::run and App::serve call it before
serving. A consumer assembling a router manually should call check before
into_router. Invalid manual builder routes can still encounter Axum's own
validation behavior.

Graceful shutdown drains active work; a handler that never completes can
prevent completion. A forced shutdown deadline is future work.

## ⚙️ Configuration internals

Env owns a snapshot rather than mutating global process variables.

- load reads an optional .env in the current working directory.
- load_from selects a directory explicitly.
- from_file requires the selected file to exist.
- from_values makes isolated snapshots for deterministic tests.
- Process values take precedence over dotenv values.
- Duplicate dotenv keys and malformed files fail with useful diagnostics.
- Typed getters distinguish absent values from invalid present values.
- Debug output and parse errors omit raw values.

Do not add global set_var calls to simplify tests. Use snapshots or subprocess
environments. Tests that exercise .env loading must set the child's working
directory deliberately; inheriting a temporary directory containing malformed
configuration can invalidate otherwise unrelated startup checks.

Settings::with_env applies application name and socket settings.
App::environment additionally controls docs and provides Env for injection.
Bootstrap performs this step automatically; manual App users opt in.

Keep parser errors clear without embedding secrets, raw database URLs, or
untrusted parse-error text in public diagnostics.

## 🗄️ SQLite adapter

The sqlite facade feature exports Database, Migrations, MigrationStatus, and sqlx. SQLx uses SQLite and Tokio
runtime and migration support without its query macros, PostgreSQL, or MySQL
features. Queries in the example use runtime query/query_as plus bound values.

Database::connect currently:

- Requires a sqlite: connection URL.
- Creates the file when appropriate, but not missing parent directories.
- Enables foreign keys.
- Uses one pooled connection and five-second busy/acquire timeouts.
- Reports connection/URL failures without exposing the raw URL.

One connection keeps in-memory SQLite consistent across requests and serializes
access. Do not claim production pool tuning or WAL configuration: those are not
implemented policies.

Database::pool permits SQLx execution. begin returns a SQLx transaction; commit,
explicit rollback, and drop rollback are covered by tests. Clones share the same
pool, and close affects all of them.

The notes example maps query failures to Error::internal before responding.
Its numbered first migration preserves the known original notes schema.
Arbitrary legacy schemas still require an explicit migration plan.

### Versioned migrations (Experiment 009)

Database::migrate(path) loads and applies files; migration_status(path) validates
and reports history. Migrations::load(path) creates a reusable source snapshot,
with run(&database) and status(&database) operations. MigrationStatus exposes
version, description, and applied.

The adapter enables SQLx's migrate feature. It validates positive unique
versions and simple filenames, then uses SQLx for checksums, history storage,
and per-file transactions. Before running pending files, it verifies every
recorded checksum, missing files, incomplete history, and append-only ordering.
This prevents a pending earlier file from running before discovering changed
later history. Driver errors omit SQL and raw connection details.

History lives in _sqlx_migrations. Status checks sqlite_master first, so a fresh
status request does not create migration history. The connection itself can
create a database file. Earlier committed migrations survive a later failure;
tests verify both DDL rollback and a fixed-file retry.

The CLI now links the SQLite adapter directly and supports migrate/--status
without compiling application code. The default application facade still
excludes SQLx. CLI configuration requires DATABASE_URL; it cannot infer a
fallback defined inside an application's configure hook.

Sources are runtime files, not embedded assets. Ship them with deployed
applications. Scripts are trusted developer SQL and must not control their own
transactions. SQLx's SQLite migration lock is a no-op; run one migration actor
at a time rather than claiming cross-process coordination.

Coverage includes five migration integration tests, two CLI behavior tests,
and live startup/upgrade/recovery checks in the notes example. Reversible
migrations and migration-file generation remain deferred.



## 📚 OpenAPI generation

The document is OpenAPI 3.1. The schema attribute adds the appropriate Schemars
derive and crate path so application authors need no separate schema dependency.

Schema generation handles input/output contracts, nested definitions, and
recursive references. Shared definitions are hoisted into components. Preserve
Serde naming, optionality, defaults, and unknown-field behavior when changing
schema conversion.

Known signature shapes contribute path/query parameters, request bodies, and
response contracts. Module paths group routes and make operation IDs distinct.
Opaque responses and dynamic statuses require conservative documentation rather
than invented schemas. HeaderMap keys, every alias, and custom validation
constraints cannot be inferred.

The reserved endpoints are /docs, /openapi.json, /docs/swagger-ui.css, and
/docs/swagger-ui-bundle.js. Conflict checks must include route-pattern conflicts,
not merely identical strings. Disabling docs removes the HTTP endpoints.

Swagger UI 5.11.0 assets are bundled for offline serving. Preserve the vendor
license when updating them. Verify CSS/JavaScript content types, local asset
links, and a real browser's Try it out workflow when replacing the UI.
The [README screenshot](../docs/images/swagger-ui.jpg) is an existing captured
image, not a generated mockup.

## 🧰 CLI and generator safety

The CLI exposes only the commands listed in the user guide. Do not document
future routes, doctor, database scaffolding, or reversible migration commands as available.

new validates names before writing and refuses non-empty, file, or symlink
targets. It creates files exclusively and rolls back only entries it owns.
Preserve its exactly-three-file initial contract.

add app parses existing Rust with Syn before wiring modules. It refuses
ambiguous/custom module declarations and existing paths. File edits use sibling
temporary files, snapshots, permission preservation, conflict checks, and
rollback of owned changes. This is a conservative update mechanism, not a
crash-proof filesystem transaction.

dev checks the project marker and binary selection before launching Cargo.
It preserves terminal interaction and exit/signal behavior. File watching is
not implemented. Updating the checkout does not update an already installed CLI.

## 🧪 Verification workflow

From the framework root:

```sh
cargo fmt --all --check
cargo build --workspace --locked
cargo check --workspace --locked
cargo test --workspace --locked
cargo check --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
git diff --check
```

The examples are independent workspaces; verify them separately:

```sh
cargo check --manifest-path examples/app/Cargo.toml --locked
cargo check --manifest-path examples/sqlite-api/Cargo.toml --locked
cargo check --manifest-path examples/migration-demo/Cargo.toml --locked
cargo clippy --manifest-path examples/app/Cargo.toml --all-targets --locked -- -D warnings
cargo clippy --manifest-path examples/sqlite-api/Cargo.toml --all-targets --locked -- -D warnings
cargo clippy --manifest-path examples/migration-demo/Cargo.toml --all-targets --locked -- -D warnings
cargo install --path crates/ruvoraq-cli --locked --force
python3 examples/app/tests/smoke.py
python3 examples/sqlite-api/tests/smoke.py
```

Run cargo fmt when making edits, then use the check form. Route files loaded by
bootstrap expansion may need direct rustfmt invocation because the formatter
does not expand procedural macros.

If a dependency change intentionally modifies a lockfile, update it first;
--locked verifies the resulting committed dependency graph.

Check the optional-dependency boundary:

```sh
cargo tree -p ruvoraq --no-default-features --prefix none
```

SQLx should not appear in the default facade graph. Building every workspace
member still builds the database member.

### Existing evidence

Experiment 009 verification recorded:

| Suite | Verified scope |
| --- | --- |
| Framework tests | 73 passing, including a documentation test; default and all features. |
| General application smoke suite | 68 live checks. |
| SQLite application smoke suite | 30 live checks, including migration upgrades and recovery. |
| Fresh task demo | 11 direct live checks and migration commands run in its actual directory. |
| Migration integration tests | Ordering, repeat/append, rollback/retry, changed/missing/dirty history, invalid sources, persistence. |
| Build/lint checks | Workspace and three independent applications, with strict Clippy. |

The automated live suites total 98 checks. The task demo's 11 checks were a
separate direct verification run; it has no standalone smoke script.
These counts describe the recorded validation, not permanent targets.

The automated live suites build temporary copies and own their server processes
and database files. They exercise startup errors, concurrent requests, signals,
configuration precedence, schemas, and real HTTP responses. Local task-demo
test data remains in its ignored SQLite database; it is not committed.

For manual migration verification, follow the
[task demo README](../examples/migration-demo/README.md). Run status, apply,
status, and apply again before starting the API. A repeat should apply zero
files without losing existing rows.

The workspace declares Rust 1.85. Experiment 008 was tested using the installed
Rust 1.99.0 toolchain and dependency MSRV metadata was checked; an actual
Rust 1.85 build has not yet been verified. A minimum-toolchain CI job remains
necessary before claiming that compatibility has been demonstrated.

## 🌱 Adding the next feature

Start with a user-visible behavior and a narrow scope. Decide which crate owns
it, whether it is optional, and how settings.rs composes it. Preserve the small
default scaffold and avoid extra dependencies in the default facade.

Add meaningful tests for behavior, failures, and boundaries. Update the user
guide with working examples, this guide with architectural implications, and
[project.md](../project.md) with implementation and verification status.

For further migration work, preserve version/checksum validation and test
recovery before adding reversible or generated workflows. For PostgreSQL, keep
backend selection explicit and test against a real server rather than treating SQLite success as equivalent.

Before a public release, establish CI, verify the declared minimum Rust
toolchain, review public API stability, and replace checkout-specific packaging.
Committing, pushing, and publishing remain separate actions requested by the
maintainer; documentation work does not implicitly publish the local changes.
