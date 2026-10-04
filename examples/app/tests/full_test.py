#!/usr/bin/env python3
"""Run all example checks; --postgres owns an ephemeral Docker test database."""
import argparse
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import tempfile
import time
import tomllib
import uuid

APP = Path(__file__).resolve().parents[1]
MANIFEST = tomllib.loads((APP / "Cargo.toml").read_text())
FACADE = (APP / MANIFEST["dependencies"]["ruvoraq"]["path"]).resolve()
ROOT = FACADE.parents[1]


def run(args, *, cwd=APP, env=None):
    subprocess.run(args, cwd=cwd, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--postgres", action="store_true", help="also test PostgreSQL using Docker postgres:17-alpine")
    args = parser.parse_args()
    if not shutil.which("cargo") or not shutil.which("ruvoraq"):
        parser.error("Install Rust and the current Ruvoraq CLI; run inside WSL/Linux.")
    environment = os.environ.copy()
    for name in ["DATABASE_URL", "MIGRATIONS_DIR", "RUVORAQ_HOST", "RUVORAQ_PORT", "RUVORAQ_DOCS", "RUVORAQ_APP_NAME", "SCHOOL_GREETING"]:
        environment.pop(name, None)
    environment["CARGO_TARGET_DIR"] = str(APP / "target/live-tests")
    run(["cargo", "build", "--locked"], env=environment)
    run(["python3", "tests/smoke.py"], env=environment)
    run(["python3", "tests/sqlite.py"], env=environment)
    if not args.postgres:
        print("SQLite/general checks complete. PostgreSQL was not requested; run again with --postgres for Experiments 001–010.")
        return
    if not shutil.which("docker"):
        parser.error("--postgres requires a working Docker daemon.")
    container = "ruvoraq-full-test-" + uuid.uuid4().hex[:12]
    with tempfile.TemporaryDirectory(prefix="ruvoraq-postgres-") as directory:
        credentials = Path(directory) / "container.env"
        password = secrets.token_hex(24)
        credentials.write_text("POSTGRES_USER=ruvoraq_test\nPOSTGRES_DB=ruvoraq_test\nPOSTGRES_PASSWORD=" + password + "\n")
        credentials.chmod(0o600)
        started = False
        try:
            subprocess.run(["docker", "run", "--rm", "-d", "--name", container,
                            "--env-file", str(credentials), "-p", "127.0.0.1::5432", "postgres:17-alpine"],
                           check=True, stdout=subprocess.DEVNULL)
            started = True
            credentials.unlink()
            deadline = time.monotonic() + 30
            while subprocess.run(["docker", "exec", container, "pg_isready", "-h", "127.0.0.1", "-U", "ruvoraq_test", "-d", "ruvoraq_test"],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
                if time.monotonic() >= deadline:
                    raise RuntimeError("Owned PostgreSQL test container did not become ready.")
                time.sleep(0.25)
            port = subprocess.check_output(["docker", "port", container, "5432/tcp"], text=True).strip().rsplit(":", 1)[1]
            environment["RUVORAQ_TEST_POSTGRES_URL"] = "postgres://ruvoraq_test:" + password + "@127.0.0.1:" + port + "/ruvoraq_test"
            run(["cargo", "test", "-p", "ruvoraq-db", "--all-features", "--locked", "--test", "postgres", "--", "--ignored"], cwd=ROOT, env=environment)
            run(["python3", "tests/postgres.py"], env=environment)
            print("All Experiments 001–010 example checks and five real PostgreSQL framework tests passed.")
        finally:
            if started:
                subprocess.run(["docker", "stop", "--time", "5", container], check=True, stdout=subprocess.DEVNULL)


if __name__ == "__main__":
    main()
