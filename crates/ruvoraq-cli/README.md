# Ruvoraq CLI

Generate, organize, run and migrate Ruvoraq Rust API projects.

```bash
cargo install ruvoraq-cli --version 0.1.0 --locked
ruvoraq --help
ruvoraq new hello_api
cd hello_api
ruvoraq dev
```

Commands: `new <project-name>`, `add app <name>`, `dev`, `migrate`,
`migrate --status`, `--help`, and `--version`.

`new` creates exactly three files and uses the matching crates.io framework
version. It validates package names and refuses non-empty target directories.
Development mode builds/runs with Cargo; file watching is not implemented.

```bash
# Optional PostgreSQL migration support in the CLI:
cargo install ruvoraq-cli --version 0.1.0 --features postgres --locked
```

Migration commands require `DATABASE_URL`. `MIGRATIONS_DIR` defaults to
`migrations`. Application database features must be enabled separately.
For framework source development, set `RUVORAQ_FRAMEWORK_PATH` to the local
`crates/ruvoraq` directory before running `new`; otherwise no checkout is needed.

This is an experimental 0.1.0 release. The framework is developed in verified
increments and is not yet a complete production platform.

Documentation: [user guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/user_guid.md),
[developer guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/developer_guid.md),
[testing guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/testing_guid.md).

Licensed under Apache-2.0. Source: [Ruvoraq](https://github.com/babar-xagi/Ruvoraq).
