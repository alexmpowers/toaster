<#
.SYNOPSIS
    Evaluates transcription word-timestamp accuracy against oracle fixtures.

.DESCRIPTION
    Reads oracle fixture files from eval/transcription-accuracy/fixtures/,
    loads the corresponding .toaster project file, and compares each word's
    start_us/end_us against the oracle ground-truth timestamps.

    Reports per-fixture metrics (median error, p95 error, max error, word
    match rate) and an overall pass/fail based on tolerance thresholds
    defined in each fixture.

.PARAMETER FixturesDir
    Path to the oracle fixtures directory.
    Defaults to eval/transcription-accuracy/fixtures/.

.PARAMETER OutputJson
    Path to write the JSON report.
    Defaults to .eval-output/transcription-accuracy-report.json.
#>

[CmdletBinding()]
param(
    [string]$FixturesDir = '',
    [string]$OutputJson = ''
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '..\..')
Set-Location $RepoRoot

if (-not $FixturesDir) {
    $FixturesDir = Join-Path $RepoRoot 'eval\transcription-accuracy\fixtures'
}
if (-not $OutputJson) {
    $OutputJson = Join-Path $RepoRoot '.eval-output\transcription-accuracy-report.json'
}

$outDir = Split-Path -Parent $OutputJson
if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

function Get-Percentile {
    param([double[]]$Values, [double]$Percentile)
    if ($Values.Count -eq 0) { return 0 }
    $sorted = $Values | Sort-Object
    $index = [math]::Ceiling($Percentile / 100.0 * $sorted.Count) - 1
    $index = [math]::Max(0, [math]::Min($index, $sorted.Count - 1))
    return $sorted[$index]
}

function Get-Median {
    param([double[]]$Values)
    return Get-Percentile -Values $Values -Percentile 50
}

# Discover oracle fixtures
$fixtureFiles = Get-ChildItem -Path $FixturesDir -Filter '*.json' -ErrorAction SilentlyContinue
if (-not $fixtureFiles -or $fixtureFiles.Count -eq 0) {
    Write-Host "No oracle fixtures found in $FixturesDir"
    $report = [ordered]@{
        timestamp = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
        fixtures_dir = $FixturesDir
        fixtures = @()
        overall = 'skip'
        notes = 'No oracle fixtures found'
    }
    $report | ConvertTo-Json -Depth 6 | Set-Content -Path $OutputJson -Encoding UTF8
    exit 0
}

$fixtureResults = @()
$overallPass = $true

foreach ($fixtureFile in $fixtureFiles) {
    Write-Host "Evaluating fixture: $($fixtureFile.Name)"
    $fixture = Get-Content $fixtureFile.FullName -Raw | ConvertFrom-Json

    # Resolve source project path relative to fixture file
    $projectPath = Join-Path $fixtureFile.DirectoryName $fixture.source_project
    $projectPath = [System.IO.Path]::GetFullPath($projectPath)

    if (-not (Test-Path $projectPath)) {
        Write-Host "  SKIP: Source project not found: $projectPath"
        $fixtureResults += [ordered]@{
            id = $fixture.id
            status = 'skip'
            notes = "Source project not found: $projectPath"
        }
        continue
    }

    $project = Get-Content $projectPath -Raw | ConvertFrom-Json
    $actualWords = $project.words
    $oracleWords = $fixture.words

    if ($actualWords.Count -eq 0) {
        $fixtureResults += [ordered]@{
            id = $fixture.id
            status = 'skip'
            notes = 'No words in project file'
        }
        continue
    }

    # Match words by position (order-based matching)
    $matchCount = [math]::Min($actualWords.Count, $oracleWords.Count)
    $startErrors = @()
    $endErrors = @()
    $textMatches = 0
    $textMismatches = @()

    for ($i = 0; $i -lt $matchCount; $i++) {
        $actual = $actualWords[$i]
        $oracle = $oracleWords[$i]

        # Text matching (case-insensitive, trim punctuation for comparison)
        $actualText = $actual.text -replace '[^\w]', ''
        $oracleText = $oracle.text -replace '[^\w]', ''
        if ($actualText -ieq $oracleText) {
            $textMatches++
        } else {
            $textMismatches += [ordered]@{
                index = $i
                actual = $actual.text
                oracle = $oracle.text
            }
        }

        # Timestamp error (absolute difference in microseconds)
        $startErr = [math]::Abs($actual.start_us - $oracle.oracle_start_us)
        $endErr = [math]::Abs($actual.end_us - $oracle.oracle_end_us)
        $startErrors += $startErr
        $endErrors += $endErr
    }

    # Compute combined error (max of start and end error per word)
    $combinedErrors = @()
    for ($i = 0; $i -lt $matchCount; $i++) {
        $combinedErrors += [math]::Max($startErrors[$i], $endErrors[$i])
    }

    $medianStartError = Get-Median -Values $startErrors
    $medianEndError = Get-Median -Values $endErrors
    $medianCombined = Get-Median -Values $combinedErrors
    $p95Combined = Get-Percentile -Values $combinedErrors -Percentile 95
    $maxCombined = ($combinedErrors | Measure-Object -Maximum).Maximum

    $wordMatchRate = if ($matchCount -gt 0) { [math]::Round($textMatches / $matchCount, 3) } else { 0 }

    # Check tolerances
    $medianTolerance = $fixture.tolerances.median_error_us
    $p95Tolerance = $fixture.tolerances.p95_error_us
    $medianPass = $medianCombined -le $medianTolerance
    $p95Pass = $p95Combined -le $p95Tolerance
    $fixturePass = $medianPass -and $p95Pass

    if (-not $fixturePass) { $overallPass = $false }

    $statusIcon = if ($fixturePass) { "PASS" } else { "FAIL" }
    Write-Host ("  {0}: median={1:N0}us (tol={2:N0}), p95={3:N0}us (tol={4:N0}), match={5:P0}" -f `
        $statusIcon, $medianCombined, $medianTolerance, $p95Combined, $p95Tolerance, $wordMatchRate)

    if ($textMismatches.Count -gt 0) {
        Write-Host "  Text mismatches: $($textMismatches.Count) words differ"
    }

    $fixtureResults += [ordered]@{
        id = $fixture.id
        status = if ($fixturePass) { 'pass' } else { 'fail' }
        engine = $fixture.engine
        word_count = $matchCount
        word_match_rate = $wordMatchRate
        metrics = [ordered]@{
            median_start_error_us = [math]::Round($medianStartError)
            median_end_error_us = [math]::Round($medianEndError)
            median_combined_error_us = [math]::Round($medianCombined)
            p95_combined_error_us = [math]::Round($p95Combined)
            max_combined_error_us = [math]::Round($maxCombined)
        }
        tolerances = [ordered]@{
            median_error_us = $medianTolerance
            p95_error_us = $p95Tolerance
        }
        gates = [ordered]@{
            median_pass = $medianPass
            p95_pass = $p95Pass
        }
        text_mismatches = $textMismatches
    }
}

$overall = if ($overallPass) { 'pass' } else { 'fail' }

$report = [ordered]@{
    timestamp = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    commit = (& git rev-parse HEAD 2>$null)
    fixtures_dir = $FixturesDir
    fixture_count = $fixtureFiles.Count
    fixtures = $fixtureResults
    overall = $overall
}

$report | ConvertTo-Json -Depth 6 | Set-Content -Path $OutputJson -Encoding UTF8

Write-Host "`nTranscription accuracy eval: $overall ($($fixtureFiles.Count) fixtures)"
$fixtureResults | ForEach-Object {
    $icon = switch ($_.status) { 'pass' {'✓'} 'fail' {'✗'} 'skip' {'⊘'} default {'?'} }
    Write-Host ("  {0} {1}" -f $icon, $_.id)
}

if ($overall -ne 'pass') { exit 1 }
exit 0
