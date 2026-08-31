#!/usr/bin/env pwsh
#Requires -Version 7.0
<#
.SYNOPSIS
    Tear down the tests-mock environment (docs-only stub mode).

.DESCRIPTION
    Idempotent: re-running on an already-clean environment is a no-op.
    Removes:
      - $env:TEMP/tests-mock-state.json
      - $env:TEMP/tests-mock-smoke-report.json
      - $env:TEMP/tests-mock-stress-report.json
      - $env:TEMP/tests-mock-seed-report.json
    Cleans cargo log artifacts under D:/tests-mock/target-* and
    prints a summary of what was removed.

.PARAMETER StateFile
    Path to the state file. Default: $env:TEMP/tests-mock-state.json.

.EXAMPLE
    pwsh D:/tests-mock/scripts/cleanup_mock_env.ps1
#>

[CmdletBinding()]
param(
    [string]$StateFile
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$tempRoot = $env:TEMP
if (-not $tempRoot) { $tempRoot = [System.IO.Path]::GetTempPath() }
if (-not $StateFile) { $StateFile = Join-Path $tempRoot 'tests-mock-state.json' }

$repoRoot = Split-Path -Parent $PSScriptRoot

# ---------------------------------------------------------------------------
# 1. List candidate paths to remove
# ---------------------------------------------------------------------------
$candidates = @(
    $StateFile
    (Join-Path $tempRoot 'tests-mock-smoke-report.json')
    (Join-Path $tempRoot 'tests-mock-seed-report.json')
    (Join-Path $tempRoot 'tests-mock-stress-report.json')
    (Join-Path $repoRoot 'target-cargo-check.log')
    (Join-Path $repoRoot 'target-cargo-check.log.err')
    (Join-Path $repoRoot 'target-smoke-test.log')
    (Join-Path $repoRoot 'target-smoke-test.log.err')
)

# ---------------------------------------------------------------------------
# 2. Remove each candidate if it exists (idempotent)
# ---------------------------------------------------------------------------
$removed = @()
$kept = @()
foreach ($p in $candidates) {
    if (Test-Path $p) {
        try {
            Remove-Item $p -Force
            $removed += $p
        } catch {
            $kept += "$p (error: $_)"
        }
    }
    # else: not present, nothing to do
}

# ---------------------------------------------------------------------------
# 3. Print summary
# ---------------------------------------------------------------------------
Write-Host "✓ Cleanup complete" -ForegroundColor Green
Write-Host "  removed ($($removed.Count)):"
foreach ($p in $removed) { Write-Host "    - $p" }
if ($kept.Count -gt 0) {
    Write-Host "  kept (errors):" -ForegroundColor Yellow
    foreach ($p in $kept) { Write-Host "    - $p" }
}
exit 0
