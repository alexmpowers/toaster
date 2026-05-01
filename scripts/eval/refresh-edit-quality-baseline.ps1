<#
.SYNOPSIS
    Regenerates tests/fixtures/edit-quality.baseline.json from the canonical
    eval fixtures using scripts/eval/eval-edit-quality.ps1.

.DESCRIPTION
    This helper keeps export-parity baseline refresh explicit and reproducible.
    Use it when the fixture rendering pipeline changes and the baseline needs
    to be intentionally updated.

.PARAMETER Original
    Source media fixture path.

.PARAMETER Edited
    Edited media fixture path.

.PARAMETER BaselinePath
    Output baseline JSON path.

.EXAMPLE
    pwsh -NoProfile -File scripts/eval/refresh-edit-quality-baseline.ps1
#>

[CmdletBinding()]
param(
    [string]$Original = "eval/fixtures/toaster_example.mp4",
    [string]$Edited = "eval/fixtures/toaster_example-edited.mp4",
    [string]$BaselinePath = "tests/fixtures/edit-quality.baseline.json"
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Set-Location $RepoRoot

$evalScript = Join-Path $RepoRoot "scripts\eval\eval-edit-quality.ps1"
if (-not (Test-Path $evalScript)) {
    Write-Error "eval-edit-quality.ps1 not found: $evalScript"
    exit 2
}

if (-not (Test-Path $Original)) {
    Write-Error "Original fixture not found: $Original"
    exit 2
}

if (-not (Test-Path $Edited)) {
    Write-Error "Edited fixture not found: $Edited"
    exit 2
}

$baselineDir = Split-Path -Parent $BaselinePath
if ($baselineDir -and -not (Test-Path $baselineDir)) {
    New-Item -ItemType Directory -Path $baselineDir -Force | Out-Null
}

Write-Host "Refreshing export-parity baseline..." -ForegroundColor Cyan
Write-Host "  Original: $Original"
Write-Host "  Edited:   $Edited"
Write-Host "  Output:   $BaselinePath"

& pwsh -NoProfile -File $evalScript -Original $Original -Edited $Edited -OutputJson $BaselinePath
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to refresh baseline (eval-edit-quality exited $LASTEXITCODE)"
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "Baseline refreshed: $BaselinePath" -ForegroundColor Green
Write-Host "Next: run eval harness with export parity enabled to validate drift against this baseline."
exit 0
