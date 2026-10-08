# 🦀 Comprehensive Ruvoraq example

One fresh application covers Experiments 001–011: typed routes, validation,
status helpers, services, configuration, offline Swagger, SQLite/PostgreSQL and
versioned migrations. `src/main.rs` contains the greeting route;
`src/settings.rs` handles configuration, database startup and providers.
School/billing live in `src/apps`; database note handlers live in `src/notes`.

```bash
cd /home/xagi/Ruvoraq/examples/app
cp -n .env.example .env
ruvoraq migrate --status
ruvoraq migrate
ruvoraq dev
```

Open http://127.0.0.1:8000/docs. School students and billing counters are in
memory; `/notes` persists in SQLite. The default backend is SQLite. PostgreSQL
uses `cargo run --no-default-features --features postgres`, an existing database,
and `MIGRATIONS_DIR=migrations/postgres`. The CLI chooses its backend from
DATABASE_URL; install the CLI's PostgreSQL feature separately.

```bash
cd /home/xagi/Ruvoraq
python3 examples/app/tests/full_test.py --postgres
```

The runner uses temporary copies and an owned disposable Docker PostgreSQL
container. It preserves your app source and local database. Without
`--postgres`, it runs only general/SQLite checks and says so explicitly.

See the [complete testing guide](../../doc/testing_guid.md) for installation,
all commands, expected responses, your own new project, manual CRUD, migration
recovery tests, configuration and PostgreSQL setup. There is no database
scaffolding command yet; the comprehensive example deliberately adds these
modules after the three-file generator.


## 🔎 Middleware defaults in this example

Request IDs are enabled; JSON request logs, a two-second timeout and the browser
origin http://localhost:3000 are configured in settings.rs. Override logging and
timeout through RUVORAQ_REQUEST_LOG/RUVORAQ_REQUEST_TIMEOUT_MS and the frontend
through FRONTEND_ORIGIN. The configure hook reads these values when choosing
defaults. Request IDs can be disabled with RUVORAQ_REQUEST_ID=false.

See the [middleware guide](../../doc/middleware_guid.md) for browser headers,
tracing context and deadline limits. tests/middleware.py checks real CLI-launched
HTTP behavior on temporary copies; the slow fixture is not part of this app.
