# SQLite migration demo

A small task API that demonstrates running migration commands from the
application directory before starting the server. It uses the optional sqlite
feature and keeps routes in main.rs and startup in settings.rs.

## Run migrations first

```sh
cd /home/xagi/Ruvoraq/examples/migration-demo
cp -n .env.example .env
ruvoraq migrate --status
ruvoraq migrate
ruvoraq migrate --status
ruvoraq migrate
```

On a fresh database, versions 1 and 2 are pending before the first apply. Applying reports
`Applied 2 migration(s).`; status then marks both applied. Running migrate
again reports `Applied 0 migration(s).`.

The local tasks.sqlite file and .env are ignored by the framework checkout.
Keep applied SQL files unchanged and add higher versions for future changes.
The CLI requires DATABASE_URL from the process environment or .env.

## Start the API

```sh
ruvoraq dev
```

Visit http://127.0.0.1:8010/docs to test routes in Swagger UI.

```sh
curl -i http://127.0.0.1:8010/tasks \
  -H 'Content-Type: application/json' \
  -d '{"title":"Try versioned migrations"}'
curl http://127.0.0.1:8010/tasks
curl http://127.0.0.1:8010/tasks/1
```

POST /tasks returns 201; GET /tasks lists records and GET /tasks/{id} reads one.
Titles are trimmed and must contain 1–120 characters. Missing records return
404. Records survive restarts.

Startup also checks and applies pending migrations before binding. This makes
the manual migration workflow and server startup use the same version history.

## Files

- migrations/0001_create_tasks.sql creates the tasks table.
- migrations/0002_tasks_title_index.sql adds an index.
- src/main.rs contains attributed route handlers and bound SQL queries.
- src/models.rs contains request/response schemas and validation.
- src/settings.rs connects the database, migrates, and registers its provider.

The example requires the current local CLI. Reinstall it from the framework
root after CLI changes:

```sh
cargo install --path crates/ruvoraq-cli --locked --force
```

See the [user guide](../../doc/user_guid.md) for migration rules and limitations.

## Verification

```sh
cargo fmt --check
rustfmt --edition 2024 --check src/main.rs
cargo check --locked
cargo clippy --all-targets --locked -- -D warnings
```

This example was exercised in its own directory: both migrations applied,
a repeat skipped both, and 11 live checks verified requests, validation,
OpenAPI/Swagger, shutdown signals, and data surviving a restart. Those checks
were a direct verification run; this example has no standalone automated smoke
script. The notes example provides the repeatable migration smoke suite.

See the [developer guide](../../doc/developer_guid.md) for workspace checks
and [project tracker](../../project.md) for the current roadmap.
