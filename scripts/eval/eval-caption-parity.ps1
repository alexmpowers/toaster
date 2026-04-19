<#
.SYNOPSIS
    Caption parity eval harness. Verifies preview<->export caption
    geometry + timing parity within 1 px and 1 sample @ 48 kHz.

.DESCRIPTION
    For each fixture under eval/caption-parity/fixtures/, the harness:

      1. Invokes the `caption_parity_dump` test binary (it reads
         `input.json`, runs the authoritative `build_blocks` +
         `compute_caption_layout`, and writes the result to a temp
         actual.json along with the raw ASS document).
      2. Parses the `ass` field from that dump and cross-checks each
         Dialogue line's Start/End/MarginV/Text against the
         corresponding `CaptionBlock` — this is the preview<->export
         parity assertion (same CaptionBlock[] feeds both).
      3. Compares the dump to `expected.json` within tolerances:
         geometry = 1 px, timing = 21 us (1 sample @ 48 kHz, rounded
         from 20.833...us).
      4. Emits a pass/fail JSON + markdown report under
         eval/output/caption-parity/<timestamp>/ and exits 0 (pass)
         or 1 (fail). Exit 2 when `expected.json` is missing and
         -Regenerate was not supplied. Exit 3 on harness errors.

    Skill invocation:
        pwsh -NoProfile -File scripts/eval/eval-caption-parity.ps1
        pwsh -NoProfile -File scripts/eval/eval-caption-parity.ps1 -Fixture single_line_01
        pwsh -NoProfile -File scripts/eval/eval-caption-parity.ps1 -Regenerate
        pwsh -NoProfile -File scripts/eval/eval-caption-parity.ps1 -ForceDrift "block_0.padding_x_px=+2"

.PARAMETER Fixture
    Fixture stem to run (default: all fixture directories).

.PARAMETER Regenerate
    Rewrite `expected.json` for each run fixture from the fresh dump.
    Review the resulting git diff before staging.

.PARAMETER ForceDrift
    Inject a synthetic drift of shape `<path>=<delta>` into the actual
    dump before comparison. The path is either `layout.<field>` or
    `block_<index>.<field>`. Integer fields accept signed integer
    deltas; string fields accept a replacement string prefixed with
    `=`. Used as a negative test to prove the harness fails on drift.

.PARAMETER OutputRoot
    Root directory for reports. Default: eval/output/caption-parity.
#>

[CmdletBinding()]
param(
    [string]$Fixture,
    [switch]$Regenerate,
    [string]$ForceDrift,
    [string]$OutputRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot     = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$FixturesRoot = Join-Path $RepoRoot 'eval\caption-parity\fixtures'
$CargoDir     = Join-Path $RepoRoot 'src-tauri'
if (-not $OutputRoot) {
    $OutputRoot = Join-Path $RepoRoot 'eval\output\caption-parity'
}

# Tolerances. Declared once, intentionally. See BLUEPRINT §Tolerances.
$Tol = @{
    GeometryPx = 1
    TimingUs   = 21   # 1 sample @ 48 kHz = 20.833...us -> 21 after ceil.
}

function Write-Section {
    param([string]$Text)
    Write-Host ''
    Write-Host "== $Text ==" -ForegroundColor Cyan
}

function Resolve-FixtureDirs {
    if (-not (Test-Path $FixturesRoot)) {
        throw "Fixtures root not found: $FixturesRoot"
    }
    $dirs = Get-ChildItem -Path $FixturesRoot -Directory | Sort-Object Name
    if ($Fixture) {
        $dirs = $dirs | Where-Object { $_.Name -eq $Fixture }
        if (-not $dirs) {
            throw "Fixture not found: $Fixture"
        }
    }
    return ,@($dirs)
}

function Invoke-Dump {
    param([string]$InputPath, [string]$OutputPath)
    Push-Location $CargoDir
    try {
        $env:CAPTION_PARITY_FIXTURE = $InputPath
        $env:CAPTION_PARITY_OUTPUT  = $OutputPath
        # Quiet unless the test fails.
        $log = & cargo test --test caption_parity_dump --quiet -- --nocapture 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "caption_parity_dump failed for $InputPath`n$($log -join [Environment]::NewLine)"
        }
    }
    finally {
        Remove-Item Env:CAPTION_PARITY_FIXTURE -ErrorAction SilentlyContinue
        Remove-Item Env:CAPTION_PARITY_OUTPUT  -ErrorAction SilentlyContinue
        Pop-Location
    }
}

function Read-Json {
    param([string]$Path)
    return (Get-Content -Raw -Path $Path | ConvertFrom-Json)
}

function Apply-ForceDrift {
    param($Actual, [string]$Spec)
    if (-not $Spec) { return }

    $eqIdx = $Spec.IndexOf('=')
    if ($eqIdx -lt 1) { throw "Invalid -ForceDrift spec (expected `<path>=<delta>`): $Spec" }
    $path  = $Spec.Substring(0, $eqIdx)
    $delta = $Spec.Substring($eqIdx + 1)

    $segments = $path.Split('.')
    if ($segments.Count -lt 2) { throw "Invalid -ForceDrift path: $path" }
    $root = $segments[0]
    $field = $segments[-1]

    # Walk to the target object.
    $target = $null
    if ($root -eq 'layout') {
        $target = $Actual.layout
    }
    elseif ($root -match '^block_(\d+)$') {
        $idx = [int]$Matches[1]
        if ($idx -lt 0 -or $idx -ge $Actual.blocks.Count) {
            throw "-ForceDrift block index out of range: $idx"
        }
        $target = $Actual.blocks[$idx]
    }
    else {
        throw "Unsupported -ForceDrift root: $root (expected 'layout' or 'block_<n>')"
    }

    $current = $target.$field
    if ($null -eq $current) { throw "-ForceDrift field not present: $path" }

    # Integer delta (signed) vs literal replacement.
    if ($current -is [int] -or $current -is [long]) {
        $d = [int64]$delta
        $target.$field = [int64]$current + $d
    }
    else {
        # Non-integer: treat delta as literal replacement.
        $target.$field = $delta
    }

    Write-Host "[force-drift] $path : $current -> $($target.$field)" -ForegroundColor Yellow
}

function Add-Diff {
    param([System.Collections.Generic.List[hashtable]]$List, [string]$Field, $Expected, $Actual, [string]$Tolerance)
    $entry = @{
        field     = $Field
        expected  = $Expected
        actual    = $Actual
        tolerance = $Tolerance
    }
    $List.Add($entry)
}

function Compare-Scalar {
    param($Diffs, [string]$Field, $Expected, $Actual, [int]$ToleranceValue, [string]$ToleranceLabel)
    if ($Expected -is [string] -or $Actual -is [string]) {
        if ("$Expected" -ne "$Actual") {
            Add-Diff $Diffs $Field $Expected $Actual 'exact'
        }
        return
    }
    $diff = [Math]::Abs([double]$Actual - [double]$Expected)
    if ($diff -gt $ToleranceValue) {
        Add-Diff $Diffs $Field $Expected $Actual $ToleranceLabel
    }
}

function Compare-Rgba {
    param($Diffs, [string]$Field, $Expected, $Actual)
    $ePairs = @($Expected.r, $Expected.g, $Expected.b, $Expected.a)
    $aPairs = @($Actual.r,   $Actual.g,   $Actual.b,   $Actual.a)
    for ($i = 0; $i -lt 4; $i++) {
        if ($ePairs[$i] -ne $aPairs[$i]) {
            Add-Diff $Diffs "$Field[$i]" $ePairs[$i] $aPairs[$i] 'exact'
            return
        }
    }
}

function Compare-Layout {
    param($Diffs, $Expected, $Actual)
    $geomFields = @(
        'margin_v_px','margin_h_px','box_width_px','font_size_px',
        'padding_x_px','padding_y_px','radius_px','frame_width','frame_height'
    )
    foreach ($f in $geomFields) {
        Compare-Scalar $Diffs "layout.$f" $Expected.$f $Actual.$f $Tol.GeometryPx '1 px'
    }
    for ($i = 0; $i -lt 4; $i++) {
        if ($Expected.bg_rgba[$i] -ne $Actual.bg_rgba[$i]) {
            Add-Diff $Diffs "layout.bg_rgba[$i]" $Expected.bg_rgba[$i] $Actual.bg_rgba[$i] 'exact'; break
        }
    }
    for ($i = 0; $i -lt 4; $i++) {
        if ($Expected.fg_rgba[$i] -ne $Actual.fg_rgba[$i]) {
            Add-Diff $Diffs "layout.fg_rgba[$i]" $Expected.fg_rgba[$i] $Actual.fg_rgba[$i] 'exact'; break
        }
    }
    if ("$($Expected.font_family)" -ne "$($Actual.font_family)") {
        Add-Diff $Diffs 'layout.font_family' $Expected.font_family $Actual.font_family 'exact'
    }
}

function Compare-Blocks {
    param($Diffs, $Expected, $Actual)
    if ($Expected.Count -ne $Actual.Count) {
        Add-Diff $Diffs 'blocks.length' $Expected.Count $Actual.Count 'exact'
        return
    }
    for ($i = 0; $i -lt $Expected.Count; $i++) {
        $e = $Expected[$i]; $a = $Actual[$i]
        $prefix = "block_$i"
        Compare-Scalar $Diffs "$prefix.index"          $e.index          $a.index          0                 'exact'
        Compare-Scalar $Diffs "$prefix.start_us"       $e.start_us       $a.start_us       $Tol.TimingUs     '21 us'
        Compare-Scalar $Diffs "$prefix.end_us"         $e.end_us         $a.end_us         $Tol.TimingUs     '21 us'
        Compare-Scalar $Diffs "$prefix.font_css"       $e.font_css       $a.font_css       0                 'exact'
        Compare-Scalar $Diffs "$prefix.font_ass_name"  $e.font_ass_name  $a.font_ass_name  0                 'exact'
        Compare-Scalar $Diffs "$prefix.font_size_px"   $e.font_size_px   $a.font_size_px   $Tol.GeometryPx   '1 px'
        Compare-Scalar $Diffs "$prefix.padding_x_px"   $e.padding_x_px   $a.padding_x_px   $Tol.GeometryPx   '1 px'
        Compare-Scalar $Diffs "$prefix.padding_y_px"   $e.padding_y_px   $a.padding_y_px   $Tol.GeometryPx   '1 px'
        Compare-Scalar $Diffs "$prefix.radius_px"      $e.radius_px      $a.radius_px      $Tol.GeometryPx   '1 px'
        Compare-Scalar $Diffs "$prefix.margin_v_px"    $e.margin_v_px    $a.margin_v_px    $Tol.GeometryPx   '1 px'
        Compare-Scalar $Diffs "$prefix.text_width_px"  $e.text_width_px  $a.text_width_px  $Tol.GeometryPx   '1 px'
        Compare-Scalar $Diffs "$prefix.line_height_px" $e.line_height_px $a.line_height_px $Tol.GeometryPx   '1 px'
        Compare-Scalar $Diffs "$prefix.frame_width"    $e.frame_width    $a.frame_width    $Tol.GeometryPx   '1 px'
        Compare-Scalar $Diffs "$prefix.frame_height"   $e.frame_height   $a.frame_height   $Tol.GeometryPx   '1 px'
        Compare-Rgba   $Diffs "$prefix.text_color"     $e.text_color     $a.text_color
        Compare-Rgba   $Diffs "$prefix.background"     $e.background     $a.background
        # Lines: exact match string-by-string.
        if ($e.lines.Count -ne $a.lines.Count) {
            Add-Diff $Diffs "$prefix.lines.length" $e.lines.Count $a.lines.Count 'exact'
        } else {
            for ($j = 0; $j -lt $e.lines.Count; $j++) {
                if ($e.lines[$j] -ne $a.lines[$j]) {
                    Add-Diff $Diffs "$prefix.lines[$j]" $e.lines[$j] $a.lines[$j] 'exact'
                }
            }
        }
    }
}

function Convert-AssTimeToUs {
    param([string]$Time)
    if ($Time -notmatch '^(\d+):(\d{2}):(\d{2})\.(\d{2})$') { throw "bad ASS time: $Time" }
    $h = [int]$Matches[1]; $m = [int]$Matches[2]; $s = [int]$Matches[3]; $cs = [int]$Matches[4]
    return ([int64]$h * 3600 + [int64]$m * 60 + [int64]$s) * 1000000 + [int64]$cs * 10000
}

function Parse-AssDialogues {
    param([string]$Ass)
    $result = New-Object System.Collections.Generic.List[hashtable]
    foreach ($raw in ($Ass -split "`n")) {
        $line = $raw.TrimEnd("`r")
        if (-not $line.StartsWith('Dialogue:')) { continue }
        $payload = $line.Substring('Dialogue:'.Length).TrimStart()
        # Dialogue format: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text
        # Text is the 10th field and may contain commas — split with a limit.
        $parts = $payload.Split(',', 10)
        if ($parts.Count -lt 10) { continue }
        $result.Add(@{
            start_us  = Convert-AssTimeToUs $parts[1]
            end_us    = Convert-AssTimeToUs $parts[2]
            margin_v  = [int]$parts[7]
            text      = $parts[9]
        })
    }
    return ,$result
}

function Compare-AssAgainstBlocks {
    param($Diffs, $Actual)
    $dialogues = Parse-AssDialogues -Ass $Actual.ass
    if ($dialogues.Count -ne $Actual.blocks.Count) {
        Add-Diff $Diffs 'ass.dialogues.length' $Actual.blocks.Count $dialogues.Count 'exact'
        return
    }
    for ($i = 0; $i -lt $dialogues.Count; $i++) {
        $d = $dialogues[$i]; $b = $Actual.blocks[$i]
        $prefix = "ass[$i]"
        Compare-Scalar $Diffs "$prefix.start_us"  $b.start_us  $d.start_us  $Tol.TimingUs    '21 us'
        Compare-Scalar $Diffs "$prefix.end_us"    $b.end_us    $d.end_us    $Tol.TimingUs    '21 us'
        Compare-Scalar $Diffs "$prefix.margin_v"  $b.margin_v_px $d.margin_v $Tol.GeometryPx '1 px'
        $expectedText = ($b.lines -join '\N')
        if ($d.text -ne $expectedText) {
            Add-Diff $Diffs "$prefix.text" $expectedText $d.text 'exact'
        }
    }
}

# --------------------- main ---------------------

Write-Section 'caption-parity-eval'
$timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$runDir    = Join-Path $OutputRoot $timestamp
New-Item -ItemType Directory -Force -Path $runDir | Out-Null

$dirs = Resolve-FixtureDirs
$fixtureReports = New-Object System.Collections.Generic.List[hashtable]
$overallPass = $true

foreach ($d in $dirs) {
    $name = $d.Name
    $inputPath    = Join-Path $d.FullName 'input.json'
    $expectedPath = Join-Path $d.FullName 'expected.json'
    if (-not (Test-Path $inputPath)) {
        Write-Host "[skip] $name — no input.json" -ForegroundColor DarkYellow
        continue
    }

    $actualPath = Join-Path $runDir "$name.actual.json"
    Invoke-Dump -InputPath $inputPath -OutputPath $actualPath
    $actual = Read-Json -Path $actualPath
    Apply-ForceDrift -Actual $actual -Spec $ForceDrift

    if ($Regenerate) {
        $actual | ConvertTo-Json -Depth 20 | Set-Content -Path $expectedPath -NoNewline
        Write-Host "[regen] $name -> $expectedPath" -ForegroundColor Green
        $fixtureReports.Add(@{ id = $name; pass = $true; regenerated = $true; diffs = @() })
        continue
    }

    if (-not (Test-Path $expectedPath)) {
        Write-Host "[skip] $name — no expected.json (run with -Regenerate)" -ForegroundColor DarkYellow
        $fixtureReports.Add(@{ id = $name; pass = $false; diffs = @(@{ field='expected.json'; expected='present'; actual='missing'; tolerance='exact' }) })
        $overallPass = $false
        continue
    }

    $expected = Read-Json -Path $expectedPath
    $diffs = New-Object System.Collections.Generic.List[hashtable]
    Compare-Layout -Diffs $diffs -Expected $expected.layout -Actual $actual.layout
    Compare-Blocks -Diffs $diffs -Expected $expected.blocks -Actual $actual.blocks
    Compare-AssAgainstBlocks -Diffs $diffs -Actual $actual

    $pass = ($diffs.Count -eq 0)
    if (-not $pass) { $overallPass = $false }
    $status = if ($pass) { 'PASS' } else { 'FAIL' }
    $colour = if ($pass) { 'Green' } else { 'Red' }
    Write-Host "[$status] $name ($($diffs.Count) diffs)" -ForegroundColor $colour

    $fixtureReports.Add(@{ id = $name; pass = $pass; diffs = @($diffs) })
}

$summary = @{
    total  = @($fixtureReports).Count
    passed = @($fixtureReports | Where-Object { $_.pass }).Count
    failed = @($fixtureReports | Where-Object { -not $_.pass }).Count
}
$report = [ordered]@{
    pass     = $overallPass
    timestamp = $timestamp
    tolerances = $Tol
    fixtures = $fixtureReports
    summary  = $summary
}

$reportJson = Join-Path $runDir 'report.json'
$report | ConvertTo-Json -Depth 20 | Set-Content -Path $reportJson -NoNewline
Write-Host "report: $reportJson"

# Small markdown report for humans.
$md = New-Object System.Text.StringBuilder
[void]$md.AppendLine("# caption-parity-eval — $timestamp")
[void]$md.AppendLine("")
[void]$md.AppendLine("Overall: **$(if($overallPass){'PASS'}else{'FAIL'})** ($($summary.passed)/$($summary.total))")
[void]$md.AppendLine("")
[void]$md.AppendLine("Tolerances: geometry = $($Tol.GeometryPx) px, timing = $($Tol.TimingUs) us (1 sample @ 48 kHz).")
[void]$md.AppendLine("")
foreach ($f in $fixtureReports) {
    $tag = if ($f.pass) { 'PASS' } else { 'FAIL' }
    [void]$md.AppendLine("## $($f.id) — $tag")
    $fDiffs = @($f.diffs)
    if ($fDiffs.Count -gt 0) {
        [void]$md.AppendLine("")
        [void]$md.AppendLine("| field | expected | actual | tolerance |")
        [void]$md.AppendLine("|---|---|---|---|")
        foreach ($diff in $fDiffs) {
            [void]$md.AppendLine("| $($diff.field) | $($diff.expected) | $($diff.actual) | $($diff.tolerance) |")
        }
    }
    [void]$md.AppendLine("")
}
$mdPath = Join-Path $runDir 'report.md'
Set-Content -Path $mdPath -Value $md.ToString() -NoNewline
Write-Host "report.md: $mdPath"

if ($overallPass) { exit 0 } else { exit 1 }

