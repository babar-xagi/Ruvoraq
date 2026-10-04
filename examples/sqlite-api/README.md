# Persistent SQLite API

A notes API demonstrating validated CRUD, bound SQL queries, shared database
injection, OpenAPI, and forward-only migrations. Routes live in main.rs;
settings.rs connects the database and applies migrations before serving requests.

## Run the example

```sh
cd /home/xagi/Ruvoraq/examples/sqlite-api
ruvoraq dev
```

Open http://127.0.0.1:8000/docs for Swagger UI.
The server defaults to sqlite://notes.sqlite. Set DATABASE_URL to choose another
database; sqlite::memory: gives an ephemeral database.

```sh
curl -i http://127.0.0.1:8000/notes \
  -H 'Content-Type: application/json' \
  -d '{"title":"First note"}'
curl http://127.0.0.1:8000/notes
curl http://127.0.0.1:8000/notes/1
```

| Route | Behavior |
| --- | --- |
| GET / | Plain-text greeting. |
| GET /notes | List notes ordered by identifier. |
| POST /notes | Validate and create a note; return 201. |
| GET /notes/{id} | Read a note or return 404. |
| PUT /notes/{id} | Replace its title or return 404. |
| DELETE /notes/{id} | Delete a note; return 204, or 404 if missing. |

Titles are trimmed and must contain 1–120 characters. Records persist across
restarts with a file database. SQLite files and local .env files are ignored by
the framework checkout.

## Run migrations separately

From this example directory:

```sh
DATABASE_URL=sqlite://notes.sqlite ruvoraq migrate --status
DATABASE_URL=sqlite://notes.sqlite ruvoraq migrate
DATABASE_URL=sqlite://notes.sqlite ruvoraq migrate --status
```

Or copy .env.example to .env and run the commands without an environment prefix.
The CLI requires DATABASE_URL explicitly; it does not call the application's
configure hook or inherit that hook's fallback.

The migration in migrations/0001_create_notes.sql creates the known notes schema.
Its CREATE TABLE IF NOT EXISTS keeps the original Experiment 008 database
compatible. This is not automatic adoption of arbitrary legacy schemas.

Keep applied files unchanged. Add a higher version for changes. A failed file
rolls back its transactional work and can be fixed before retrying; earlier
successful files stay committed. Run one migrator at a time and include
migrations/ when deploying.

## Verification

From this example directory, inside Linux/WSL2:

```sh
cargo check --locked
cargo clippy --all-targets --locked -- -D warnings
python3 tests/smoke.py
```

The automated suite performs 30 live checks using a temporary project and
database. It verifies CRUD, validation, bound SQL, concurrent writes, restart
persistence, OpenAPI, migration status/repeated runs/upgrades, failed migration
rollback, retry, checksum drift, and redacted startup failures.

See the [migration-first task demo](../migration-demo/README.md),
[general API example](../app/README.md), and
[user guide](../../doc/user_guid.md). PostgreSQL, reversible migrations, and an
ORM remain future work.
