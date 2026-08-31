#!/usr/bin/env pwsh
#Requires -Version 7.0
<#
.SYNOPSIS
    Initialize the tests-mock environment (docs-only stub mode).

.DESCRIPTION
    Phase 1 (docs-only) behaviour:
      - Writes a state file with mode=in-process and 4 backend slots.
      - Performs a health check by running `cargo check --workspace`
        to confirm all trait stubs compile.
      - On failure, removes the state file (rollback).

    Phase 2 (B 子代理) will extend this script to optionally launch
    docker compose for fake minIO / postgres / gitea / llama.cpp.

.PARAMETER Docker
    Switch to docker mode (Phase 2 only; ignored in docs-only phase).

.PARAMETER StateFile
    Override state file path. Default: $env:TEMP/tests-mock-state.json.

.EXAMPLE
    pwsh D:/tests-mock/scripts/init_mock_env.ps1
#>

[CmdletBinding()]
param(
    [switch]$Docker,
    [string]$StateFile
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

# ---------------------------------------------------------------------------
# 1. Resolve state file path (Windows-friendly)
# ---------------------------------------------------------------------------
if (-not $StateFile) {
    $tempRoot = $env:TEMP
    if (-not $tempRoot) { $tempRoot = [System.IO.Path]::GetTempPath() }
    $StateFile = Join-Path $tempRoot 'tests-mock-state.json'
}

$mode = if ($Docker) { 'docker' } else { 'in-process' }
$repoRoot = Split-Path -Parent $PSScriptRoot

# ---------------------------------------------------------------------------
# 2. Health check: cargo check --workspace
# ---------------------------------------------------------------------------
Write-Host "→ Health check: cargo check --workspace" -ForegroundColor Cyan
$checkLog = Join-Path $repoRoot 'target-cargo-check.log'
$checkProc = Start-Process -FilePath 'cargo' `
    -ArgumentList @('check', '--workspace', '--quiet') `
    -WorkingDirectory $repoRoot `
    -RedirectStandardOutput $checkLog `
    -RedirectStandardError "$checkLog.err" `
    -NoNewWindow -PassThru -Wait

if ($checkProc.ExitCode -ne 0) {
    Write-Host "✗ cargo check failed (exit=$($checkProc.ExitCode))" -ForegroundColor Red
    if (Test-Path $StateFile) { Remove-Item $StateFile -Force }
    exit 1
}

# ---------------------------------------------------------------------------
# 3. Write state file
# ---------------------------------------------------------------------------
$state = [ordered]@{
    mode               = $mode
    pid                = $PID
    started_at_unix_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    backends           = @('s3', 'vault', 'git', 'ai')
    state_file         = $StateFile
    cargo_check_log    = $checkLog
}

$state | ConvertTo-Json -Depth 5 | Out-File -FilePath $StateFile -Encoding utf8
Write-Host "✓ Mock environment initialized (mode=$mode)" -ForegroundColor Green
Write-Host "  state: $StateFile"
Write-Host "  backends: s3, vault, git, ai (all docs-only stubs)"
exit 0
