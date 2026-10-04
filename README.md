# Ruvoraq

Experiment 001 is a minimal Rust CLI that scaffolds a standalone Rust binary
project. There are no web, database, authentication, AI, or runtime framework
features yet.

## Usage

Rust 1.85 or newer is required (edition 2024). From the repository root:

```sh
cargo run -p ruvoraq-cli -- --help
cargo run -p ruvoraq-cli -- --version
cargo run -p ruvoraq-cli -- new hello-app
cd hello-app
cargo check
cargo run
```

To use the `ruvoraq` command directly:

```sh
cargo install --path crates/ruvoraq-cli --locked
ruvoraq new my-app
```

`new` creates exactly three files and one source directory:

```text
my-app/
├── Cargo.toml
└── src/
    ├── main.rs
    └── settings.rs
```

`main.rs` prints a greeting using `settings::APP_NAME`. The generator does not
run Cargo, initialize Git, or create a lockfile. Running Cargo later may create
`Cargo.lock` and `target/`.

Names are 1–64 ASCII letters, digits, hyphens or underscores, starting with a
letter or underscore. Rust keywords, Cargo artifact directory names and Windows
device names are rejected. Names are single directory names, not paths.
An existing empty directory is accepted; non-empty directories (including hidden
entries), files and symlink targets are refused. Errors go to stderr with exit
code 1. Files use exclusive creation, and failures attempt to remove only entries
created by that invocation.

## Architecture and verification

The Cargo workspace currently has one member, `crates/ruvoraq-cli`, exposing
the `ruvoraq` binary. `src/main.rs` handles arguments and output;
`src/project.rs` validates names and writes the scaffold. Both the CLI and
generated projects use only the Rust standard library. The generated manifest
contains an empty `[workspace]` so it builds independently, even inside this
repository. The repository lockfile is included for reproducible CLI builds.

Integration tests execute the real CLI in temporary directories, verify the
exact initial file tree, protect existing content, and run offline `cargo check`
on generated projects.

```sh
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
```

Licensed under Apache-2.0; see [LICENSE](LICENSE).
