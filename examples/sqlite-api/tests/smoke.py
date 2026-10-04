#!/usr/bin/env python3
"""Run the persistent API on an owned temporary database and project copy."""
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

APP=Path(__file__).resolve().parents[1]
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location("shared_smoke",APP.parent/"app/tests/smoke.py")
shared=importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
def main():
    assert shared.CLI and shared.CARGO
    with tempfile.TemporaryDirectory(prefix="ruvoraq-sqlite-smoke-") as folder:
        directory=Path(folder);project=directory/"api";project.mkdir()
        shutil.copytree(APP/"src",project/"src")
        for name in ["Cargo.toml","Cargo.lock"]:shutil.copy2(APP/name,project/name)
        environment=os.environ.copy()
        for key in ["RUVORAQ_APP_NAME","RUVORAQ_HOST","RUVORAQ_PORT","RUVORAQ_DOCS","DATABASE_URL"]:
            environment.pop(key,None)
        database=directory/"notes.sqlite"
        environment.update(CARGO=shared.CARGO,CARGO_NET_OFFLINE="true",CARGO_TARGET_DIR=str(APP/"target/live-tests"),
                           RUVORAQ_PORT="0",DATABASE_URL="sqlite://"+str(database))
        with shared.Server(project,environment) as server:
            address=server.address
            shared.expect(address,"persistent API greeting","GET","/",200,text="Persistent notes API")
            shared.expect(address,"initial empty database","GET","/notes",200,expected=[])
            shared.expect(address,"create a validated persistent note","POST","/notes",201,body={"title":" First "},
                          expected={"id":1,"title":"First"})
            shared.expect(address,"read persistent note","GET","/notes/1",200,expected={"id":1,"title":"First"})
            shared.expect(address,"update persistent note","PUT","/notes/1",200,body={"title":"Persist me"},
                          expected={"id":1,"title":"Persist me"})
            title="'); DROP TABLE notes; --"
            shared.expect(address,"bound SQL-like input remains ordinary text","POST","/notes",201,body={"title":title},
                          expected={"id":2,"title":title})
            shared.expect(address,"blank title validation","POST","/notes",422,body={"title":" "},error="validation_error")
            shared.expect(address,"title length validation","POST","/notes",422,body={"title":"a"*121},error="validation_error")
            shared.expect(address,"bad path parameter","GET","/notes/bad",400,error="invalid_path")
            _,document=shared.expect(address,"persistent API OpenAPI","GET","/openapi.json",200)
            assert document["paths"]["/notes"]["post"]["responses"]["201"]
            assert document["paths"]["/notes/{id}"]["delete"]["responses"]["204"]
            assert document["paths"]["/notes"]["post"]["requestBody"]["content"]["application/json"]["schema"]["properties"]["title"]["type"]=="string"
            shared.passed("database handlers have typed OpenAPI schemas and statuses")
            with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
                results=list(executor.map(lambda n:shared.request(address,"POST","/notes",body={"title":f"Note {n}"}),range(8)))
            assert all(status==201 for status,_,_ in results)
            assert sorted(json.loads(body)["id"] for _,_,body in results)==list(range(3,11))
            shared.passed("eight concurrent writes receive unique persistent IDs")
            server.stop(signal.SIGINT)
        assert database.is_file()
        shared.passed("SQLite creates the configured database file")
        with shared.Server(project,environment) as server:
            address=server.address
            shared.expect(address,"record survives server restart","GET","/notes/1",200,expected={"id":1,"title":"Persist me"})
            _,rows=shared.expect(address,"all records survive restart","GET","/notes",200)
            assert len(rows)==10
            shared.passed("SQL-looking value never removed the table")
            shared.expect(address,"persistent deletion","DELETE","/notes/1",204,empty=True)
            shared.expect(address,"deleted note gives 404","GET","/notes/1",404,error="not_found")
            shared.expect(address,"repeated deletion gives 404","DELETE","/notes/1",404,error="not_found")
            shared.expect(address,"missing update gives 404","PUT","/notes/999",404,body={"title":"Missing"},error="not_found")
            server.stop(signal.SIGTERM)
        invalid=dict(environment,DATABASE_URL="postgres://private-secret")
        result=subprocess.run([shared.CLI,"dev"],cwd=project,env=invalid,capture_output=True,text=True,timeout=60)
        assert result.returncode!=0 and not result.stdout
        assert "DATABASE_URL" in result.stderr and "private-secret" not in result.stderr
        shared.passed("bad database configuration fails before server startup without printing the URL")
    print(f"All {shared.CHECKS} persistent API checks passed.",flush=True)
if __name__=="__main__":main()
