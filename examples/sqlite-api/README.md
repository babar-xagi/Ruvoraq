# Persistent SQLite API

Run from this directory with `ruvoraq dev`, then open http://127.0.0.1:8000/docs.
This example enables the optional Ruvoraq sqlite feature. Its async settings hook
opens DATABASE_URL (default sqlite://notes.sqlite), creates an initial schema,
and injects Database into the typed note handlers.

GET/POST /notes and GET/PUT/DELETE /notes/{id} use parameterized queries.
The schema is idempotent initial setup; versioned migrations and an ORM are
not implemented. Data stays in notes.sqlite across server restarts.
To use a temporary in-memory database, set DATABASE_URL=sqlite::memory:.
The existing examples/app keeps its in-memory services and does not enable SQLite.

```sh
ruvoraq dev
# in another terminal
curl -H 'Content-Type: application/json' -d '{"title":"First note"}' http://127.0.0.1:8000/notes
curl http://127.0.0.1:8000/notes
```

Repeatable live verification (inside WSL):

```sh
python3 tests/smoke.py
```

Tests use temporary project/database files and check persistence after restart,
bound SQL values, CRUD, validation, concurrent inserts, OpenAPI and startup errors.
Real SQLite files are ignored by Git.
