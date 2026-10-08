#!/usr/bin/env python3
"""Verify the PostgreSQL API against a dedicated, initially empty test database."""
import concurrent.futures
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile

APP = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("shared_smoke", APP / "tests/smoke.py")
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)

def command(project, environment, args):
    return subprocess.run([shared.CLI, *args], cwd=project, env=environment,
                          capture_output=True, text=True, timeout=180)

def main():
    url = os.environ.get("RUVORAQ_TEST_POSTGRES_URL")
    if not url:
        raise SystemExit("Set RUVORAQ_TEST_POSTGRES_URL to a dedicated, empty test database.")
    assert shared.CLI and shared.CARGO
    with tempfile.TemporaryDirectory(prefix="ruvoraq-postgres-smoke-") as folder:
        project = Path(folder) / "api"
        project.mkdir()
        shutil.copytree(APP / "src", project / "src", copy_function=shutil.copy)
        shutil.copytree(APP / "migrations/postgres", project / "migrations")
        for name in ["Cargo.toml", "Cargo.lock"]:
            shutil.copy2(APP / name, project / name)
        manifest_path = project / "Cargo.toml"
        manifest_path.write_text(manifest_path.read_text().replace('path = "../../crates/ruvoraq"', 'path = "' + str((APP / "../../crates/ruvoraq").resolve()) + '"'))
        (project / "Cargo.toml").write_text((project / "Cargo.toml").read_text().replace('default = ["sqlite"]', 'default = ["postgres"]'))
        environment = os.environ.copy()
        for key in ["RUVORAQ_APP_NAME", "RUVORAQ_HOST", "RUVORAQ_PORT", "RUVORAQ_DOCS", "DATABASE_URL", "MIGRATIONS_DIR"]:
            environment.pop(key, None)
        environment.update(CARGO=shared.CARGO, CARGO_NET_OFFLINE="true",
                           CARGO_TARGET_DIR=str(APP / "target/live-tests"),
                           RUVORAQ_PORT="0", MIGRATIONS_DIR="migrations", DATABASE_URL=url)
        result = command(project, environment, ["migrate", "--status"])
        assert result.returncode == 0, "status failed; install CLI with --features postgres and check test database access"
        assert "1  pending" in result.stdout, "use an empty dedicated test database"
        shared.passed("PostgreSQL CLI reports the initial pending migration")
        for count in [1, 0]:
            result = command(project, environment, ["migrate"])
            assert result.returncode == 0 and f"Applied {count} migration(s)." in result.stdout
        shared.passed("PostgreSQL migration applies once and skips a repeat")
        with shared.Server(project, environment) as server:
            address = server.address
            shared.expect(address, "PostgreSQL greeting", "GET", "/", 200, text="Hello")
            shared.expect(address, "initial empty PostgreSQL notes", "GET", "/notes", 200, expected=[])
            shared.expect(address, "create a PostgreSQL note", "POST", "/notes", 201,
                          body={"title": " First "}, expected={"id": 1, "title": "First"})
            shared.expect(address, "read a PostgreSQL note", "GET", "/notes/1", 200,
                          expected={"id": 1, "title": "First"})
            shared.expect(address, "update uses PostgreSQL parameter positions", "PUT", "/notes/1", 200,
                          body={"title": "Persist me"}, expected={"id": 1, "title": "Persist me"})
            title = "'); DROP TABLE notes; --"
            shared.expect(address, "bound SQL-looking PostgreSQL input", "POST", "/notes", 201,
                          body={"title": title}, expected={"id": 2, "title": title})
            shared.expect(address, "PostgreSQL blank title validation", "POST", "/notes", 422,
                          body={"title": " "}, error="validation_error")
            shared.expect(address, "PostgreSQL title length validation", "POST", "/notes", 422,
                          body={"title": "a" * 121}, error="validation_error")
            shared.expect(address, "PostgreSQL invalid path", "GET", "/notes/bad", 400, error="invalid_path")
            _, document = shared.expect(address, "PostgreSQL OpenAPI", "GET", "/openapi.json", 200)
            assert document["paths"]["/notes"]["post"]["responses"]["201"]
            assert document["paths"]["/notes/{id}"]["delete"]["responses"]["204"]
            shared.passed("PostgreSQL route metadata matches HTTP statuses")
            with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
                results = list(executor.map(
                    lambda n: shared.request(address, "POST", "/notes", body={"title": f"Note {n}"}), range(8)))
            assert all(status == 201 for status, _, _ in results)
            assert sorted(json.loads(body)["id"] for _, _, body in results) == list(range(3, 11))
            shared.passed("PostgreSQL pool handles eight concurrent writes")
            server.stop(signal.SIGINT)
        (project / "migrations/0002_notes_title_index.sql").write_text(
            "CREATE INDEX notes_title_index ON notes(title);\n")
        result = command(project, environment, ["migrate", "--status"])
        assert result.returncode == 0 and "1  applied" in result.stdout and "2  pending" in result.stdout
        shared.passed("PostgreSQL status distinguishes applied and appended files")
        with shared.Server(project, environment) as server:
            address = server.address
            shared.expect(address, "PostgreSQL note survives app restart", "GET", "/notes/1", 200,
                          expected={"id": 1, "title": "Persist me"})
            _, rows = shared.expect(address, "PostgreSQL records survive app restart", "GET", "/notes", 200)
            assert len(rows) == 10
            shared.expect(address, "delete PostgreSQL note", "DELETE", "/notes/1", 204, empty=True)
            shared.expect(address, "deleted PostgreSQL note", "GET", "/notes/1", 404, error="not_found")
            shared.expect(address, "missing PostgreSQL update", "PUT", "/notes/999", 404,
                          body={"title": "Missing"}, error="not_found")
            server.stop(signal.SIGTERM)
        result = command(project, environment, ["migrate", "--status"])
        assert result.returncode == 0 and "2  applied" in result.stdout
        shared.passed("PostgreSQL async startup applies the appended migration")
        broken = project / "migrations/0003_failure.sql"
        broken.write_text("CREATE TABLE transient (id BIGINT); INSERT INTO private_secret_missing_table VALUES (1);")
        result = command(project, environment, ["dev"])
        assert result.returncode != 0 and "migration 3 failed" in result.stderr and not result.stdout
        assert "private_secret" not in result.stderr and url not in result.stderr
        shared.passed("PostgreSQL migration failure stops startup and redacts SQL")
        broken.write_text("CREATE TABLE transient (id BIGINT);")
        result = command(project, environment, ["migrate"])
        assert result.returncode == 0 and "Applied 1 migration(s)." in result.stdout
        shared.passed("failed PostgreSQL DDL rolled back and corrected file can run")
        initial = project / "migrations/0001_create_notes.sql"
        initial.write_text(initial.read_text() + "\n-- changed\n")
        result = command(project, environment, ["dev"])
        assert result.returncode != 0 and "has changed" in result.stderr and not result.stdout
        shared.passed("PostgreSQL checksum drift stops startup")
        invalid = dict(environment, DATABASE_URL="postgres://private-secret@127.0.0.1:invalid/db")
        result = command(project, invalid, ["dev"])
        assert result.returncode != 0 and "DATABASE_URL" in result.stderr and not result.stdout
        assert "private-secret" not in result.stderr
        shared.passed("invalid PostgreSQL connection URL is redacted before serving")
    print(f"All {shared.CHECKS} PostgreSQL API checks passed.", flush=True)

if __name__ == "__main__":
    main()
