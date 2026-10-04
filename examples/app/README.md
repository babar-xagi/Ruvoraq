# Fresh Ruvoraq example

Generated with the installed Experiment 005 CLI, then expanded with school and
billing modules. main.rs remains the original hello handler; settings.rs owns
configuration and provider registration.

Run inside WSL:

```sh
cd /home/xagi/Ruvoraq/examples/app
ruvoraq dev
```

Try requests in another terminal:

```sh
curl http://127.0.0.1:8000/
curl -i -H 'Content-Type: application/json' -d '{"name":"Ada"}' http://127.0.0.1:8000/school/students
curl -H 'x-request-id: demo-123' http://127.0.0.1:8000/school/students/1
curl 'http://127.0.0.1:8000/school/students?limit=5'
curl -i -X PATCH -H 'Content-Type: application/json' -d '{"name":"Grace"}' http://127.0.0.1:8000/school/students/1
curl -i -X DELETE http://127.0.0.1:8000/school/students/1
curl -i -X POST http://127.0.0.1:8000/billing/requests
```

| Route | Demonstrates |
| --- | --- |
| GET / | Original plain-text handler |
| GET /school | Automatic module registration and JSON values |
| GET /school/visits | Alias-based injection and a shared atomic counter |
| GET or POST /school/status | Stacked route attributes |
| GET /school/students | Typed query parameters and a JSON model collection |
| POST /school/students | Validated JSON input and created() / 201 |
| GET /school/students/{id} | Typed path, async service method and response headers |
| PUT /school/students/{id} | Validated replacement and automatic model response |
| PATCH /school/students/{id} | Json input, explicit validation and bad_request() |
| DELETE /school/students/{id} | no_content() / 204 and not_found() |
| GET /school/errors/internal | Generic JSON 500 without internal details |
| GET /billing | Independent provider and shared state |
| POST /billing/requests | accepted() / 202 |
| GET /billing/health | ok() / 200 |

Data is held in memory and resets when the server stops. Accepted billing requests
are illustrative records; this example does not start a worker or use a database.

Run the repeatable live test:

```sh
python3 tests/smoke.py
```

The test uses the installed CLI and runs servers on available local ports from a
temporary copy. It checks status helpers, CRUD, path/query/header handling,
validation, body limits, JSON errors, concurrent state, provider checks, CLI
protections, SIGINT and SIGTERM. It does not edit this example's source/settings.

Verify both the framework and this independent application:

```sh
cd /home/xagi/Ruvoraq
cargo fmt --all --check
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --manifest-path examples/app/Cargo.toml --check
cargo build --manifest-path examples/app/Cargo.toml --locked
cargo clippy --manifest-path examples/app/Cargo.toml --all-targets --locked -- -D warnings
python3 examples/app/tests/smoke.py
```
