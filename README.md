# Ruvoraq

Ruvoraq is being developed through small experiments. Experiment 001 delivered
a protected three-file project generator. **Experiment 002 adds the web core**:
route attributes, automatic route registration, server startup, host/port settings,
graceful shutdown, and a development command.

## Quick start

Rust 1.85 or newer is required (edition 2024). Install the CLI from this checkout:

```sh
cargo install --path crates/ruvoraq-cli --locked --force
ruvoraq --help
ruvoraq --version
ruvoraq new web-demo
cd web-demo
ruvoraq dev
```

Open http://127.0.0.1:8000/ or run `curl http://127.0.0.1:8000/`.
The response is `Hello`. Press Ctrl+C to stop.

`ruvoraq dev` runs Cargo with inherited terminal input/output and propagates its
exit status. It is a build-and-run command; file watching is not implemented.
Run it from the project's root. Projects are identified by
`[package.metadata.ruvoraq]` with `project = true`.
Set `CARGO` to override the Cargo executable if needed.

## Three-file applications

`new` still creates exactly three files and one source directory:

```text
web-demo/
├── Cargo.toml
└── src/
    ├── main.rs
    └── settings.rs
```

`main.rs` contains only your routes:

```rust
use ruvoraq::prelude::*;

#[get("/")]
async fn hello() -> &'static str {
    "Hello"
}
```

Add handlers with `#[get]`, `#[post]`, `#[put]`, `#[patch]`, or `#[delete]`,
each taking a path such as `#[get("/hello")]`. There is no manual route list,
settings import, runtime setup, or main function in this file.

`settings.rs` contains `APP_NAME`, `HOST`, `PORT`, a `settings()` function,
and `ruvoraq::bootstrap!();`:

```rust
use std::net::Ipv4Addr;

use ruvoraq::Settings;

pub const APP_NAME: &str = "web-demo";
pub const HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const PORT: u16 = 8000;

pub fn settings() -> Settings {
    Settings {
        app_name: APP_NAME.to_owned(),
        address: (HOST, PORT).into(),
    }
}

ruvoraq::bootstrap!();
```

Rust still requires a binary entry point. The manifest sets `autobins = false`
and points its single binary at `src/settings.rs`. `bootstrap!` generates the
entry point, loads `main.rs` as a handler module, applies settings and runs
the server. Both `cargo run` and `ruvoraq dev` work. No build script or fourth
project file is created.

Route attributes preserve callable async functions and emit typed registrations
collected with [inventory](https://docs.rs/inventory/0.3.24/). Registration does
not scan source files at runtime or depend on macro invocation order.
Routes are sorted before startup; duplicate method/path pairs and apps without
routes return errors before binding. Disabled `#[cfg]` handlers are not registered.

Edit host or port in settings.rs. Port 0 selects an available port; the startup
banner reports the actual bound address. The default binds only to 127.0.0.1:8000.

Ruvoraq is not published yet. The generated manifest uses an absolute path
dependency on the local `crates/ruvoraq` directory from which the CLI was built.
Keep that checkout available. If it moves, update the dependency path or reinstall
the CLI and regenerate a project. No registry package is assumed to exist.
Existing explicit-startup apps remain compatible and are not migrated automatically.
To migrate an existing app, add route attributes,
keep configuration and `bootstrap!` in settings.rs, and configure its binary target
as described above.

The generator does not run Cargo, initialize Git, or create a lockfile. Running
Cargo later may create `Cargo.lock` and `target/`.
An empty `[workspace]` keeps generated apps independent of surrounding workspaces.

Names are 1–64 ASCII letters, digits, hyphens or underscores, starting with a
letter or underscore. Rust keywords, Cargo artifact directory names and Windows
device names are rejected. Paths are not accepted. Existing empty directories
are accepted; non-empty directories (including hidden entries), files and
symlink targets are refused. Errors go to stderr with exit code 1. Files use
exclusive creation; failures attempt to remove only entries created by that call.

## Web core and architecture

The workspace has four members:

| Crate | Responsibility |
| --- | --- |
| `ruvoraq` | Public facade, prelude, and async runtime entry point |
| `ruvoraq-macros` | Route attributes and settings bootstrap |
| `ruvoraq-web` | Axum HTTP adapter, routes, settings, server lifecycle |
| `ruvoraq-cli` | Project generation and Cargo development launcher |

`App` supports `.get()`, `.post()`, `.put()`, `.patch()`, and `.delete()`.
Methods can share a route path. Axum supplies response conversion, 404/405
responses, and HEAD for GET routes. Handler bounds and invalid-route behavior
currently follow Axum. Attribute paths are static paths starting with `/`;
parameters, queries, fragments and wildcard paths are deferred. Handler attributes
require safe, non-generic async functions and report invalid signatures/paths at
compile time. A backend-independent handler API remains future work.

The server drains active requests on Ctrl+C (Unix/Windows) or SIGTERM (Unix).
`App::serve` accepts an already-bound Tokio listener and a custom shutdown future
for tests and lifecycle integrations. A stuck handler can delay shutdown; a
forced-shutdown timeout is deferred.

There are no database, auth, AI, module, validation, OpenAPI, or hot-reload
features in this experiment.

## Verification

```sh
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

CLI tests verify the exact initial scaffold, overwrite protection, project
recognition, launcher errors and exit codes, generated-project compilation,
route-only source, useful compile errors, duplicate/missing routes, and automatic
registration across modules and conditional handlers.
On Unix, a generated application is launched through `ruvoraq dev`, queried over
HTTP, and stopped with both SIGINT and SIGTERM. Web tests use real local sockets
to check all five methods, responses, 404/405, bind errors, and draining an active
request. Generated-project checks run offline after workspace dependencies have
been fetched, sharing ignored build artifacts under
`target/generated-project-tests/`.

Licensed under Apache-2.0; see [LICENSE](LICENSE).
