#!/usr/bin/env python3
"""Exercise the installed CLI and the real example server on a temporary copy."""
import concurrent.futures
import hashlib
import http.client
import json
import os
from pathlib import Path
import queue
import shutil
import signal
import subprocess
import tempfile
import threading
import tomllib

APP = Path(__file__).resolve().parents[1]
framework = tomllib.loads((APP / "Cargo.toml").read_text())["dependencies"]["ruvoraq"]["path"]
os.environ.setdefault("RUVORAQ_FRAMEWORK_PATH", str((APP / framework).resolve()))
CLI = shutil.which("ruvoraq")
CARGO = shutil.which("cargo")
CHECKS = 0


def passed(label):
    global CHECKS
    CHECKS += 1
    print(f"PASS {label}", flush=True)


def fingerprint(project):
    files = [project / "Cargo.toml", *sorted((project / "src").rglob("*.rs"))]
    return {str(path.relative_to(project)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in files}


def request(address, method, path, *, body=None, raw=None, headers=None):
    headers = dict(headers or {})
    if body is not None:
        raw = json.dumps(body).encode()
        headers.setdefault("Content-Type", "application/json")
    headers["Connection"] = "close"
    connection = http.client.HTTPConnection(*address, timeout=10)
    try:
        connection.request(method, path, body=raw, headers=headers)
        response = connection.getresponse()
        return response.status, dict((key.lower(), value) for key, value in response.getheaders()), response.read()
    finally:
        connection.close()


def expect(address, label, method, path, status, *, expected=None, error=None,
           text=None, empty=False, **kwargs):
    actual, headers, body = request(address, method, path, **kwargs)
    assert actual == status, (label, actual, body[:500])
    value = None
    if empty:
        assert not body, (label, body)
    elif text is not None:
        assert body.decode() == text, (label, body)
        assert headers["content-type"].startswith("text/plain")
    else:
        assert headers["content-type"] == "application/json", (label, headers)
        value = json.loads(body)
        if expected is not None:
            assert value == expected, (label, value)
        if error:
            assert set(value) == {"error"}, (label, value)
            details = value["error"]
            assert set(details) == {"code", "message", "details"}
            assert details["code"] == error, (label, value)
            assert isinstance(details["message"], str) and isinstance(details["details"], dict)
    passed(label)
    return headers, value


class Server:
    def __init__(self, project, environment):
        self.log = tempfile.TemporaryFile(mode="w+")
        self.lines = []
        self.ready = queue.Queue()
        self.process = subprocess.Popen(
            [CLI, "dev"], cwd=project, env=environment,
            stdout=subprocess.PIPE, stderr=self.log, text=True,
            start_new_session=True,
        )
        self.reader = threading.Thread(target=self.read_output, daemon=True)
        self.reader.start()
        try:
            address = None
            while True:
                line = self.ready.get(timeout=180)
                if line is None:
                    self.log.seek(0)
                    raise AssertionError("Server exited before Ready: " + self.log.read()[-4000:])
                if line.startswith("Server:      http://"):
                    address = line.split("http://", 1)[1]
                if line == "Ready":
                    assert address
                    host, port = address.rsplit(":", 1)
                    self.address = (host, int(port))
                    break
        except BaseException:
            self.close()
            raise

    def read_output(self):
        for line in self.process.stdout:
            line = line.rstrip()
            self.lines.append(line)
            self.ready.put(line)
        self.ready.put(None)

    def send(self, signum):
        try:
            os.killpg(self.process.pid, signum)
        except ProcessLookupError:
            pass

    def stop(self, signum):
        self.send(signum)
        status = self.process.wait(timeout=15)
        self.reader.join(timeout=2)
        self.log.seek(0)
        assert status == 0, self.log.read()[-4000:]
        assert "Stopped" in self.lines, self.lines
        passed("graceful " + signal.Signals(signum).name + " shutdown")

    def close(self):
        if self.process.poll() is None:
            self.send(signal.SIGTERM)
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.send(signal.SIGKILL)
                self.process.wait(timeout=5)
        self.reader.join(timeout=2)
        self.log.close()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()


def exercise(address):
    _, document = expect(address, "OpenAPI JSON endpoint", "GET", "/openapi.json", 200)
    assert document["openapi"] == "3.1.0" and document["info"]["title"] == "app"
    paths = document["paths"]
    assert "/" in paths and "/docs" not in paths and "/openapi.json" not in paths
    assert len(paths) == 12, list(paths)
    passed("all public API paths documented")
    create = paths["/school/students"]["post"]
    schema = create["requestBody"]["content"]["application/json"]["schema"]
    assert schema["properties"]["name"]["type"] == "string"
    assert schema["required"] == ["name"] and schema["additionalProperties"] is False
    assert "201" in create["responses"]
    assert "202" in paths["/billing/requests"]["post"]["responses"]
    assert "204" in paths["/school/students/{id}"]["delete"]["responses"]
    assert "content" not in paths["/school/students/{id}"]["delete"]["responses"]["204"]
    passed("documented models and 201/202/204 statuses")
    parameters = paths["/school/students"]["get"]["parameters"]
    assert {item["name"] for item in parameters} == {"limit", "min_id"}
    assert all(item["required"] is False for item in parameters)
    assert next(item for item in parameters if item["name"] == "limit")["schema"]["default"] == 10
    path_parameters = paths["/school/students/{id}"]["get"]["parameters"]
    assert path_parameters[0]["name"] == "id" and path_parameters[0]["required"] is True
    passed("documented query defaults and required path parameters")
    for path, media in [("/docs", "text/html"), ("/docs/swagger-ui.css", "text/css"),
                        ("/docs/swagger-ui-bundle.js", "application/javascript")]:
        status, headers, body = request(address, "GET", path)
        assert status == 200 and headers["content-type"].startswith(media)
        assert body
        if path == "/docs":
            assert b'SwaggerUIBundle' in body and b'validatorUrl: null' in body
            assert b'https://' not in body
        passed("local interactive docs asset " + path)
    expect(address, "plain-text root", "GET", "/", 200, text="Hello")
    expect(address, "HEAD has no body", "HEAD", "/", 200, empty=True)
    expect(address, "school module registration", "GET", "/school", 200,
           expected={"name": "school", "message": "School API"})
    for method in ["GET", "POST"]:
        expect(address, "stacked " + method + " route", method, "/school/status", 200,
               expected={"status": "ready"})
    expect(address, "empty typed collection", "GET", "/school/students", 200, expected=[])
    expect(address, "201 validated JSON creation", "POST", "/school/students", 201,
           body={"name": " Ada "}, expected={"id": 1, "name": "Ada"})
    headers, _ = expect(address, "typed path and request header", "GET", "/school/students/1", 200,
                        headers={"x-request-id": "request-42"}, expected={"id": 1, "name": "Ada"})
    assert headers["x-request-id"] == "request-42"
    passed("response header conversion")
    expect(address, "PUT typed replacement", "PUT", "/school/students/1", 200,
           body={"name": "Grace"}, expected={"id": 1, "name": "Grace"})
    expect(address, "PATCH typed update", "PATCH", "/school/students/1", 200,
           body={"name": "Linus"}, expected={"id": 1, "name": "Linus"})
    for name, identity in [("Bob", 2), ("Carol", 3)]:
        expect(address, "shared state creates " + name, "POST", "/school/students", 201,
               body={"name": name}, expected={"id": identity, "name": name})
    expect(address, "query limit and optional filter", "GET", "/school/students?limit=1&min_id=2", 200,
           expected=[{"id": 2, "name": "Bob"}])
    expect(address, "DELETE 204 empty response", "DELETE", "/school/students/1", 204, empty=True)
    expect(address, "deleted student gives 404", "GET", "/school/students/1", 404, error="not_found")
    expect(address, "bad path gives 400", "GET", "/school/students/not-a-number", 400, error="invalid_path")
    expect(address, "bad query gives 400", "GET", "/school/students?limit=wrong", 400, error="invalid_query")
    expect(address, "application query validation", "GET", "/school/students?limit=0", 422, error="validation_error")
    expect(address, "malformed JSON gives 400", "POST", "/school/students", 400,
           raw=b"{", headers={"Content-Type": "application/json"}, error="invalid_json")
    for label, body in [("missing field", {}), ("wrong type", {"name": 42}),
                        ("unknown field", {"name": "Ada", "extra": True})]:
        expect(address, label + " gives 422", "POST", "/school/students", 422,
               body=body, error="validation_error")
    _, value = expect(address, "field validation details", "POST", "/school/students", 422,
                      body={"name": " "}, error="validation_error")
    assert value["error"]["details"] == {"name": "Name is required"}
    expect(address, "name length validation", "POST", "/school/students", 422,
           body={"name": "a" * 81}, error="validation_error")
    expect(address, "missing JSON content type", "POST", "/school/students", 415,
           raw=b'{"name":"Ada"}', error="unsupported_media_type")
    expect(address, "wrong content type", "POST", "/school/students", 415,
           raw=b'{"name":"Ada"}', headers={"Content-Type": "text/plain"}, error="unsupported_media_type")
    expect(address, "2 MiB body limit", "POST", "/school/students", 413,
           raw=b"x" * (2 * 1024 * 1024 + 1),
           headers={"Content-Type": "application/json"}, error="payload_too_large")
    expect(address, "bad_request helper", "PATCH", "/school/students/2", 400,
           body={}, error="bad_request")
    expect(address, "missing route JSON 404", "GET", "/missing", 404, error="not_found")
    headers, _ = expect(address, "unsupported method JSON 405", "OPTIONS", "/school/students", 405,
                        error="method_not_allowed")
    assert {"GET", "POST"} <= set(headers["allow"].replace(" ", "").split(","))
    passed("405 Allow header")
    _, value = expect(address, "redacted internal error", "GET", "/school/errors/internal", 500,
                      error="internal_error")
    assert value == {"error": {"code": "internal_error", "message": "Internal server error", "details": {}}}
    expect(address, "billing provider initially empty", "GET", "/billing", 200, expected={"accepted": 0})
    for identity in [1, 2]:
        expect(address, f"202 accepted request {identity}", "POST", "/billing/requests", 202,
               expected={"id": identity, "state": "accepted"})
    expect(address, "provide_shared state persists", "GET", "/billing", 200, expected={"accepted": 2})
    expect(address, "ok response helper", "GET", "/billing/health", 200, expected={"status": "ready"})

    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as executor:
        results = list(executor.map(
            lambda _: request(address, "GET", "/school/visits"), range(24)))
    assert all(status == 200 for status, _, _ in results)
    counts = sorted(json.loads(body)["visits"] for _, _, body in results)
    assert counts == list(range(1, 25)), counts
    passed("24 concurrent requests share the alias-injected counter")
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as executor:
        results = list(executor.map(
            lambda number: request(address, "POST", "/school/students", body={"name": f"Student {number}"}),
            range(12)))
    assert all(status == 201 for status, _, _ in results)
    identities = sorted(json.loads(body)["id"] for _, _, body in results)
    assert identities == list(range(4, 16)), identities
    passed("12 concurrent creates share one in-memory service")


def main():
    assert os.name == "posix", "Run this smoke test inside WSL/Linux."
    assert CLI and CARGO, "Install the current Ruvoraq CLI and Rust/Cargo first."
    help_text = subprocess.check_output([CLI, "--help"], text=True)
    assert "add app <name>" in help_text and "new <project-name>" in help_text
    assert subprocess.check_output([CLI, "--version"], text=True).startswith("ruvoraq ")
    passed("installed CLI help and version")

    with tempfile.TemporaryDirectory(prefix="ruvoraq-example-smoke-") as directory:
        temporary = Path(directory)
        subprocess.run([CLI, "new", "minimal"], cwd=temporary, check=True)
        minimal = temporary / "minimal"
        assert sorted(str(path.relative_to(minimal)) for path in minimal.rglob("*") if path.is_file()) == [
            "Cargo.toml", "src/main.rs", "src/settings.rs"]
        assert "fn main" not in (minimal / "src/main.rs").read_text()
        passed("exact three-file route-only generator")
        generator_env = dict(os.environ, CARGO_TARGET_DIR=str(APP / "target/live-tests"))
        subprocess.run([CARGO, "check", "--offline"], cwd=minimal, env=generator_env, check=True)
        passed("fresh generated three-file project passes cargo check")
        subprocess.run([CLI, "add", "app", "demo"], cwd=minimal, check=True)
        assert all((minimal / ("src/apps/demo/" + name)).is_file() for name in ["mod.rs", "models.rs", "routes.rs", "services.rs"])
        subprocess.run([CARGO, "check", "--offline"], cwd=minimal, env=generator_env, check=True)
        passed("add app creates and wires a compiling module")

        project = temporary / "app"
        project.mkdir()
        shutil.copytree(APP / "src", project / "src", copy_function=shutil.copy)
        shutil.copytree(APP / "migrations", project / "migrations")
        for name in ["Cargo.toml", "Cargo.lock"]:
            if (APP / name).exists():
                shutil.copy2(APP / name, project / name)
        before = fingerprint(project)
        for command, cwd, expected_error in [
            ([CLI, "new", "app"], temporary, "not empty"),
            ([CLI, "add", "app", "school"], project, "refusing to overwrite"),
            ([CLI, "add", "app", "../escape"], project, "invalid"),
        ]:
            result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
            assert result.returncode == 1 and not result.stdout, result
            assert expected_error in result.stderr, result.stderr
            assert fingerprint(project) == before
        passed("generator and module overwrite/name protections")

        settings = project / "src/settings.rs"
        source = settings.read_text()
        assert "pub const PORT: u16 = 8000;" in source
        settings.write_text(source.replace("pub const PORT: u16 = 8000;", "pub const PORT: u16 = 0;"))
        environment = os.environ.copy()
        for name in ["RUVORAQ_HOST", "RUVORAQ_PORT", "RUVORAQ_APP_NAME", "RUVORAQ_DOCS", "SCHOOL_GREETING", "DATABASE_URL", "MIGRATIONS_DIR"]:
            environment.pop(name, None)
        environment.update(CARGO=CARGO, CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(APP / "target" / "live-tests"))
        with Server(project, environment) as server:
            exercise(server.address)
            server.stop(signal.SIGINT)
        with Server(project, environment) as server:
            expect(server.address, "services reset on fresh startup", "GET", "/school/students", 200, expected=[])
            expect(server.address, "billing is scoped to the new app instance", "GET", "/billing", 200, expected={"accepted": 0})
            server.stop(signal.SIGTERM)

        dotenv = project / ".env"
        dotenv.write_text('RUVORAQ_APP_NAME="Configured app"\nRUVORAQ_HOST=127.0.0.1\nRUVORAQ_PORT=0\nRUVORAQ_DOCS=false\nSCHOOL_GREETING="Hello from dotenv"\n')
        with Server(project, environment) as server:
            assert "Application: Configured app" in server.lines
            passed("dotenv app name and ephemeral port startup")
            expect(server.address, "fallible configure hook uses the loaded dotenv snapshot", "GET", "/school", 200,
                   expected={"name": "school", "message": "Hello from dotenv"})
            expect(server.address, "dotenv disables docs", "GET", "/docs", 404, error="not_found")
            expect(server.address, "dotenv disables the OpenAPI endpoint", "GET", "/openapi.json", 404, error="not_found")
            server.stop(signal.SIGTERM)
        overrides = dict(environment, RUVORAQ_APP_NAME="Process app", RUVORAQ_DOCS="true", SCHOOL_GREETING="Hello from process")
        with Server(project, overrides) as server:
            assert "Application: Process app" in server.lines
            passed("process environment overrides dotenv app name")
            expect(server.address, "process variables override dotenv service configuration", "GET", "/school", 200,
                   expected={"name": "school", "message": "Hello from process"})
            _, document = expect(server.address, "process override enables OpenAPI", "GET", "/openapi.json", 200)
            assert document["info"]["title"] == "Process app"
            passed("OpenAPI title uses the effective configuration")
            server.stop(signal.SIGTERM)
        for content, variable, secret in [
            ("RUVORAQ_PORT=secret-port\n", "RUVORAQ_PORT", "secret-port"),
            ("RUVORAQ_DOCS=secret-bool\n", "RUVORAQ_DOCS", "secret-bool"),
            ('PASSWORD="private-secret\n', "configuration file", "private-secret"),
        ]:
            dotenv.write_text(content)
            result = subprocess.run([CLI, "dev"], cwd=project, env=environment,
                                    capture_output=True, text=True, timeout=180)
            assert result.returncode != 0 and not result.stdout, result
            assert variable in result.stderr and secret not in result.stderr, result.stderr
            passed("redacted configuration failure before startup: " + variable)
        dotenv.unlink()

        configured = settings.read_text()
        provider = ".provide(SchoolService::with_greeting(greeting))"
        assert configured.count(provider) == 1
        settings.write_text(configured.replace(provider, ""))
        result = subprocess.run([CLI, "dev"], cwd=project, env=environment,
                                capture_output=True, text=True, timeout=180)
        assert result.returncode != 0 and not result.stdout, result
        assert "missing dependency" in result.stderr and "SchoolService" in result.stderr
        assert "App::provide" in result.stderr
        passed("missing provider fails before server startup")
    print(f"All {CHECKS} end-to-end checks passed.", flush=True)


if __name__ == "__main__":
    main()
