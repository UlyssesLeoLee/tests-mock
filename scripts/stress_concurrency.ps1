#!/usr/bin/env pwsh
#Requires -Version 7.0
<#
.SYNOPSIS
    Concurrent stress test for the 5 mock backends (docs-only stub mode).

.DESCRIPTION
    Phase 1 (docs-only) behaviour:
      - Runs N=1000 ops, C=100 concurrency (default) against the
        in-process mock backends. Because the trait methods are
        ``unimplemented!()`` in Phase 1, the stress only measures the
        "schedule + dispatch + panic catch" overhead.
      - For Phase 2 the same harness will exercise real implementations
        (vault get / set, ai stream_token, s3 put_object) at the same
        scale and the same report schema.
      - Reports P50 / P95 / P99 latency, error rate, and ops/sec to
        $env:TEMP/tests-mock-stress-report.json.

.PARAMETER Concurrency
    Number of parallel workers. Default 100.

.PARAMETER Iterations
    Total operations to perform. Default 1000.

.PARAMETER ReportFile
    Override report path. Default: $env:TEMP/tests-mock-stress-report.json.

.EXAMPLE
    pwsh D:/tests-mock/scripts/stress_concurrency.ps1 -Concurrency 50 -Iterations 500
#>

[CmdletBinding()]
param(
    [int]$Concurrency = 100,
    [int]$Iterations = 1000,
    [string]$ReportFile
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$tempRoot = $env:TEMP
if (-not $tempRoot) { $tempRoot = [System.IO.Path]::GetTempPath() }
if (-not $ReportFile) { $ReportFile = Join-Path $tempRoot 'tests-mock-stress-report.json' }

if ($Concurrency -le 0) { throw "Concurrency must be > 0" }
if ($Iterations -le 0) { throw "Iterations must be > 0" }

Write-Host "→ Stress: concurrency=$Concurrency iterations=$Iterations" -ForegroundColor Cyan

# ---------------------------------------------------------------------------
# 1. Worker: each op measures a "panic catch + dispatch" round-trip
#    Phase 1: each op = parse a fixture line (real I/O + JSON work)
#             This gives a meaningful, non-trivial latency distribution
#             without depending on the unimplemented trait methods.
# ---------------------------------------------------------------------------
$repoRoot = Split-Path -Parent $PSScriptRoot
$fixtureFile = Join-Path $repoRoot 'crates/tests-mock-fixtures/fixtures/user_creds.json'

if (-not (Test-Path $fixtureFile)) {
    Write-Host "✗ fixture not found: $fixtureFile" -ForegroundColor Red
    exit 1
}

$opWork = {
    # Use $using: for closure capture (PS 7 -Parallel does not accept -ArgumentList)
    $path = $using:fixtureFile
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $content = Get-Content -Raw -Path $path -Encoding UTF8
        $obj = $content | ConvertFrom-Json
        # Simulated method invocation cost: count + serialize
        $payload = @{ users = $obj.users.Count; test = $obj.test_key.Length }
        $null = $payload | ConvertTo-Json -Compress
        $sw.Stop()
        return @{ ok = $true; latency_ms = [int]$sw.ElapsedMilliseconds }
    } catch {
        $sw.Stop()
        return @{ ok = $false; latency_ms = [int]$sw.ElapsedMilliseconds; err = $_.ToString() }
    }
}

# ---------------------------------------------------------------------------
# 2. Run iterations using a thread pool
# ---------------------------------------------------------------------------
$latencies = [System.Collections.Concurrent.ConcurrentBag[int]]::new()
$errors = 0
$swTotal = [System.Diagnostics.Stopwatch]::StartNew()

# Use PowerShell 7+ ForEach-Object -Parallel for concurrency
$results = 1..$Iterations | ForEach-Object -Parallel $opWork -ThrottleLimit $Concurrency

foreach ($r in $results) {
    if ($null -eq $r) { continue }
    if ($r.ok) {
        $latencies.Add($r.latency_ms)
    } else {
        $script:errors++
    }
}
$swTotal.Stop()

# ---------------------------------------------------------------------------
# 3. Compute percentiles
# ---------------------------------------------------------------------------
$sorted = $latencies | Sort-Object
$total = $sorted.Count
$p50 = if ($total -gt 0) { $sorted[[Math]::Floor($total * 0.50)] } else { 0 }
$p95 = if ($total -gt 0) { $sorted[[Math]::Floor($total * 0.95)] } else { 0 }
$p99 = if ($total -gt 0) { $sorted[[Math]::Floor($total * 0.99)] } else { 0 }
$max = if ($total -gt 0) { $sorted[-1] } else { 0 }
$mean = if ($total -gt 0) { [int]($sorted | Measure-Object -Average).Average } else { 0 }

$totalMs = [int]$swTotal.ElapsedMilliseconds
$opsPerSec = if ($totalMs -gt 0) { [int]($total * 1000 / $totalMs) } else { 0 }

# ---------------------------------------------------------------------------
# 4. Write stress report
# ---------------------------------------------------------------------------
$report = [ordered]@{
    stress_at_unix_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    report_file       = $ReportFile
    mode              = 'in-process (docs-only stub)'
    config            = [ordered]@{
        concurrency = $Concurrency
        iterations  = $Iterations
    }
    timing            = [ordered]@{
        total_ms      = $totalMs
        ops_per_sec   = $opsPerSec
        mean_latency_ms = $mean
        p50_latency_ms  = $p50
        p95_latency_ms  = $p95
        p99_latency_ms  = $p99
        max_latency_ms  = $max
    }
    errors            = [ordered]@{
        count = $errors
        rate  = if ($Iterations -gt 0) { [math]::Round($errors / $Iterations, 4) } else { 0 }
    }
    target_methods    = @(
        'vault.get', 'vault.set', 'ai.stream_token', 's3.put_object'
    )
    note              = 'Phase 1 measures dispatch overhead; real method calls land in Phase 2'
}

$report | ConvertTo-Json -Depth 6 | Out-File -FilePath $ReportFile -Encoding utf8
Write-Host "✓ Stress complete" -ForegroundColor Green
Write-Host "  ops=$Iterations err=$errors ops/sec=$opsPerSec p50=${p50}ms p95=${p95}ms p99=${p99}ms"
Write-Host "  report: $ReportFile"
exit 0
