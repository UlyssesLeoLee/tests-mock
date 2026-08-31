#!/usr/bin/env python3
"""Load 3 JSON fixtures into the tests-mock backend (docs-only stub mode).

Phase 1 (docs-only) behaviour:
  - Reads the 3 fixture JSON files from ``crates/tests-mock-fixtures/fixtures/``.
  - Validates each one (non-empty + has ``version`` + has expected list field).
  - Writes a seed report to ``$TEMP/tests-mock-seed-report.json`` (or
    ``/tmp/tests-mock-seed-report.json`` on POSIX).
  - Idempotent: re-running updates the report without breaking the env.

Phase 2 will route the loaded data into the actual in-process mock backends.

Usage:
    python3 seed_fixtures.py [--state-file PATH]
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys
import tempfile
import time
from typing import List

# Force UTF-8 stdout on Windows (default GBK can't print ✓/✗)
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURES_DIR = REPO_ROOT / "crates" / "tests-mock-fixtures" / "fixtures"

EXPECTED_FIXTURES = (
    ("user_creds.json", "users"),
    ("repo_metadata.json", "repos"),
    ("ai_response_cache.json", "responses"),
)


def _default_state_file() -> str:
    return os.path.join(tempfile.gettempdir(), "tests-mock-state.json")


def _validate_fixture(path: pathlib.Path, expected_key: str) -> dict:
    raw = path.read_text(encoding="utf-8")
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError as exc:
        print(f"✗ Invalid JSON in {path.name}: {exc}", file=sys.stderr)
        raise SystemExit(1)
    if "version" not in obj:
        print(f"✗ {path.name} missing 'version' field", file=sys.stderr)
        raise SystemExit(1)
    if expected_key not in obj:
        print(f"✗ {path.name} missing '{expected_key}' field", file=sys.stderr)
        raise SystemExit(1)
    return {
        "fixture": path.name,
        "version": obj["version"],
        "key": expected_key,
        "count": len(obj[expected_key]),
        "status": "loaded",
        "path": str(path),
    }


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--state-file", default=_default_state_file())
    args = parser.parse_args(argv)

    state_file = pathlib.Path(args.state_file)
    if not state_file.exists():
        print(f"✗ State file not found: {state_file}", file=sys.stderr)
        print("  Run init_mock_env first.", file=sys.stderr)
        return 1

    results: list[dict] = []
    for name, key in EXPECTED_FIXTURES:
        path = FIXTURES_DIR / name
        if not path.exists():
            print(f"✗ Missing fixture: {path}", file=sys.stderr)
            return 1
        result = _validate_fixture(path, key)
        results.append(result)
        print(f"  ✓ {name} v{result['version']}: {result['count']} entries")

    report = {
        "seeded_at_unix_ms": int(time.time() * 1000),
        "state_file": str(state_file),
        "fixtures": results,
        "total_fixtures": len(results),
        "backend_mode": "in-process (docs-only stub)",
    }
    report_path = state_file.parent / "tests-mock-seed-report.json"
    report_path.write_text(
        json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    print(f"✓ Seeded {len(results)} fixtures")
    print(f"  report: {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
