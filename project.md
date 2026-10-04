# 🧭 Ruvoraq project tracker

Ruvoraq aims to make Rust backend development approachable through simple route
handlers, explicit application composition, and optional batteries. Longer-term
goals include production operations and provider-independent AI capabilities.

This tracker describes repository work as of **2026-10-04**. Future phases are
proposals, not delivered features or promised release dates.

## 📍 Current position

- Version: 0.1.0, experimental local development.
- Current implementation: Experiment 010, optional PostgreSQL support and consolidated testing.
- Delivery includes Experiment 010 and the fresh comprehensive example; see Git history.
- One runnable example: school/billing APIs with SQLite or PostgreSQL notes.
- Verification: 75 regular tests per configuration, five real PostgreSQL tests,
  and 127 automated live checks, plus direct fresh-example migration/restart checks.
- Framework crates: all six packages published to crates.io as 0.1.0.
- Next proposed phase: request middleware and observability, with scope selected by the maintainer.
- Delivery identifiers and branch publication are recorded in Git history.

The current foundation is functional and tested. It is not yet a complete
production platform with authentication, complete operations tooling, or AI.

## ✅ Completed experiments

| Experiment | Delivered outcome | Repository status |
| --- | --- | --- |
| 001 — Minimal CLI | Help/version, safe name validation, protected three-file project generation. | Pushed |
| 002 — Web startup | Route attributes, automatic registration, settings bootstrap, server lifecycle, dev command. | Pushed |
| 003 — Typed APIs | Typed extraction, explicit validation, JSON models, response helpers, error envelopes. | Pushed |
| 004 — Modules | Optional application modules and conservative automatic wiring. | Pushed |
| 005 — Services | Per-application shared services and typed Inject<T>. | Pushed |
| 006 — API documentation | Automatic OpenAPI 3.1, offline Swagger UI, API testing UI and screenshot. | Pushed |
| 007 — Configuration | Typed environment snapshots, optional dotenv, built-in settings overrides. | Pushed |
| 008 — SQLite | Optional database feature, bound SQL, transactions, async setup, durable notes example. | Pushed |
| 009 — Migrations | Ordered SQL files, checksums, history validation, transactions, CLI status/apply, startup migrations. | Pushed |
| 010 — PostgreSQL | Opt-in provider, transactions, migration locks, feature-aware CLI, real-server tests, and example. | Verified; included in this delivery |

These experiments extend one framework; their numbers record actual work.
Earlier experiment limitations should not be read as current feature limits.

## 🧩 Implemented capabilities

### Application experience

- [x] Exactly three files in a newly generated project.
- [x] Route-focused main.rs and composition-focused settings.rs.
- [x] Automatic attributed route registration.
- [x] GET, POST, PUT, PATCH, and DELETE attributes.
- [x] Configurable listening address and development launcher.
- [x] Ctrl+C and Unix SIGTERM graceful shutdown.
- [x] Optional app modules without forcing them into small applications.

### HTTP and application state

- [x] Path, query, JSON, validated JSON, and header access.
- [x] Automatic JSON serialization for attributed model responses.
- [x] Named success helpers and consistent framework errors.
- [x] Explicit validation rules and JSON body limits.
- [x] Shared typed services isolated per App.
- [x] Missing attributed-handler providers checked before binding.

### Documentation and configuration

- [x] OpenAPI schemas, parameters, bodies, and response metadata.
- [x] Offline Swagger UI and local assets.
- [x] Optional documentation endpoints with conflict checks.
- [x] Typed configuration without global environment mutation.
- [x] Process-over-dotenv precedence and redacted parse diagnostics.
- [x] Detailed [user](doc/user_guid.md) and [developer](doc/developer_guid.md) guides.

### Database foundation

- [x] Opt-in SQLite facade feature.
- [x] SQLx pool and parameterized runtime queries.
- [x] Transaction commit and rollback.
- [x] Foreign keys and redacted connection failures.
- [x] Async application configuration hook.
- [x] Persistent notes CRUD example with restart tests.
- [x] Forward-only versioned SQLite migrations.
- [x] SQLite migration apply/status CLI commands.
- [ ] Database scaffolding and reversible migration CLI commands.
- [x] Optional PostgreSQL adapter and real-server integration coverage.
- [ ] ORM or high-level model/repository generation.

## 🏗️ Repository structure

```text
Ruvoraq/
├── Cargo.toml / Cargo.lock
├── README.md
├── project.md
├── doc/
│   ├── user_guid.md
│   └── developer_guid.md
├── docs/images/swagger-ui.jpg
├── crates/
│   ├── ruvoraq/
│   ├── ruvoraq-cli/
│   ├── ruvoraq-config/
│   ├── ruvoraq-db/
│   ├── ruvoraq-macros/
│   └── ruvoraq-web/
└── examples/
    └── app/
```

The singular doc/ directory contains the requested written guides. The existing
plural docs/ directory holds the captured Swagger image.

The default facade uses the web/configuration foundation. SQLite and PostgreSQL are optional
for applications, although workspace checks include the database crate.
The framework currently uses Axum/Tokio for HTTP, Schemars for schema generation,
and SQLx for SQLite/PostgreSQL.

## 🧪 Verification evidence

### Experiment 010 initial verification (before consolidation)

Default and all-feature workspace runs each pass 74 regular tests. The
default-only CLI gating test and PostgreSQL-only URL test differ between those
runs. Five additional server tests were explicitly requested and passed on an
isolated PostgreSQL 17 container. The API suites pass 125 live checks:
68 general, 30 SQLite, and 27 PostgreSQL. PostgreSQL-only builds and strict
Clippy also pass. TLS-required rejection is tested; successful verified-TLS
connections are not yet tested.

The temporary PostgreSQL container and generated test credentials are removed
after verification. Real-server tests are opt-in in ordinary cargo test runs.

### Experiment 009

Experiment 009 adds seven framework tests (73 total) and extends the SQLite
live suite to 30 checks. The general live suite retains 68 checks, for 98
combined live checks. Format/build/check/test and strict Clippy cover the
workspace and independent examples. The default facade still excludes SQLx.

A fresh migration-demo example was also generated and exercised in its own
directory: two pending versions applied, repeated application skipped both,
and 11 live checks verified the task API, documentation, signals, and restart
persistence. That historical demo was subsequently removed during example consolidation.

### Experiment 008 baseline

Experiment 008 passed workspace build, format, check, test, and strict Clippy
checks, including all features. Both independent examples were also checked.

| Evidence | Result |
| --- | --- |
| Framework test count | 66 passing, including the documentation test |
| General application live checks | 68 passing |
| SQLite application live checks | 23 passing |
| Combined live checks | 91 passing |
| Default facade dependency graph | SQLx absent without sqlite feature |
| Database persistence | Verified through close/reopen and server restarts |
| Secret handling | Configuration and connection failures tested for redaction |

These counts describe the recorded implementation verification. Documentation
edits receive their own link and example checks rather than implying every test
was rerun after each prose change.

Rust 1.85 is the declared minimum; verification used the installed Rust 1.99.0
toolchain. Dependency MSRV metadata was checked, but a real 1.85 build remains
unverified.

## 🚧 Current limitations

- dev builds and runs; file watching and hot reload are not implemented.
- SQLite uses one connection and PostgreSQL uses five; configurable pool tuning and reversible migrations are pending.
- Example school/billing state is in memory; billing is not payment processing.
- Custom validation rules are not automatically translated into OpenAPI.
- Custom extractors, aliases, and dynamic response types have documentation limits.
- Manual builder routes need explicit response wrappers and are not auto-documented.
- No automatic dependency constructor graph or request-scoped providers.
- No authentication, authorization, admin, cache, background jobs, or AI APIs.
- No established public-release compatibility policy or published crates.
- Graceful shutdown has no forced deadline for permanently blocked handlers.

## 🗺️ Current phase and proposed next phases

### Experiment 009 — Versioned SQLite migrations (complete)

Goal: evolve a database schema safely and repeatably.

- [x] Define ordered migration files and a schema-version table.
- [x] Apply pending migrations without reapplying completed ones.
- [x] Define transaction and failed-migration recovery behavior.
- [x] Add a small migration CLI workflow with clear errors.
- [x] Test fresh databases, repeated runs, upgrades, and failures.
- [x] Convert the notes example from startup DDL to migrations.
- [x] Document the workflow before release.

Commands: `ruvoraq migrate` and `ruvoraq migrate --status`.
The API offers Database::migrate and Database::migration_status.
Reversible migrations, file generation, and cross-process coordination remain
future work; scripts must leave transaction boundaries to the runner.

### Experiment 010 — Optional PostgreSQL (implemented locally)

Goal: support a server database while retaining a minimal default application.

- [x] Decide shared database abstractions versus explicit backend types.
- [x] Add optional PostgreSQL connection/configuration support.
- [x] Test binding, transactions, migrations, and connection failures on a real server.
- [x] Document backend differences rather than hiding them.
- [x] Keep SQLx backend dependencies opt-in.

### Experiment 011 — Request middleware and observability (proposed)

Goal: expose useful request diagnostics and explicit HTTP policies while keeping
application handlers small. Exact APIs and scope remain to be decided.

### Later foundation work

- [ ] Request logging, request identifiers, and useful tracing.
- [ ] Explicit middleware configuration, CORS, and request timeouts.
- [ ] A documented application testing interface.
- [ ] Shutdown deadlines and operational health patterns.
- [ ] Automated CI with default/all-feature checks and minimum Rust coverage.
- [ ] Reproducible packaging and a public API/versioning policy.
- [ ] Configuration profiles if real application needs justify them.

### Optional application batteries

- [ ] Authentication and authorization with tested secure defaults.
- [ ] Database-backed services and reusable repository patterns.
- [ ] Cache, background jobs, and scheduling.
- [ ] Admin capabilities and WebSocket support where required.

### Longer-term AI capabilities

- [ ] Provider-independent model interfaces.
- [ ] Streaming and structured outputs.
- [ ] Explicit tool execution and agent composition.
- [ ] Embeddings, vector-store adapters, and retrieval workflows.
- [ ] Local-model integration where feasible.

These are product goals. No AI providers, agents, RAG system, or enterprise
features are implemented by the current experiments.

## 🎯 Principles for future decisions

1. Keep new applications minimal; add features deliberately.
2. Keep composition visible in settings.rs.
3. Preserve Rust's explicit types and understandable compile-time behavior.
4. Make optional capabilities available without forcing their dependencies.
5. Prefer verified behavior and clear errors over broad untested APIs.
6. Keep documentation examples aligned with executable examples.

## 📦 Experiment 009 delivery

- [x] Implement ordered, checked, transactional SQLite migrations.
- [x] Add migration status/apply commands.
- [x] Upgrade the notes example and create the migration-first task demo.
- [x] Verify migration behavior, real HTTP, and persistence.
- [x] Update every maintained guide and example README.
- [x] Review tracked files and keep local databases/.env out of the delivery.

Experiment 009 was delivered in commit 8fc3f7a. Experiment 010 and the
consolidated example are included in the following delivery. Consult Git
history for commit identifiers and remote branch state.

For later releases, keep minimum-toolchain checks, establish CI, review the
public API, and maintain packaging and package ownership. Keep those
release tasks separate from completing an individual experiment.

Update this file whenever scope, verification evidence, or repository status
changes. Mark planned work complete only after implementation and verification.

## 🧪 Fresh example consolidation (Experiments 001–010)

- [x] Remove all four previous example applications and generate one fresh app.
- [x] Verify the generator creates exactly three files and the result compiles.
- [x] Exercise add app, then combine school/billing APIs with persistent notes.
- [x] Keep SQLite as the example default and PostgreSQL an explicit feature.
- [x] Add MIGRATIONS_DIR so CLI and startup select backend-specific SQL consistently.
- [x] Add a migration-directory/precedence regression test.
- [x] Fix stale Cargo reuse caused by preserving temporary source timestamps.
- [x] Add a full live runner with an owned disposable PostgreSQL container.
- [x] Fix PostgreSQL readiness probing to wait for the final TCP server.
- [x] Write [complete manual and automated testing commands](doc/testing_guid.md).

The consolidated suite supersedes the old multi-example commands. Historical
verification counts above describe previous runs. The maintainer requested
publication of this verified work. Minimum Rust-toolchain and successful
certificate-verified PostgreSQL TLS checks remain future work.

Current consolidation verification: **75 regular tests per workspace feature
configuration, 127 live checks, and five real PostgreSQL server tests passed**.
Format, build, check and strict Clippy passed for the framework and both example
backends. The owned PostgreSQL test container and credentials were removed.

## Crates.io publication (0.1.0)

- [x] Enable six registry packages with repository metadata and packaged README/licenses.
- [x] Add version constraints to all internal path dependencies.
- [x] Generate registry projects by default; explicit source override remains available.
- [x] Add two meaningful generator regression cases (77 regular tests per configuration).
- [x] Verify the complete all-feature workspace using Rust 1.85.0.
- [x] Complete authenticated crates.io upload and verify a fresh registry installation.

Published version: 0.1.0 for all six crates. Existing Git tag 0.01 remains
unchanged. See [publishing instructions](doc/publishing_guid.md).

### Public registry verification

The installed registry CLI generates portable version-only applications without
the checkout. Default, SQLite and PostgreSQL feature builds, real HTTP greeting,
OpenAPI/Swagger, module generation and signal shutdown pass with public packages.
The initial new-crate rate limit was handled by waiting for the specified retry
time and uploading only the remaining core crate. All six registry versions are
0.1.0; the original Git tag 0.01 remains unchanged.

The published package source is tagged v0.1.0 at commit d00392c.
