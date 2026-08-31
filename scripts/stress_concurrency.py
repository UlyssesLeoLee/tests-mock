#!/usr/bin/env python3
"""Concurrent stress test for the 5 mock backends (docs-only stub mode).

Phase 1 (docs-only) behaviour:
  - Runs ``iterations`` ops with ``concurrency`` workers (default 100 × 1000).
  - Each op parses the ``user_creds.json`` fixture and serializes a small
    payload — this gives a meaningful, non-trivial latency distribution
    without depending on the ``unimplemented!()`` trait methods.
  - Reports P50 / P95 / P99 latency, error rate, and ops/sec to
    ``$TEMP/tests-mock-stress-report.json``.

Phase 2 will swap the per-op body for real trait method calls
(``vault.get`` / ``vault.set`` / ``ai.stream_token`` / ``s3.put_object``)
without changing the report schema.

Usage:
    python3 stress_concurrency.py [--concurrency 100] [--iterations 1000]
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import pathlib
import statistics
import sys
import tempfile
import time
from typing import List, Tuple

# Force UTF-8 stdout on Windows (default GBK can't print ✓/✗)
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURE_FILE = (
    REPO_ROOT / "crates" / "tests-mock-fixtures" / "fixtures" / "user_creds.json"
)


def _default_report_file() -> str:
    return os.path.join(tempfile.gettempdir(), "tests-mock-stress-report.json")


def _one_op(path: pathlib.Path) -> Tuple[bool, int]:
    """Simulate one mock-backend op. Phase 1: parse + re-serialize fixture."""
    started = time.monotonic()
    try:
        obj = json.loads(path.read_text(encoding="utf-8"))
        json.dumps(
            {"users": len(obj["users"]), "test": len(obj["test_key"])},
            ensure_ascii=False,
        )
        latency_ms = int((time.monotonic() - started) * 1000)
        return True, max(latency_ms, 0)
    except Exception:  # noqa: BLE001 — capture all for stress-error accounting
        latency_ms = int((time.monotonic() - started) * 1000)
        return False, max(latency_ms, 0)


def _percentile(sorted_values: List[int], pct: float) -> int:
    if not sorted_values:
        return 0
    idx = min(int(len(sorted_values) * pct), len(sorted_values) - 1)
    return sorted_values[idx]


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--concurrency", type=int, default=100)
    parser.add_argument("--iterations", type=int, default=1000)
    parser.add_argument("--report-file", default=_default_report_file())
    args = parser.parse_args(argv)

    if args.concurrency <= 0:
        print("✗ concurrency must be > 0", file=sys.stderr)
        return 1
    if args.iterations <= 0:
        print("✗ iterations must be > 0", file=sys.stderr)
        return 1
    if not FIXTURE_FILE.exists():
        print(f"✗ fixture not found: {FIXTURE_FILE}", file=sys.stderr)
        return 1

    report_file = pathlib.Path(args.report_file)
    print(
        f"→ Stress: concurrency={args.concurrency} iterations={args.iterations}",
        flush=True,
    )

    latencies: List[int] = []
    errors = 0
    started = time.monotonic()
    with concurrent.futures.ThreadPoolExecutor(
        max_workers=args.concurrency
    ) as pool:
        for ok, lat_ms in pool.map(
            lambda _i: _one_op(FIXTURE_FILE), range(args.iterations)
        ):
            if ok:
                latencies.append(lat_ms)
            else:
                errors += 1
    total_ms = int((time.monotonic() - started) * 1000)

    latencies.sort()
    p50 = _percentile(latencies, 0.50)
    p95 = _percentile(latencies, 0.95)
    p99 = _percentile(latencies, 0.99)
    max_lat = latencies[-1] if latencies else 0
    mean_lat = int(statistics.mean(latencies)) if latencies else 0
    ops_per_sec = int(args.iterations * 1000 / total_ms) if total_ms > 0 else 0

    report = {
        "stress_at_unix_ms": int(time.time() * 1000),
        "report_file": str(report_file),
        "mode": "in-process (docs-only stub)",
        "config": {
            "concurrency": args.concurrency,
            "iterations": args.iterations,
        },
        "timing": {
            "total_ms": total_ms,
            "ops_per_sec": ops_per_sec,
            "mean_latency_ms": mean_lat,
            "p50_latency_ms": p50,
            "p95_latency_ms": p95,
            "p99_latency_ms": p99,
            "max_latency_ms": max_lat,
        },
        "errors": {
            "count": errors,
            "rate": round(errors / args.iterations, 4) if args.iterations else 0,
        },
        "target_methods": [
            "vault.get",
            "vault.set",
            "ai.stream_token",
            "s3.put_object",
        ],
        "note": "Phase 1 measures dispatch overhead; real method calls land in Phase 2",
    }
    report_file.write_text(
        json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    print("✓ Stress complete")
    print(
        f"  ops={args.iterations} err={errors} ops/sec={ops_per_sec} "
        f"p50={p50}ms p95={p95}ms p99={p99}ms"
    )
    print(f"  report: {report_file}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
