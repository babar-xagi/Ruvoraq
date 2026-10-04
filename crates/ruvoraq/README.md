# Ruvoraq

Build Rust APIs with simple handlers and explicit configuration.
Ruvoraq provides route attributes, typed extraction, validated JSON, response
helpers, shared services, typed environment configuration and OpenAPI 3.1 with
locally bundled Swagger UI. SQLite/PostgreSQL are opt-in features.

```bash
cargo install ruvoraq-cli --version 0.1.0 --locked
ruvoraq new hello_api
cd hello_api
ruvoraq dev
```

Generated `src/main.rs`:

```rust
use ruvoraq::prelude::*;

#[get("/")]
async fn hello() -> &'static str {
    "Hello"
}
```

The CLI generates exactly `Cargo.toml`, `src/main.rs`, and `src/settings.rs`.
The settings file owns bootstrap/startup. Open `/docs` for Swagger UI or
`/openapi.json` for the generated specification.

```toml
[dependencies]
ruvoraq = "0.1.0"
```

Optional features: `sqlite` exports `Database`; `postgres` exports
`PostgresDatabase`. Both expose SQLx, transactions and checked forward-only
migrations. The default framework does not include database drivers.

Use `created(value)` for HTTP 201, `accepted(value)` for 202, and `no_content()`
for 204. Route status metadata documents responses; helpers determine runtime
HTTP status. Use `ValidatedJson<T>` with an explicit `Validate` implementation.

This is an experimental 0.1.0 release. The framework is developed in verified
increments and is not yet a complete production platform.

Documentation: [user guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/user_guid.md),
[developer guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/developer_guid.md),
[testing guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/testing_guid.md).

Licensed under Apache-2.0. Source: [Ruvoraq](https://github.com/babar-xagi/Ruvoraq).
