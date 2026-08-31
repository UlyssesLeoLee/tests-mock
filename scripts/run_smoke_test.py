#!/usr/bin/env python3
"""End-to-end smoke test for the 5 mock backends (docs-only stub mode).

Phase 1 (docs-only) behaviour:
  - Runs ``cargo test --workspace`` to verify all trait stubs compile + pass.
  - For each of 5 mock backends × 5 methods, records a status row.
    Phase 1 status is ``skipped_unimplemented`` (trait stub exists, real impl
    lands in Phase 2). ``pass`` / ``fail`` will be set by Phase 2 once
    in-process implementations land.
  - Writes report to ``$TEMP/tests-mock-smoke-report.json``.

Usage:
    python3 run_smoke_test.py [--report-file PATH]
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
from typing import Dict, List

# Force UTF-8 stdout on Windows (default GBK can't print ✓/✗)
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

METHODS: Dict[str, List[str]] = {
    "s3": ["head_bucket", "put_object", "get_object", "list_objects", "delete_object"],
    "vault": ["get", "set", "delete", "list", "rotate"],
    "git": ["init_bare", "receive_pack", "upload_pack", "get_refs", "list_refs"],
    "ai": ["complete", "embed", "stream_token", "cancel", "usage_stats"],
}


def _default_report_file() -> str:
    return os.path.join(tempfile.gettempdir(), "tests-mock-smoke-report.json")


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--report-file", default=_default_report_file())
    args = parser.parse_args(argv)

    report_file = pathlib.Path(args.report_file)
    test_log = REPO_ROOT / "target-smoke-test.log"

    print("→ Running cargo test --workspace", flush=True)
    started = time.monotonic()
    with test_log.open("w", encoding="utf-8") as out, test_log.with_suffix(
        ".log.err"
    ).open("w", encoding="utf-8") as err:
        proc = subprocess.run(
            ["cargo", "test", "--workspace", "--quiet"],
            cwd=str(REPO_ROOT),
            stdout=out,
            stderr=err,
            check=False,
        )
    cargo_latency_ms = int((time.monotonic() - started) * 1000)
    cargo_exit = proc.returncode

    results: list[dict] = []
    for backend, methods in METHODS.items():
        for method in methods:
            results.append(
                {
                    "backend": backend,
                    "method": method,
                    "status": "skipped_unimplemented"
                    if cargo_exit == 0
                    else "fail",
                    "latency_ms": 0,
                    "note": "docs-only phase: trait stub exists, real impl lands in Phase 2",
                }
            )

    results.append(
        {
            "backend": "core",
            "method": "cargo_test_workspace",
            "status": "pass" if cargo_exit == 0 else "fail",
            "latency_ms": cargo_latency_ms,
            "note": f"cargo test --workspace exit={cargo_exit}",
        }
    )

    pass_count = sum(1 for r in results if r["status"] == "pass")
    fail_count = sum(1 for r in results if r["status"] == "fail")
    skip_count = sum(1 for r in results if r["status"] == "skipped_unimplemented")

    report = {
        "smoke_at_unix_ms": int(time.time() * 1000),
        "report_file": str(report_file),
        "mode": "in-process (docs-only stub)",
        "cargo_test_log": str(test_log),
        "cargo_exit": cargo_exit,
        "summary": {
            "pass": pass_count,
            "fail": fail_count,
            "skipped": skip_count,
            "total": len(results),
        },
        "results": results,
    }
    report_file.write_text(
        json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8"
    )

    if cargo_exit != 0:
        print(f"✗ cargo test failed (exit={cargo_exit})", file=sys.stderr)
        return 1

    print("✓ Smoke test complete")
    print(
        f"  pass: {pass_count} / fail: {fail_count} / "
        f"skipped (unimplemented): {skip_count}"
    )
    print(f"  report: {report_file}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
