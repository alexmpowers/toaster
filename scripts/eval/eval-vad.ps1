<#
.SYNOPSIS
    Silero VAD feature eval harness — Phase-3/4 gate for
    features/reintroduce-silero-vad.

.DESCRIPTION
    Produces a structured pass/fail report for the VAD reintroduction
    feature. Runs cheaply (seconds, no GPU, no ONNX download required)
    so it can gate CI and PR review.

    Gates:

      G1  catalog_pin             URL + SHA pinned in catalog/vad.rs
      G2  catalog_registered      silero-vad ModelInfo in build_static_catalog
      G3  prefilter_degrades      3 R-002 graceful-absence tests (AC-005-c)
      G4  boundary_ssot           snap_segments_vad_biased(&[], ...)
                                  byte-identical to energy path (AC-003-d)
      G5  gap_classification      5 R-004 classification bucket tests
                                  (AC-004-a..c)
      G6  runtime_delta           AC-002-b/c/d runtime reduction on
                                  silence-heavy fixtures. SKIP until a
                                  live-model eval fixture exists.
      G7  seam_rms_delta          AC-003-d seam-click RMS reduction on
                                  known-leaky boundary fixtures with
                                  VAD snap enabled. SKIP until a
                                  live-model eval fixture exists.

    G1-G5 are binary (pass/fail). G6-G7 are explicitly skipped with
    notes until the feature picks up an end-to-end perf fixture.

    Invocation:
        pwsh -NoProfile -File scripts/eval/eval-vad.ps1

.PARAMETER OutputRoot
    Root directory for reports. Default: eval/output/vad.
#>

[CmdletBinding()]
param(
    [string]$OutputRoot = (Join-Path $PSScriptRoot '..\..\eval\output\vad')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot   = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$CatalogRs  = Join-Path $RepoRoot 'src-tauri\src\managers\model\catalog\vad.rs'
$CatalogMod = Join-Path $RepoRoot 'src-tauri\src\managers\model\catalog\mod.rs'

# Source the repo's Windows build env so cargo can find cmake/clang/ffmpeg.
# Hygiene R rule: cargo must be invoked with setup-env.ps1 loaded.
$SetupEnv = Join-Path $RepoRoot 'scripts\setup-env.ps1'
if (Test-Path $SetupEnv) {
    Push-Location $RepoRoot
    try { . $SetupEnv *>&1 | Out-Null } finally { Pop-Location }
}

# -------------------------------------------------------------------------
function New-Result {
    param([string]$Name, [string]$Status, [hashtable]$Metrics = @{}, [string]$Notes = '')
    [ordered]@{
        name    = $Name
        status  = $Status
        metrics = $Metrics
        notes   = $Notes
    }
}

# G1: catalog pin ----------------------------------------------------------
function Invoke-G1-CatalogPin {
    if (-not (Test-Path $CatalogRs)) {
        return (New-Result 'G1_catalog_pin' 'error' @{} "missing file: $CatalogRs")
    }
    $src = Get-Content $CatalogRs -Raw
    $urlMatch  = [regex]::Match($src, 'SILERO_VAD_URL:\s*&str\s*=\s*"([^"]+)"')
    $shaMatch  = [regex]::Match($src, 'SILERO_VAD_SHA256:\s*&str\s*=\s*"([a-f0-9]{64})"')
    $metrics = @{
        url_pinned = $urlMatch.Success
        sha_pinned = $shaMatch.Success
        url        = if ($urlMatch.Success) { $urlMatch.Groups[1].Value } else { '' }
        sha_prefix = if ($shaMatch.Success) { $shaMatch.Groups[1].Value.Substring(0, 16) + '...' } else { '' }
    }
    $status = if ($urlMatch.Success -and $shaMatch.Success) { 'pass' } else { 'fail' }
    return (New-Result 'G1_catalog_pin' $status $metrics 'catalog/vad.rs URL + SHA-256 must be pinned before STATE->implemented.')
}

# G2: catalog registration -------------------------------------------------
function Invoke-G2-CatalogRegistered {
    if (-not (Test-Path $CatalogMod)) {
        return (New-Result 'G2_catalog_registered' 'error' @{} "missing: $CatalogMod")
    }
    $src = Get-Content $CatalogMod -Raw
    $hasVadIter = $src -match 'for\s+entry\s+in\s+vad::entries\(\)'
    $metrics = @{ vad_entries_iterated = $hasVadIter }
    $status = if ($hasVadIter) { 'pass' } else { 'fail' }
    return (New-Result 'G2_catalog_registered' $status $metrics 'build_static_catalog must iterate vad::entries().')
}

# G3-G5: cargo tests -------------------------------------------------------
function Invoke-CargoTest {
    param([string]$TestName, [int]$ExpectedPassing, [string]$Gate, [string]$Ac)

    Push-Location (Join-Path $RepoRoot 'src-tauri')
    try {
        $output = & cargo test -p toaster --test $TestName 2>&1 | Out-String
        $passedMatch = [regex]::Match($output, 'test result:\s*ok\.\s*(\d+)\s*passed')
        $failedMatch = [regex]::Match($output, '(\d+)\s*failed')
        $passed = if ($passedMatch.Success) { [int]$passedMatch.Groups[1].Value } else { 0 }
        $failed = if ($failedMatch.Success) { [int]$failedMatch.Groups[1].Value } else { -1 }
        $metrics = @{
            test           = $TestName
            expected_pass  = $ExpectedPassing
            observed_pass  = $passed
            observed_fail  = $failed
            ac             = $Ac
        }
        $status = if ($passed -ge $ExpectedPassing -and $failed -eq 0) { 'pass' } else { 'fail' }
        $notes  = if ($status -ne 'pass') { ($output -split "`n" | Select-Object -Last 20) -join "`n" } else { '' }
        return (New-Result $Gate $status $metrics $notes)
    } finally { Pop-Location }
}

# G6-G7: deferred perf gates ----------------------------------------------
function New-DeferredGate {
    param([string]$Name, [string]$Ac, [string]$Why)
    return (New-Result $Name 'skip' @{ ac = $Ac } $Why)
}

# -------------------------------------------------------------------------
Write-Host "=== Silero VAD eval ===" -ForegroundColor Cyan
$results = @()
$results += Invoke-G1-CatalogPin
$results += Invoke-G2-CatalogRegistered
$results += Invoke-CargoTest -TestName 'prefilter_degrades_gracefully' `
    -ExpectedPassing 3 -Gate 'G3_prefilter_degrades' `
    -Ac 'AC-005-c, AC-002-shape'
$results += Invoke-CargoTest -TestName 'vad_biased_snap_disabled_matches_baseline' `
    -ExpectedPassing 1 -Gate 'G4_boundary_ssot' `
    -Ac 'AC-003-d'
$results += Invoke-CargoTest -TestName 'filler_gap_classification' `
    -ExpectedPassing 5 -Gate 'G5_gap_classification' `
    -Ac 'AC-004-a..c'
$results += New-DeferredGate 'G6_runtime_delta' 'AC-002-b/c/d' `
    'Requires silence-heavy ASR fixture + downloaded Silero ONNX; track as follow-up eval-surface expansion.'
$results += New-DeferredGate 'G7_seam_rms_delta' 'AC-003-d-perf' `
    'Requires known-leaky boundary fixture with VAD snap enabled; track as follow-up eval-surface expansion.'

# -------------------------------------------------------------------------
# Aggregate
$failed  = @($results | Where-Object { $_.status -eq 'fail' })
$errored = @($results | Where-Object { $_.status -eq 'error' })
$overallStatus = if ($errored.Count) { 'error' } elseif ($failed.Count) { 'fail' } else { 'pass' }

$report = [ordered]@{
    timestamp  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    feature    = 'reintroduce-silero-vad'
    status     = $overallStatus
    gates      = $results
}

if (-not (Test-Path $OutputRoot)) { New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null }
$jsonPath = Join-Path $OutputRoot 'report.json'
$mdPath   = Join-Path $OutputRoot 'report.md'
($report | ConvertTo-Json -Depth 10) | Set-Content -Path $jsonPath -Encoding UTF8

$md = @()
$md += "# Silero VAD eval report"
$md += ""
$md += "Status: **$overallStatus** (generated $($report.timestamp))"
$md += ""
$md += "| Gate | Status | AC | Notes |"
$md += "| --- | --- | --- | --- |"
foreach ($r in $results) {
    $ac = if ($r.metrics.ContainsKey('ac')) { $r.metrics.ac } else { '-' }
    $notes = if ($r.notes) { ($r.notes -replace '\|', '\|' -replace "`r?`n", ' ') } else { '' }
    if ($notes.Length -gt 80) { $notes = $notes.Substring(0, 77) + '...' }
    $md += "| $($r.name) | $($r.status) | $ac | $notes |"
}
($md -join "`n") | Set-Content -Path $mdPath -Encoding UTF8

Write-Host ""
foreach ($r in $results) {
    $color = switch ($r.status) { 'pass' { 'Green' } 'fail' { 'Red' } 'skip' { 'Yellow' } default { 'DarkYellow' } }
    Write-Host ("  [{0}] {1}" -f $r.status.ToUpper().PadRight(5), $r.name) -ForegroundColor $color
    if ($r.notes -and $r.status -ne 'pass') { Write-Host "         $($r.notes)" -ForegroundColor DarkGray }
}
Write-Host ""
$c = if ($overallStatus -eq 'pass') { 'Green' } else { 'Red' }
Write-Host ("Overall: {0}" -f $overallStatus.ToUpper()) -ForegroundColor $c
Write-Host ("Report : {0}" -f $jsonPath) -ForegroundColor DarkGray

if ($overallStatus -ne 'pass') { exit 1 }
exit 0
