#!/usr/bin/env python3
"""Require measured 100% line/function/region coverage for Experiment 011 middleware."""
import json
from pathlib import Path
import sys

report=json.loads(Path(sys.argv[1]).read_text())
matches=[file for data in report["data"] for file in data["files"]
         if file["filename"].replace("\\","/").endswith("/crates/ruvoraq-web/src/middleware.rs")]
if len(matches)!=1:
    raise SystemExit("Expected exactly one production middleware coverage entry.")
summary=matches[0]["summary"]
for metric in ["lines","functions","regions"]:
    result=summary[metric]
    print(f"{metric}: {result['covered']}/{result['count']} ({result['percent']:.2f}%)")
    if not result["count"] or result["covered"]!=result["count"]:
        raise SystemExit("Middleware coverage gate failed; cover the missing behavior before releasing.")
print("100% middleware coverage gate passed. This is not a whole-workspace or branch-coverage claim.")
