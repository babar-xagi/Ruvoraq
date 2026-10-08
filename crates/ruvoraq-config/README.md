# Ruvoraq configuration

Typed configuration snapshots with process environment precedence over an
optional `.env` file. Loading does not mutate process environment variables.

```rust
use ruvoraq_config::Env;

fn load_port() -> std::io::Result<u16> {
    let env = Env::load()?;
    env.get_or("PORT", 8000u16)
}
```

Use `get::<T>` for required values, `optional::<T>` for optional values and
`get_or::<T>` for defaults when keys are absent. Invalid present values fail
instead of silently falling back. Diagnostics omit raw configuration values.

This is an experimental 0.2.0 release. The framework is developed in verified
increments and is not yet a complete production platform.

Documentation: [user guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/user_guid.md),
[developer guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/developer_guid.md),
[testing guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/testing_guid.md).

Licensed under Apache-2.0. Source: [Ruvoraq](https://github.com/babar-xagi/Ruvoraq).

🔎 Experiment 011 adds request IDs, structured tracing, explicit CORS and request
timeouts. Read the [middleware guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/middleware_guid.md).
