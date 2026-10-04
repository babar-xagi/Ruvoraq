#!/usr/bin/env python3
"""Run the persistent API on an owned temporary database and project copy."""
import concurrent.futures
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile

APP=Path(__file__).resolve().parents[1]
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location("shared_smoke",APP/"tests/smoke.py")
shared=importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
def main():
    assert shared.CLI and shared.CARGO
    with tempfile.TemporaryDirectory(prefix="ruvoraq-sqlite-smoke-") as folder:
        directory=Path(folder);project=directory/"api";project.mkdir()
        shutil.copytree(APP/"src",project/"src",copy_function=shutil.copy)
        shutil.copytree(APP/"migrations/sqlite",project/"migrations")
        for name in ["Cargo.toml","Cargo.lock"]:shutil.copy2(APP/name,project/name)
        environment=os.environ.copy()
        for key in ["RUVORAQ_APP_NAME","RUVORAQ_HOST","RUVORAQ_PORT","RUVORAQ_DOCS","DATABASE_URL", "MIGRATIONS_DIR"]:
            environment.pop(key,None)
        database=directory/"notes.sqlite"
        environment.update(CARGO=shared.CARGO,CARGO_NET_OFFLINE="true",CARGO_TARGET_DIR=str(APP/"target/live-tests"),
                           RUVORAQ_PORT="0",MIGRATIONS_DIR="migrations",DATABASE_URL="sqlite://"+str(database))
        status=subprocess.run([shared.CLI,"migrate","--status"],cwd=project,env=environment,capture_output=True,text=True,timeout=180)
        assert status.returncode==0 and "1  pending" in status.stdout
        with sqlite3.connect(database) as connection:
            assert not connection.execute("SELECT name FROM sqlite_master WHERE name='_sqlx_migrations'").fetchall()
        shared.passed("CLI status leaves fresh migration history untouched")
        for count in [1,0]:
            result=subprocess.run([shared.CLI,"migrate"],cwd=project,env=environment,capture_output=True,text=True,timeout=180)
            assert result.returncode==0 and f"Applied {count} migration(s)." in result.stdout
        shared.passed("CLI applies initial migration once and repeats safely")
        with shared.Server(project,environment) as server:
            address=server.address
            shared.expect(address,"persistent API greeting","GET","/",200,text="Hello")
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
        (project/"migrations/0002_notes_title_index.sql").write_text("CREATE INDEX notes_title_index ON notes(title);\n")
        status=subprocess.run([shared.CLI,"migrate","--status"],cwd=project,env=environment,capture_output=True,text=True,timeout=180)
        assert status.returncode==0 and "1  applied" in status.stdout and "2  pending" in status.stdout
        shared.passed("CLI reports appended migration as pending")
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
        with sqlite3.connect(database) as connection:
            versions=connection.execute("SELECT version FROM _sqlx_migrations ORDER BY version").fetchall()
            assert versions==[(1,),(2,)]
        shared.passed("async startup applies appended migration and retains version history")
        broken=project/"migrations/0003_broken.sql"
        broken.write_text("CREATE TABLE temporary_migration_table (id INTEGER); INSERT INTO private_secret_missing_table VALUES (1);")
        result=subprocess.run([shared.CLI,"dev"],cwd=project,env=environment,capture_output=True,text=True,timeout=180)
        assert result.returncode!=0 and "migration 3 failed" in result.stderr and not result.stdout
        assert "private_secret" not in result.stderr
        with sqlite3.connect(database) as connection:
            assert not connection.execute("SELECT name FROM sqlite_master WHERE name='temporary_migration_table'").fetchall()
            assert connection.execute("SELECT COUNT(*) FROM notes").fetchone()==(9,)
        shared.passed("failed startup migration rolls back DDL without losing notes or exposing SQL")
        broken.write_text("CREATE INDEX notes_id_title_index ON notes(id,title);")
        result=subprocess.run([shared.CLI,"migrate"],cwd=project,env=environment,capture_output=True,text=True,timeout=180)
        assert result.returncode==0 and "Applied 1 migration(s)." in result.stdout
        shared.passed("fixed pending migration can be retried with the CLI")
        initial=project/"migrations/0001_create_notes.sql"
        original=initial.read_text()
        initial.write_text(original+"\n-- changed after applying\n")
        result=subprocess.run([shared.CLI,"dev"],cwd=project,env=environment,capture_output=True,text=True,timeout=180)
        assert result.returncode!=0 and "has changed" in result.stderr and not result.stdout
        shared.passed("checksum drift fails startup before serving requests")
        initial.write_text(original)
        invalid=dict(environment,DATABASE_URL="postgres://private-secret")
        result=subprocess.run([shared.CLI,"dev"],cwd=project,env=invalid,capture_output=True,text=True,timeout=180)
        assert result.returncode!=0 and not result.stdout
        assert "DATABASE_URL" in result.stderr and "private-secret" not in result.stderr
        shared.passed("bad database configuration fails before server startup without printing the URL")
    print(f"All {shared.CHECKS} persistent API checks passed.",flush=True)
if __name__=="__main__":main()
