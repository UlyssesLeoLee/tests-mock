#!/usr/bin/env pwsh
#Requires -Version 7.0
<#
.SYNOPSIS
    Load 3 JSON fixtures into the tests-mock backend (docs-only stub mode).

.DESCRIPTION
    Phase 1 (docs-only) behaviour:
      - Reads the 3 fixture JSON files from crates/tests-mock-fixtures/fixtures/
      - Validates each one with a simple JSON-schema check (non-empty + version field)
      - Writes a seed report to $env:TEMP/tests-mock-seed-report.json
      - Idempotent: re-running updates the report without breaking the env

    Phase 2 will route the loaded data into the actual in-process mock backends.

.PARAMETER StateFile
    Path to the init state file. Default: $env:TEMP/tests-mock-state.json.

.EXAMPLE
    pwsh D:/tests-mock/scripts/seed_fixtures.ps1
#>

[CmdletBinding()]
param(
    [string]$StateFile
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = Split-Path -Parent $PSScriptRoot
$fixturesDir = Join-Path $repoRoot 'crates/tests-mock-fixtures/fixtures'

if (-not $StateFile) {
    $tempRoot = $env:TEMP
    if (-not $tempRoot) { $tempRoot = [System.IO.Path]::GetTempPath() }
    $StateFile = Join-Path $tempRoot 'tests-mock-state.json'
}

# ---------------------------------------------------------------------------
# 1. Verify init has been run
# ---------------------------------------------------------------------------
if (-not (Test-Path $StateFile)) {
    Write-Host "✗ State file not found: $StateFile" -ForegroundColor Red
    Write-Host "  Run init_mock_env first." -ForegroundColor Red
    exit 1
}

# ---------------------------------------------------------------------------
# 2. Load and validate 3 fixtures
# ---------------------------------------------------------------------------
$fixtures = @(
    @{ name = 'user_creds.json';       key = 'users' },
    @{ name = 'repo_metadata.json';    key = 'repos' },
    @{ name = 'ai_response_cache.json'; key = 'responses' }
)

$results = @()
foreach ($f in $fixtures) {
    $path = Join-Path $fixturesDir $f.name
    if (-not (Test-Path $path)) {
        Write-Host "✗ Missing fixture: $path" -ForegroundColor Red
        exit 1
    }
    try {
        $obj = Get-Content -Raw -Path $path -Encoding UTF8 | ConvertFrom-Json
    } catch {
        Write-Host "✗ Invalid JSON in $($f.name): $_" -ForegroundColor Red
        exit 1
    }
    if (-not $obj.version) {
        Write-Host "✗ $($f.name) missing 'version' field" -ForegroundColor Red
        exit 1
    }
    if (-not ($obj.PSObject.Properties.Name -contains $f.key)) {
        Write-Host "✗ $($f.name) missing '$($f.key)' field" -ForegroundColor Red
        exit 1
    }
    $count = ($obj.$($f.key)).Count
    $results += [ordered]@{
        fixture   = $f.name
        version   = $obj.version
        key       = $f.key
        count     = $count
        status    = 'loaded'
        path      = $path
    }
    Write-Host "  ✓ $($f.name) v$($obj.version): $count entries"
}

# ---------------------------------------------------------------------------
# 3. Write seed report
# ---------------------------------------------------------------------------
$report = [ordered]@{
    seeded_at_unix_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    state_file        = $StateFile
    fixtures          = $results
    total_fixtures    = $results.Count
    backend_mode      = 'in-process (docs-only stub)'
}

$reportPath = Join-Path (Split-Path $StateFile -Parent) 'tests-mock-seed-report.json'
$report | ConvertTo-Json -Depth 5 | Out-File -FilePath $reportPath -Encoding utf8
Write-Host "✓ Seeded $($results.Count) fixtures" -ForegroundColor Green
Write-Host "  report: $reportPath"
exit 0
