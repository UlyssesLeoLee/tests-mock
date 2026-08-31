#!/usr/bin/env python3
"""Initialize the tests-mock environment (docs-only stub mode).

Phase 1 (docs-only) behaviour:
  - Writes a state file with mode=in-process and 4 backend slots.
  - Performs a health check by running ``cargo check --workspace`` to confirm
    all trait stubs compile.
  - On failure, removes the state file (rollback).

Phase 2 (B 子代理) will extend this script to optionally launch docker
compose for fake minIO / postgres / gitea / llama.cpp.

Usage:
    python3 init_mock_env.py [--docker] [--state-file PATH]
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
from typing import List

# Force UTF-8 stdout on Windows (default GBK can't print ✓/✗)
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")


REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


def _default_state_file() -> str:
    return os.path.join(tempfile.gettempdir(), "tests-mock-state.json")


def _health_check(check_log: pathlib.Path) -> int:
    """Run ``cargo check --workspace`` and return its exit code."""
    print("→ Health check: cargo check --workspace", flush=True)
    err_log = check_log.with_suffix(".log.err")
    with check_log.open("w", encoding="utf-8") as out, err_log.open(
        "w", encoding="utf-8"
    ) as err:
        proc = subprocess.run(
            ["cargo", "check", "--workspace", "--quiet"],
            cwd=str(REPO_ROOT),
            stdout=out,
            stderr=err,
            check=False,
        )
    return proc.returncode


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--docker",
        action="store_true",
        help="Use docker mode (Phase 2 only; ignored in docs-only phase).",
    )
    parser.add_argument(
        "--state-file",
        default=_default_state_file(),
        help="Override state file path.",
    )
    args = parser.parse_args(argv)

    mode = "docker" if args.docker else "in-process"
    state_file = pathlib.Path(args.state_file)
    check_log = REPO_ROOT / "target-cargo-check.log"

    rc = _health_check(check_log)
    if rc != 0:
        print(f"✗ cargo check failed (exit={rc})", file=sys.stderr)
        if state_file.exists():
            state_file.unlink()
        return 1

    state = {
        "mode": mode,
        "pid": os.getpid(),
        "started_at_unix_ms": int(time.time() * 1000),
        "backends": ["s3", "vault", "git", "ai"],
        "state_file": str(state_file),
        "cargo_check_log": str(check_log),
    }
    state_file.write_text(
        json.dumps(state, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    print(f"✓ Mock environment initialized (mode={mode})")
    print(f"  state: {state_file}")
    print("  backends: s3, vault, git, ai (all docs-only stubs)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
