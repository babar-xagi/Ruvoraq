#!/usr/bin/env python3
"""Verify Experiment 011 through a real CLI-launched temporary application."""
import concurrent.futures
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import uuid

APP=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location("shared_smoke",APP/"tests/smoke.py")
shared=importlib.util.module_from_spec(spec);spec.loader.exec_module(shared)

def main():
    with tempfile.TemporaryDirectory(prefix="ruvoraq-middleware-smoke-") as directory:
        project=Path(directory)/"api";project.mkdir()
        shutil.copytree(APP/"src",project/"src",copy_function=shutil.copy)
        shutil.copytree(APP/"migrations",project/"migrations")
        for name in ["Cargo.toml","Cargo.lock"]:shutil.copy2(APP/name,project/name)
        manifest_path = project / "Cargo.toml"
        manifest_path.write_text(manifest_path.read_text().replace('path = "../../crates/ruvoraq"', 'path = "' + str((APP / "../../crates/ruvoraq").resolve()) + '"'))
        main=project/"src/main.rs"
        main.write_text(main.read_text()+'''\n#[get("/middleware-test/slow")]
async fn slow() -> &'static str {
    std::future::pending::<()>().await;
    "never"
}
''')
        env=os.environ.copy()
        for key in ["DATABASE_URL","MIGRATIONS_DIR","RUVORAQ_APP_NAME","RUVORAQ_HOST","RUVORAQ_PORT","RUVORAQ_DOCS","RUVORAQ_REQUEST_ID","RUVORAQ_REQUEST_LOG","RUVORAQ_REQUEST_TIMEOUT_MS","FRONTEND_ORIGIN"]:env.pop(key,None)
        env.update(CARGO=shared.CARGO,CARGO_NET_OFFLINE="true",CARGO_TARGET_DIR=str(APP/"target/live-tests"),RUVORAQ_PORT="0",RUVORAQ_REQUEST_LOG="true",RUVORAQ_REQUEST_TIMEOUT_MS="50")
        with shared.Server(project,env) as server:
            address=server.address
            headers,_=shared.expect(address,"request ID propagation","GET","/",200,text="Hello",headers={"x-request-id":"live-trace-1"})
            assert headers["x-request-id"]=="live-trace-1"
            headers,_=shared.expect(address,"unsafe request ID replacement","GET","/",200,text="Hello",headers={"x-request-id":"bad secret"})
            assert uuid.UUID(headers["x-request-id"]).version==4
            headers,_=shared.expect(address,"allowed browser origin","GET","/",200,text="Hello",headers={"origin":"http://localhost:3000"})
            assert headers["access-control-allow-origin"]=="http://localhost:3000" and headers["access-control-expose-headers"]=="x-request-id"
            headers,_=shared.expect(address,"disallowed browser origin","GET","/",200,text="Hello",headers={"origin":"http://untrusted"})
            assert "access-control-allow-origin" not in headers
            status,headers,body=shared.request(address,"OPTIONS","/",headers={"origin":"http://localhost:3000","access-control-request-method":"POST","access-control-request-headers":"content-type,x-request-id"})
            assert status==200 and not body and "x-request-id" in headers and headers["access-control-allow-origin"]=="http://localhost:3000"
            shared.passed("CORS preflight retains request ID")
            headers,_=shared.expect(address,"timed out request uses framework errors","GET","/middleware-test/slow",408,error="request_timeout",headers={"origin":"http://localhost:3000"})
            assert "x-request-id" in headers and headers["access-control-allow-origin"]=="http://localhost:3000"
            shared.expect(address,"log-safe dynamic path","GET","/school/students/private-secret?token=private-secret",400,error="invalid_path",headers={"authorization":"Bearer private-secret"})
            shared.expect(address,"log-safe body","POST","/school/students",422,body={"name":"","extra":"private-secret"},error="validation_error")
            with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
                responses=list(pool.map(lambda _:shared.request(address,"GET","/"),range(32)))
            assert all(status==200 for status,_,_ in responses)
            assert len({headers["x-request-id"] for _,headers,_ in responses})==32
            shared.passed("concurrent request identifiers are unique")
            server.stop(signal.SIGINT)
            server.log.seek(0);output=server.log.read()
            records=[json.loads(line) for line in output.splitlines() if line.startswith('{')]
            assert records and "private-secret" not in output
            fields=[r["fields"] for r in records if r.get("target")=="ruvoraq::http"]
            assert any(f["request_id"]=="live-trace-1" and f["status"]==200 for f in fields)
            assert any(f["route"]=="/school/students/{id}" for f in fields)
            assert any(f["status"]==408 for f in fields)
            shared.passed("JSON tracing records safe route patterns, status and correlation")
        with shared.Server(project,dict(env,RUVORAQ_REQUEST_ID="false",RUVORAQ_REQUEST_LOG="false")) as server:
            headers,_=shared.expect(server.address,"environment disables request IDs","GET","/",200,text="Hello")
            assert "x-request-id" not in headers
            server.stop(signal.SIGTERM);server.log.seek(0)
            assert not any(line.startswith('{') for line in server.log.read().splitlines())
            shared.passed("environment disables request logging")
        for key in ["RUVORAQ_REQUEST_ID","RUVORAQ_REQUEST_LOG","RUVORAQ_REQUEST_TIMEOUT_MS","FRONTEND_ORIGIN"]:
            invalid=dict(env);invalid[key]="private-secret"
            result=subprocess.run([shared.CLI,"dev"],cwd=project,env=invalid,capture_output=True,text=True,timeout=180)
            assert result.returncode!=0 and "Ready" not in result.stdout and "private-secret" not in result.stderr
            shared.passed("redacted startup policy error: "+key)
        invalid=dict(env,RUVORAQ_REQUEST_TIMEOUT_MS="0")
        result=subprocess.run([shared.CLI,"dev"],cwd=project,env=invalid,capture_output=True,text=True,timeout=180)
        assert result.returncode!=0 and "greater than zero" in result.stderr and not result.stdout
        shared.passed("zero request timeout fails before startup")
    print(f"All {shared.CHECKS} middleware live checks passed.",flush=True)

if __name__=="__main__":main()
