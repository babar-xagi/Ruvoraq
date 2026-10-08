# Publishing Ruvoraq to crates.io

The next coordinated release is **0.2.0**. The previous 0.1.0 release remains
available; upload verification for the new release follows the same workflow. The existing Git tag
`0.01` is a repository label; Cargo package versions use semantic versioning.
Publishing preparation adds versioned internal dependencies, per-crate metadata,
README/license files and portable registry-based project generation.

## Packages and dependency order

| Package | Role | Internal dependencies |
| --- | --- | --- |
| ruvoraq-config | Typed configuration | None |
| ruvoraq-macros | Route/schema/bootstrap macros | None |
| ruvoraq-db | SQLite/PostgreSQL adapters | None |
| ruvoraq-web | HTTP, services, OpenAPI and Swagger | ruvoraq-config |
| ruvoraq | Public application facade | config, macros, web and optional db |
| ruvoraq-cli | Installed ruvoraq executable | config and db |

All packages are experimental. Successful certificate-verified PostgreSQL TLS
verification remains pending. The all-feature workspace has now been checked
with the declared minimum Rust 1.85.0 toolchain.

## Authenticate locally

Sign in to [crates.io](https://crates.io/) using GitHub, verify your email and
create an API token with publication permission for the packages you will upload.
In your WSL terminal:

```bash
cargo login
```

Paste the token into Cargo's prompt. Do not put it in chat, Git, shell scripts or
documentation. Cargo stores credentials in your local Cargo configuration.
These instructions follow the [Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html).

## Verify before uploading

Run from the repository root with the current stable Cargo toolchain:

```bash
cargo fmt --all --check
cargo check --workspace --all-features --locked
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +1.85.0 check --workspace --all-features --locked
cargo package --workspace --all-features
cargo publish --workspace --all-features --dry-run
```

Use `--allow-dirty` only when deliberately checking local uncommitted preparation
changes. The release should normally be committed before upload. Workspace
packaging stages internal dependencies locally so their archives can be verified
before first publication. Inspect `target/package/*.crate` and confirm each
includes its README/license and required source/assets. The web package must
include Swagger JS, CSS and the upstream license.

The declared minimum Rust build is a separate compatibility check. Use a current
Cargo for multi-package workspace publication.

## Upload

With the prepared release committed and authentication available:

```bash
cargo publish --workspace --all-features --locked
```

Current Cargo publishes workspace dependencies in the required order and waits
for registry availability. For a manual dependency-by-dependency workflow:

```bash
cargo publish -p ruvoraq-config --locked
cargo publish -p ruvoraq-macros --locked
cargo publish -p ruvoraq-db --all-features --locked
cargo publish -p ruvoraq-web --locked
cargo publish -p ruvoraq --all-features --locked
cargo publish -p ruvoraq-cli --all-features --locked
```

Run the next command only after the previous package appears in the registry.
Do not start both workflows. If upload succeeds but registry polling times out,
check the version page before retrying. Published package versions cannot be
overwritten; a later fix requires a new package version.

## Verify the public installation

After all packages are available, use an installation root separate from your
source-development CLI:

```bash
cargo install ruvoraq-cli --version 0.2.0 --features postgres --locked \
  --root "$HOME/ruvoraq-registry-check"
mkdir -p "$HOME/ruvoraq-registry-projects"
cd "$HOME/ruvoraq-registry-projects"
env -u RUVORAQ_FRAMEWORK_PATH \
  "$HOME/ruvoraq-registry-check/bin/ruvoraq" new registry_api
cd registry_api
cargo check
"$HOME/ruvoraq-registry-check/bin/ruvoraq" dev
```

Generated Cargo.toml must reference `ruvoraq` version `0.1.0` without a local
path. Verify `/`, `/docs` and `/openapi.json`. Check SQLite and PostgreSQL
feature resolution separately. Update the README/user guide/project tracker
with the actual registry publication result after these checks succeed.

## Framework source development

The published CLI defaults to a registry dependency. To test unpublished local
framework changes, explicitly select the facade crate while generating:

```bash
RUVORAQ_FRAMEWORK_PATH="$HOME/Ruvoraq/crates/ruvoraq" \
  ruvoraq new local_api
```

The generated manifest contains both the version and local path. The override
is validated before any project is created. It must identify the `ruvoraq`
crate, not the workspace root or another crate. Existing apps are unchanged.

## First-publication result

All six 0.1.0 packages were verified through the crates.io API and a clean
registry CLI installation. The sixth new-crate upload encountered a temporary
crates.io rate limit; only the remaining framework package was retried after
the server-provided time. Source overrides were unset for the public scaffold
and feature/HTTP checks.
