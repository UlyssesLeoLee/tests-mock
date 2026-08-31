#!/usr/bin/env pwsh
#Requires -Version 7.0
<#
.SYNOPSIS
    End-to-end smoke test for the 4 trait mock backends + 1 core meta entry.

.DESCRIPTION
    Phase 1 (docs-only) behaviour:
      - Runs `cargo test --workspace` to verify all trait stubs compile + pass
      - Runs the loader helper's own integration tests (loader_smoke.rs)
      - For each of 4 trait mock backends (s3 / vault / git / ai) × 5 methods,
        records pass/fail latency (Phase 1: pass = trait stub exists;
        Phase 2: pass = method returns Ok). Total 20 method entries.
      - Adds 1 core meta entry: `cargo test --workspace` overall result.
      - Total: 21 entries (20 method + 1 core).
      - core is NOT a mock backend — it is the shared chassis (error/config/lifecycle)
        tracked separately as a meta entry.
      - Writes report to $env:TEMP/tests-mock-smoke-report.json

.PARAMETER ReportFile
    Override report path. Default: $env:TEMP/tests-mock-smoke-report.json.

.EXAMPLE
    pwsh D:/tests-mock/scripts/run_smoke_test.ps1
#>

[CmdletBinding()]
param(
    [string]$ReportFile
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = Split-Path -Parent $PSScriptRoot
$tempRoot = $env:TEMP
if (-not $tempRoot) { $tempRoot = [System.IO.Path]::GetTempPath() }
if (-not $ReportFile) { $ReportFile = Join-Path $tempRoot 'tests-mock-smoke-report.json' }

# ---------------------------------------------------------------------------
# 1. Run cargo test --workspace
# ---------------------------------------------------------------------------
Write-Host "→ Running cargo test --workspace" -ForegroundColor Cyan
$testLog = Join-Path $repoRoot 'target-smoke-test.log'
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$testProc = Start-Process -FilePath 'cargo' `
    -ArgumentList @('test', '--workspace', '--quiet') `
    -WorkingDirectory $repoRoot `
    -RedirectStandardOutput $testLog `
    -RedirectStandardError "$testLog.err" `
    -NoNewWindow -PassThru -Wait
$sw.Stop()
$cargoExit = $testProc.ExitCode
$cargoLatencyMs = [int]$sw.ElapsedMilliseconds

# ---------------------------------------------------------------------------
# 2. Parse cargo test output for per-test results
# ---------------------------------------------------------------------------
$testOutput = @()
if (Test-Path $testLog) {
    $testOutput = Get-Content -Raw -Path $testLog -Encoding UTF8
}

$methods = @{
    s3    = @('head_bucket', 'put_object', 'get_object', 'list_objects', 'delete_object')
    vault = @('get', 'set', 'delete', 'list', 'rotate')
    git   = @('init_bare', 'receive_pack', 'upload_pack', 'get_refs', 'list_refs')
    ai    = @('complete', 'embed', 'stream_token', 'cancel', 'usage_stats')
}

# 4 trait mock backends (s3/vault/git/ai) — core is shared chassis, treated as meta
$traitBackends = @('s3', 'vault', 'git', 'ai')

$results = @()
foreach ($backend in $traitBackends) {
    foreach ($method in $methods[$backend]) {
        # Phase 1: trait stub exists ⇒ "skipped_unimplemented"
        # Phase 2 will exercise the method and report real pass/fail latency
        $status = if ($cargoExit -eq 0) { 'skipped_unimplemented' } else { 'fail' }
        $results += [ordered]@{
            backend    = $backend
            method     = $method
            status     = $status
            latency_ms = 0
            note       = 'docs-only phase: trait stub exists, real impl lands in Phase 2'
        }
    }
}

# Add a meta entry: cargo test overall (core = shared chassis, NOT a mock backend)
$results += [ordered]@{
    backend    = 'core'
    method     = 'cargo_test_workspace'
    status     = if ($cargoExit -eq 0) { 'pass' } else { 'fail' }
    latency_ms = $cargoLatencyMs
    note       = "cargo test --workspace exit=$cargoExit (core is shared chassis, meta entry)"
}

# ---------------------------------------------------------------------------
# 3. Write smoke report
# ---------------------------------------------------------------------------
# Use @(...) wrap to force array semantics on Where-Object result
# (PowerShell .Count on a single hashtable returns property count, not length)
$passed = @($results | Where-Object { $_.status -eq 'pass' }).Count
$failed = @($results | Where-Object { $_.status -eq 'fail' }).Count
$skipped = @($results | Where-Object { $_.status -eq 'skipped_unimplemented' }).Count

$report = [ordered]@{
    smoke_at_unix_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    report_file      = $ReportFile
    mode             = 'in-process (docs-only stub)'
    cargo_test_log   = $testLog
    cargo_exit       = $cargoExit
    summary          = [ordered]@{
        pass    = $passed
        fail    = $failed
        skipped = $skipped
        total   = $results.Count
    }
    results          = $results
}

$report | ConvertTo-Json -Depth 6 | Out-File -FilePath $ReportFile -Encoding utf8

if ($cargoExit -ne 0) {
    Write-Host "✗ cargo test failed (exit=$cargoExit)" -ForegroundColor Red
    exit 1
}

Write-Host "✓ Smoke test complete" -ForegroundColor Green
Write-Host "  pass: $passed / fail: $failed / skipped (unimplemented): $skipped"
Write-Host "  report: $ReportFile"
exit 0
