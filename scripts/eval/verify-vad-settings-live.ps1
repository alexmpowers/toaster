<#
.SYNOPSIS
    AC-006-c operator helper: live-app verification of the VAD toggles.

    SUPERSEDED (2026-04-19): `vad_prefilter_enabled` has been removed.
    Only `vad_refine_boundaries` remains. Script still works but the
    prefilter-flip assertions are no-ops.

.DESCRIPTION
    Walks an operator through the manual verification of acceptance
    criterion AC-006-c from features/reintroduce-silero-vad/PRD.md:
    toggle each VAD setting on and off, confirm the change persists in
    settings.json, and confirm the running backend picks it up.

    The script does the bookkeeping the operator otherwise does by
    hand:

      1. Captures a SHA-256 of settings.json before launch.
      2. Launches Toaster via launch-toaster-monitored.ps1 for the
         requested duration (default 5m).
      3. Polls settings.json once per second while the app is up,
         records every observed value of `vad_prefilter_enabled` and
         `vad_refine_boundaries`, and prints a pass/fail summary
         showing whether each toggle was flipped at least once and
         whether the new value persists at exit.
      4. Tails the launcher's structured log for any line containing
         "VAD boundary refinement" so the operator can spot whether
         the live wiring fired.
      5. Writes a journal entry under
         eval/output/vad/operator/<UTC-timestamp>/journal.md that
         records operator initials (interactive prompt), the toggle
         transitions captured, the launcher exit code, and any log
         lines that mention VAD activity. The journal entry is the
         AC-006-c artifact the PR body cites.

    AC-006-c is human-in-loop by definition; this script reduces it to
    "click each toggle on, off, on; type your initials". Everything
    else is captured automatically.

.PARAMETER Duration
    How long to keep the app open. Accepts the same syntax as
    launch-toaster-monitored.ps1 (e.g. 30s, 5m, 1h). Default: 5m.

.PARAMETER OutputRoot
    Where to write the journal. Default: eval/output/vad/operator.

.PARAMETER Initials
    Operator initials. If not provided, the script prompts.
#>

[CmdletBinding()]
param(
    [string]$Duration = '5m',
    [string]$OutputRoot = (Join-Path $PSScriptRoot '..\..\eval\output\vad\operator'),
    [string]$Initials
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot   = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Launcher   = Join-Path $RepoRoot 'scripts\launch-toaster-monitored.ps1'

if (-not (Test-Path $Launcher)) {
    throw "launcher missing: $Launcher"
}

# Locate settings.json — Toaster persists via tauri-plugin-store under
# %APPDATA%\com.toaster.app\settings.json (Windows) or the platform
# equivalent. Allow override via TOASTER_SETTINGS_JSON for non-default
# installs.
function Resolve-SettingsPath {
    if ($env:TOASTER_SETTINGS_JSON) { return $env:TOASTER_SETTINGS_JSON }
    if ($IsWindows -or $env:OS -eq 'Windows_NT') {
        return (Join-Path $env:APPDATA 'com.toaster.app\settings.json')
    }
    if ($IsMacOS) {
        return (Join-Path $env:HOME 'Library/Application Support/com.toaster.app/settings.json')
    }
    return (Join-Path $env:HOME '.config/com.toaster.app/settings.json')
}

$SettingsPath = Resolve-SettingsPath
$Stamp        = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$JournalDir   = Join-Path $OutputRoot $Stamp
New-Item -ItemType Directory -Path $JournalDir -Force | Out-Null
$JournalPath  = Join-Path $JournalDir 'journal.md'
$LogPath      = Join-Path $JournalDir 'launcher.log'

function Read-Toggles {
    if (-not (Test-Path $SettingsPath)) { return $null }
    try {
        $text = Get-Content $SettingsPath -Raw -ErrorAction Stop
        $json = $text | ConvertFrom-Json -ErrorAction Stop
    } catch {
        return $null
    }
    [pscustomobject]@{
        vad_prefilter_enabled = $json.vad_prefilter_enabled
        vad_refine_boundaries = $json.vad_refine_boundaries
    }
}

if (-not $Initials) {
    $Initials = Read-Host "Operator initials (e.g. AM)"
}
if (-not $Initials) { $Initials = 'unknown' }

Write-Host "=== AC-006-c live-app VAD verification ===" -ForegroundColor Cyan
Write-Host "Settings file : $SettingsPath"
Write-Host "Journal dir   : $JournalDir"
Write-Host "Operator      : $Initials"
Write-Host "Duration      : $Duration"
Write-Host ""
Write-Host "While the app is up:" -ForegroundColor Yellow
Write-Host "  1. Open Settings -> Advanced -> Voice activity detection."
Write-Host "  2. Toggle 'VAD pre-filter' OFF, then ON, then OFF."
Write-Host "  3. Toggle 'VAD boundary refinement' OFF, then ON, then OFF."
Write-Host "  4. Render a preview of any loaded media so the boundary"
Write-Host "     refinement code path executes (look for 'VAD boundary"
Write-Host "     refinement' in the launcher log)."
Write-Host ""

$initial = Read-Toggles
if ($null -eq $initial) {
    Write-Host "Note: settings.json not present yet at $SettingsPath. Will start watching after launch." -ForegroundColor DarkYellow
}

# Launch in a background job so this script can poll alongside.
$launcherJob = Start-Job -ScriptBlock {
    param($Launcher, $Duration, $LogPath)
    & pwsh -NoProfile -File $Launcher -Duration $Duration *> $LogPath
    $LASTEXITCODE
} -ArgumentList $Launcher, $Duration, $LogPath

$transitions = @()
$lastSeen    = $initial
$pollUntil   = (Get-Date).AddMinutes(60)  # safety cap; launcher's own duration governs.
while ($launcherJob.State -eq 'Running' -and (Get-Date) -lt $pollUntil) {
    Start-Sleep -Seconds 1
    $now = Read-Toggles
    if ($null -eq $now) { continue }
    if ($null -eq $lastSeen) {
        $lastSeen = $now
        $transitions += [pscustomobject]@{
            ts = (Get-Date).ToUniversalTime().ToString('o')
            kind = 'initial'
            prefilter = $now.vad_prefilter_enabled
            refine    = $now.vad_refine_boundaries
        }
        continue
    }
    if ($now.vad_prefilter_enabled -ne $lastSeen.vad_prefilter_enabled `
        -or $now.vad_refine_boundaries -ne $lastSeen.vad_refine_boundaries) {
        $transitions += [pscustomobject]@{
            ts = (Get-Date).ToUniversalTime().ToString('o')
            kind = 'change'
            prefilter = $now.vad_prefilter_enabled
            refine    = $now.vad_refine_boundaries
        }
        $lastSeen = $now
        Write-Host ("  [{0}] toggle change: prefilter={1} refine={2}" `
            -f (Get-Date).ToString('HH:mm:ss'), `
                $now.vad_prefilter_enabled, $now.vad_refine_boundaries) -ForegroundColor Green
    }
}

$launcherExit = Receive-Job -Job $launcherJob -Wait -AutoRemoveJob

$final = Read-Toggles
$prefilterFlipped = ($transitions | Where-Object { $_.kind -eq 'change' -and $_.prefilter -ne $initial.vad_prefilter_enabled }).Count -gt 0
$refineFlipped    = ($transitions | Where-Object { $_.kind -eq 'change' -and $_.refine    -ne $initial.vad_refine_boundaries }).Count -gt 0

# Scan launcher log for VAD-related lines.
$vadLogLines = @()
if (Test-Path $LogPath) {
    $vadLogLines = Select-String -Path $LogPath -Pattern 'VAD boundary refinement|vad_refine_boundaries|vad_prefilter_enabled' -SimpleMatch:$false -ErrorAction SilentlyContinue |
                   ForEach-Object { $_.Line }
}

$status = if ($prefilterFlipped -and $refineFlipped) { 'pass' }
          elseif ($prefilterFlipped -or $refineFlipped) { 'partial' }
          else { 'fail' }

# Journal
$md = @()
$md += "# AC-006-c live-app VAD verification"
$md += ""
$md += "- Operator: $Initials"
$md += "- Stamp (UTC): $Stamp"
$md += "- Duration: $Duration"
$md += "- Settings file: $SettingsPath"
$md += "- Launcher exit code: $launcherExit"
$md += "- Status: **$status**"
$md += ""
$md += "## Toggle transitions"
$md += ""
if ($transitions.Count -eq 0) {
    $md += "_No transitions captured._"
} else {
    $md += "| ts | kind | prefilter | refine |"
    $md += "|---|---|---|---|"
    foreach ($t in $transitions) {
        $md += "| $($t.ts) | $($t.kind) | $($t.prefilter) | $($t.refine) |"
    }
}
$md += ""
$md += "## Final settings"
$md += ""
if ($final) {
    $md += "- vad_prefilter_enabled = $($final.vad_prefilter_enabled)"
    $md += "- vad_refine_boundaries = $($final.vad_refine_boundaries)"
} else {
    $md += "_settings.json unreadable at exit_"
}
$md += ""
$md += "## Launcher VAD log lines"
$md += ""
if ($vadLogLines.Count -eq 0) {
    $md += "_(none — operator may not have rendered preview / export with vad_refine_boundaries=true)_"
} else {
    foreach ($line in $vadLogLines) { $md += "    $line" }
}
($md -join "`n") | Set-Content -Path $JournalPath -Encoding UTF8

Write-Host ""
Write-Host "Status         : $status"
Write-Host "Prefilter flip : $prefilterFlipped"
Write-Host "Refine flip    : $refineFlipped"
Write-Host "Journal        : $JournalPath"

if ($status -eq 'pass') { exit 0 } else { exit 1 }
