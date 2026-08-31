#!/usr/bin/env python3
"""Tear down the tests-mock environment (docs-only stub mode).

Idempotent: re-running on an already-clean environment is a no-op.

Removes the state file, smoke/seed/stress reports, and the cargo log
artifacts created by ``init_mock_env`` / ``run_smoke_test``.

Usage:
    python3 cleanup_mock_env.py [--state-file PATH]
"""

from __future__ import annotations

import argparse
import os
import pathlib
import sys
import tempfile
from typing import List

# Force UTF-8 stdout on Windows (default GBK can't print ✓/✗)
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


def _default_state_file() -> str:
    return os.path.join(tempfile.gettempdir(), "tests-mock-state.json")


def _candidate_paths(state_file: pathlib.Path) -> List[pathlib.Path]:
    temp_root = state_file.parent
    return [
        state_file,
        temp_root / "tests-mock-smoke-report.json",
        temp_root / "tests-mock-seed-report.json",
        temp_root / "tests-mock-stress-report.json",
        REPO_ROOT / "target-cargo-check.log",
        REPO_ROOT / "target-cargo-check.log.err",
        REPO_ROOT / "target-smoke-test.log",
        REPO_ROOT / "target-smoke-test.log.err",
    ]


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--state-file", default=_default_state_file())
    args = parser.parse_args(argv)

    state_file = pathlib.Path(args.state_file)
    removed: list[str] = []
    kept: list[str] = []
    for path in _candidate_paths(state_file):
        if not path.exists():
            continue
        try:
            path.unlink()
            removed.append(str(path))
        except OSError as exc:
            kept.append(f"{path} (error: {exc})")

    print("✓ Cleanup complete")
    print(f"  removed ({len(removed)}):")
    for p in removed:
        print(f"    - {p}")
    if kept:
        print("  kept (errors):")
        for p in kept:
            print(f"    - {p}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
