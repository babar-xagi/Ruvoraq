# 🧪 Experiments 001–011: complete testing guide

Run these commands inside WSL2. The repository now contains one example,
`examples/app`. It combines school/billing APIs with persistent notes. School
and billing reset on restart; notes persist in the selected database.

## ✅ 1. Install the current CLI and verify the framework

```bash
cd /home/xagi/Ruvoraq
rustc --version
cargo --version
cargo install --path crates/ruvoraq-cli --features postgres --locked --force
ruvoraq --help
ruvoraq --version
cargo fmt --all --check
cargo build --workspace --all-features --locked
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo check --manifest-path examples/app/Cargo.toml --locked
cargo check --manifest-path examples/app/Cargo.toml --no-default-features --features postgres --locked
```

The PostgreSQL server tests are intentionally ignored during ordinary Cargo
tests. The full test command below explicitly runs those five tests.

## 🚀 2. Run all automated live tests

Prerequisites: Python 3.11 or newer, the installed CLI, Rust, and a working Docker daemon.
The PostgreSQL image is `postgres:17-alpine`; Docker downloads it if necessary.

```bash
cd /home/xagi/Ruvoraq
docker info >/dev/null
python3 examples/app/tests/full_test.py --postgres
```

This builds the example and tests temporary project copies. It starts its own
PostgreSQL container on a random loopback port with a random password, then
removes the container and credentials in cleanup. It does not use an existing
database. PostgreSQL data created during the tests is discarded with that
container. The scripts do not change your example source or local notes database.

To test general HTTP behavior and SQLite without Docker:

```bash
python3 examples/app/tests/full_test.py
```

That command explicitly reports that PostgreSQL was not requested. Individual
suites are available as `tests/smoke.py`, `tests/sqlite.py`, and
`tests/postgres.py`. The PostgreSQL suite requires `RUVORAQ_TEST_POSTGRES_URL`
pointing to a dedicated, initially empty test database; prefer the Docker runner.

| Experiment | Automated coverage |
| --- | --- |
| 001 | Help/version, exact three generated files, package validation, overwrite protection, generated `cargo check`. |
| 002 | Route-only main.rs, attribute registration, settings bootstrap, real HTTP server, dev and shutdown. |
| 003 | Path/query/JSON types, status helpers, errors, body limits, validation and CRUD. |
| 004 | add app, module generation, automatic wiring and a compiling generated module. |
| 005 | Shared services, aliases, concurrency and missing providers. |
| 006 | OpenAPI schemas, response metadata, local Swagger assets and disabled docs. |
| 007 | `.env`, process precedence, typed configuration, redacted failures. |
| 008 | SQLite CRUD, bound SQL inputs, persistence and concurrent writes. |
| 009 | Migration status/apply/repeat, startup upgrades, history drift, rollback and retry. |
| 010 | PostgreSQL CRUD, parameters, persistence, migrations, real-server locking/transaction tests. |
| 011 | Request IDs, tracing, CORS/preflight, timeouts, cancellation, privacy and config failures. |

Current verified results: **92 regular framework tests per feature configuration,
146 live checks (70 general, 30 SQLite, 19 middleware, 27 PostgreSQL), and five real PostgreSQL
server tests**. Both example backends pass strict Clippy.

Coverage describes implemented behavior; it is not a production certification.
Successful certificate-verified PostgreSQL TLS still needs dedicated verification.
The declared Rust 1.85.0 minimum now passes an all-feature workspace build.

## 🛠️ 3. Create your own fresh minimal project

Choose a new name and directory; the generator refuses to overwrite non-empty
directories. These commands use `/home/xagi/ruvoraq-playground`:

```bash
mkdir -p /home/xagi/ruvoraq-playground
cd /home/xagi/ruvoraq-playground
ruvoraq new my_api
cd my_api
find . -type f | sort
# Exactly: Cargo.toml, src/main.rs, src/settings.rs
cargo check
ruvoraq dev
```

Open a second WSL terminal:

```bash
curl -i http://127.0.0.1:8000/
curl -s http://127.0.0.1:8000/openapi.json | python3 -m json.tool
```

Expected: `200` and `Hello`; OpenAPI version `3.1.0`. Open
`http://127.0.0.1:8000/docs` in your browser and use **Try it out**.
Stop the server with Ctrl+C before editing/restarting it.

```bash
ruvoraq add app school
cargo check
ruvoraq dev
# Second terminal:
curl -i http://127.0.0.1:8000/school
```

Generation is intentionally minimal. `add app` generates a module, not the
custom student/database APIs in the comprehensive example. There is no
`ruvoraq add db`, ORM, authentication, AI, or hot reload yet.

## 📦 4. Create your own comprehensive project

To exercise all currently implemented example code in your own generated
project, generate first and copy the example's deliberate additions:

```bash
cd /home/xagi/ruvoraq-playground
ruvoraq new all_features_api
cd all_features_api
cp /home/xagi/Ruvoraq/examples/app/Cargo.toml Cargo.toml
sed -i 's/name = "app"/name = "all_features_api"/g' Cargo.toml
sed -i "s|../../crates/ruvoraq|$HOME/Ruvoraq/crates/ruvoraq|g" Cargo.toml
cp -r /home/xagi/Ruvoraq/examples/app/src/. src/
cp -r /home/xagi/Ruvoraq/examples/app/migrations .
cp -r /home/xagi/Ruvoraq/examples/app/tests .
cp /home/xagi/Ruvoraq/examples/app/.env.example .env
cargo check
ruvoraq migrate --status
ruvoraq migrate
ruvoraq migrate --status
ruvoraq migrate
ruvoraq dev
```

The first migration apply prints `Applied 1 migration(s).`; the repeat prints
`Applied 0 migration(s).`. The copied settings retain the display name `app`.
Set `RUVORAQ_APP_NAME=My API` in `.env` for a different banner and OpenAPI title.
The manifest points to your local Ruvoraq checkout; update its absolute path
if you move the framework.

After stopping your copied project server, its full suite also works from that
project root:

```bash
python3 tests/full_test.py --postgres
```

For the repository example itself:

```bash
cd /home/xagi/Ruvoraq/examples/app
cp -n .env.example .env
ruvoraq migrate --status
ruvoraq migrate
ruvoraq dev
```

## 🌐 5. Exercise the APIs manually

Use a second terminal while the comprehensive server runs:

```bash
curl -i http://127.0.0.1:8000/
curl -i http://127.0.0.1:8000/school
curl -i http://127.0.0.1:8000/school/status
curl -i -X POST http://127.0.0.1:8000/school/status
curl -i http://127.0.0.1:8000/school/visits
curl -i -X POST http://127.0.0.1:8000/school/students \
  -H 'Content-Type: application/json' -d '{"name":" Babar "}'
# Fresh in-memory service: 201, {"id":1,"name":"Babar"}
curl -i 'http://127.0.0.1:8000/school/students?limit=10&min_id=1'
curl -i http://127.0.0.1:8000/school/students/1 -H 'x-request-id: manual-test'
curl -i -X PUT http://127.0.0.1:8000/school/students/1 \
  -H 'Content-Type: application/json' -d '{"name":"Ali"}'
curl -i -X PATCH http://127.0.0.1:8000/school/students/1 \
  -H 'Content-Type: application/json' -d '{"name":"Sara"}'
curl -i -X DELETE http://127.0.0.1:8000/school/students/1
# 204 with an empty body; reading it again returns 404.
curl -i http://127.0.0.1:8000/billing
curl -i -X POST http://127.0.0.1:8000/billing/requests
# 202 accepted receipt; the counter increases.
curl -i http://127.0.0.1:8000/billing/health
curl -i -X POST http://127.0.0.1:8000/notes \
  -H 'Content-Type: application/json' -d '{"title":"Persist this note"}'
curl -i http://127.0.0.1:8000/notes
```

Use the actual note ID returned by creation for subsequent commands:

```bash
curl -i http://127.0.0.1:8000/notes/1
curl -i -X PUT http://127.0.0.1:8000/notes/1 \
  -H 'Content-Type: application/json' -d '{"title":"Updated note"}'
```

Stop and restart `ruvoraq dev`: the note remains, students/billing reset.
Then delete the note:

```bash
curl -i -X DELETE http://127.0.0.1:8000/notes/1
curl -i http://127.0.0.1:8000/notes/1
# 204 followed by 404.
```

Try intentional invalid input:

```bash
curl -i -X POST http://127.0.0.1:8000/school/students \
  -H 'Content-Type: application/json' -d '{"name":" "}'  # 422
curl -i http://127.0.0.1:8000/school/students/not-a-number  # 400
curl -i 'http://127.0.0.1:8000/school/students?limit=0'    # 422
curl -i -X POST http://127.0.0.1:8000/notes \
  -H 'Content-Type: application/json' -d '{'              # 400
curl -i http://127.0.0.1:8000/missing                     # 404
curl -i -X OPTIONS http://127.0.0.1:8000/school           # 405
curl -i http://127.0.0.1:8000/school/errors/internal      # redacted 500
```

## ⚙️ 6. Verify configuration

Stop the server, then override settings without editing Rust:

```bash
RUVORAQ_PORT=8015 RUVORAQ_APP_NAME='Manual API' \
  SCHOOL_GREETING='Hello from config' ruvoraq dev
```

Use port 8015 for curl/browser requests. Process variables override `.env`;
`.env` overrides Rust defaults. To disable docs:

```bash
RUVORAQ_DOCS=false ruvoraq dev
# /docs and /openapi.json now return 404.
```

`MIGRATIONS_DIR` selects the SQL folder for both CLI and this app's configure
hook. CLI defaults to `migrations`; this comprehensive example defaults to
`migrations/sqlite` or `migrations/postgres`, according to its compiled backend.
The `.env.example` explicitly sets SQLite so CLI and startup agree. Relative
paths are resolved from the project root. Existing applications using root
`migrations/` retain their current behavior.

## 🐘 7. Manually run PostgreSQL

Use an existing dedicated development database and set its connection URL
without committing credentials. Stop SQLite before using the same port.

```bash
export DATABASE_URL='postgres://your_user:your_password@127.0.0.1:5432/your_database'
export MIGRATIONS_DIR=migrations/postgres
ruvoraq migrate --status
ruvoraq migrate
cargo run --no-default-features --features postgres
```

The application compiles SQLite by default. The explicit `postgres` feature
selects its PostgreSQL handlers, which use `$1/$2` parameters and BIGINT identity
columns. `ruvoraq dev` invokes default Cargo features; use the command above for
this optional backend. The CLI migration backend is chosen from DATABASE_URL
and needs the separately installed CLI `postgres` feature.

Use the same `/notes` HTTP commands and Swagger UI. PostgreSQL owns a different
database and version history; never point SQLite SQL at PostgreSQL. The full
automated test runner handles this selection in temporary copies for you.

## 🔎 Troubleshooting

| Issue | Fix |
| --- | --- |
| Old generated template | Reinstall CLI from this checkout using the first command block. |
| `ruvoraq`/Cargo missing | Run inside WSL and ensure `~/.cargo/bin` is on PATH. |
| Address already in use | Stop the other server or override `RUVORAQ_PORT`. |
| CLI cannot find migrations | Copy `.env.example` or set `MIGRATIONS_DIR` explicitly. |
| PostgreSQL CLI feature error | Reinstall CLI with `--features postgres`. |
| Applied migration changed | Restore the original; append a new numbered file instead. |
| Missing provider | Register the exact `Inject<T>` type in the configure hook. |
| First test build takes longer | Let Cargo finish; subsequent runs reuse test build artifacts. |
| Docker PostgreSQL test fails | Check `docker info`, image/network availability and the actual reported error. |

No commits or pushes are performed by these test scripts.


## 📊 Measured coverage

```bash
CARGO_LLVM_COV_TARGET_DIR=target/coverage-middleware cargo llvm-cov \
  -p ruvoraq-web --test middleware --json --output-path target/middleware-coverage.json
python3 scripts/check_middleware_coverage.py target/middleware-coverage.json
CARGO_LLVM_COV_TARGET_DIR=target/coverage-workspace cargo llvm-cov \
  --workspace --all-features --json --output-path target/workspace-coverage.json
```

Separate target directories avoid mixing stale binary/source mappings. The gate
requires 100% production middleware lines, functions and regions, without hiding
its code. Whole-workspace coverage is separately reported; it is not 100%, and
this command does not enable branch instrumentation. Recorded release metrics
are listed in [the coverage report](coverage.md). Broader coverage gaps include
legacy failure paths and platform-specific behavior.
