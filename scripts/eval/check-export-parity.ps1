<#
.SYNOPSIS
    Compares eval/fixtures export quality against tests/fixtures baseline.

.DESCRIPTION
    Runs scripts/eval/eval-edit-quality.ps1 to generate current metrics,
    compares them against tests/fixtures/edit-quality.baseline.json, and
    fails if deltas exceed tolerance.

.PARAMETER Original
    Path to original fixture media.

.PARAMETER Edited
    Path to edited fixture media.

.PARAMETER BaselinePath
    Path to baseline JSON.

.PARAMETER OutputJson
    Optional path for current output JSON.

.PARAMETER ToleranceSeconds
    Absolute tolerance for duration/leading/trailing silence deltas.

.EXAMPLE
    pwsh -NoProfile -File scripts/eval/check-export-parity.ps1
#>

[CmdletBinding()]
param(
    [string]$Original = "eval/fixtures/toaster_example.mp4",
    [string]$Edited = "eval/fixtures/toaster_example-edited.mp4",
    [string]$BaselinePath = "tests/fixtures/edit-quality.baseline.json",
    [string]$OutputJson = ".eval-output/edit-quality.current.json",
    [double]$ToleranceSeconds = 0.050
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Set-Location $RepoRoot

$evalScript = Join-Path $RepoRoot "scripts\eval\eval-edit-quality.ps1"
if (-not (Test-Path $evalScript)) {
    Write-Error "eval-edit-quality.ps1 not found: $evalScript"
    exit 2
}

if (-not (Test-Path $BaselinePath)) {
    Write-Error "Baseline not found: $BaselinePath"
    exit 2
}

$currDir = Split-Path -Parent $OutputJson
if ($currDir -and -not (Test-Path $currDir)) {
    New-Item -ItemType Directory -Path $currDir -Force | Out-Null
}

& pwsh -NoProfile -File $evalScript -Original $Original -Edited $Edited -OutputJson $OutputJson
if ($LASTEXITCODE -ne 0) {
    Write-Error "eval-edit-quality.ps1 exited $LASTEXITCODE"
    exit $LASTEXITCODE
}

$current = Get-Content $OutputJson -Raw | ConvertFrom-Json
$baseline = Get-Content $BaselinePath -Raw | ConvertFrom-Json

$durationDelta = [math]::Round(($current.edited.duration_s - $baseline.edited.duration_s), 3)
$gapsDelta = ($current.edited.silence_gaps - $baseline.edited.silence_gaps)
$leadingDelta = [math]::Round(($current.edited.leading_silence - $baseline.edited.leading_silence), 3)
$trailingDelta = [math]::Round(($current.edited.trailing_silence - $baseline.edited.trailing_silence), 3)

$regressed = ([math]::Abs($durationDelta) -gt $ToleranceSeconds) `
             -or ($gapsDelta -ne 0) `
             -or ([math]::Abs($leadingDelta) -gt $ToleranceSeconds) `
             -or ([math]::Abs($trailingDelta) -gt $ToleranceSeconds)

Write-Host "Export parity comparison:" -ForegroundColor Cyan
Write-Host "  duration delta:        $durationDelta s"
Write-Host "  silence gaps delta:    $gapsDelta"
Write-Host "  leading silence delta: $leadingDelta s"
Write-Host "  trailing silence delta:$trailingDelta s"
Write-Host "  tolerance:             +/-$ToleranceSeconds s"

if ($regressed) {
    Write-Host ""
    Write-Host "Export parity check FAILED." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Export parity check PASSED." -ForegroundColor Green
exit 0
