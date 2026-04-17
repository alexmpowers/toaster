[CmdletBinding()]
param(
    [string]$MediaPath = "C:\Users\alexm\Downloads\AddReleaseItem.mp4",
    [string]$OutputDir,
    [string]$AsrModelPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not (Test-Path $MediaPath -PathType Leaf)) {
    Write-Error "Media file not found: $MediaPath"
}

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $OutputDir = Join-Path $repoRoot ".launch-monitor\live-validation-$stamp"
}

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

Write-Host "[live-validation] media: $MediaPath"
Write-Host "[live-validation] output: $OutputDir"
if (-not [string]::IsNullOrWhiteSpace($AsrModelPath)) {
    Write-Host "[live-validation] asr_model_path: $AsrModelPath"
}

. (Join-Path $PSScriptRoot "setup-env.ps1")

$env:TOASTER_LIVE_MEDIA_PATH = $MediaPath
$env:TOASTER_LIVE_OUTPUT_DIR = $OutputDir
if (-not [string]::IsNullOrWhiteSpace($AsrModelPath)) {
    $env:TOASTER_LIVE_ASR_MODEL_PATH = $AsrModelPath
}

Push-Location (Join-Path $repoRoot "src-tauri")
try {
    cargo test commands::waveform::tests::live_validation_backend_media_pipeline -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}

Write-Host "[live-validation] completed"
Write-Host "[live-validation] report: $(Join-Path $OutputDir 'live-validation-report.json')"
