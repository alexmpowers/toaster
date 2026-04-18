<#
.SYNOPSIS
    Audio-boundary eval helpers: PCM IO, normalized cross-correlation,
    spectral-discontinuity z-score, Word Error Rate, sample-boundary error.

.DESCRIPTION
    Used by scripts/eval-audio-boundary.ps1 and anything else needing
    sample-accurate splice measurements. All helpers operate on
    f32 little-endian mono PCM at a caller-provided sample rate
    (the runner standardizes on 48 kHz).

    Rationale for PowerShell (not a Rust CLI):
      - Avoids cargo lock contention with in-flight toolchain work
      - Keeps the harness hermetic aside from FFmpeg (already required
        by every other eval script in this repo)
      - Core ops are O(N) over samples; PowerShell is adequate for
        <= ~5s fixtures at 48 kHz (240k samples).
#>

Set-StrictMode -Off
$script:DefaultSampleRate = 48000

function Assert-FFmpeg {
    foreach ($cmd in @('ffmpeg', 'ffprobe')) {
        if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
            throw "Required executable '$cmd' not on PATH."
        }
    }
}

function ConvertTo-F32MonoPcm {
    <#
    .SYNOPSIS Decode any media file to f32 LE mono PCM at -SampleRate.
    .OUTPUTS float[] array of samples.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)] [string]$Path,
        [int]$SampleRate = $script:DefaultSampleRate
    )
    Assert-FFmpeg
    if (-not (Test-Path $Path)) { throw "PCM source not found: $Path" }

    $tmp = [System.IO.Path]::GetTempFileName() + '.f32'
    try {
        & ffmpeg -hide_banner -loglevel error -y -i $Path `
            -vn -ac 1 -ar $SampleRate -f f32le $tmp 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path $tmp)) {
            throw "ffmpeg failed to decode: $Path"
        }
        $bytes = [System.IO.File]::ReadAllBytes($tmp)
        $count = [int]($bytes.Length / 4)
        $samples = New-Object 'single[]' $count
        [System.Buffer]::BlockCopy($bytes, 0, $samples, 0, $count * 4)
        return ,$samples
    } finally {
        if (Test-Path $tmp) { Remove-Item $tmp -Force -ErrorAction SilentlyContinue }
    }
}

function Get-PcmSlice {
    param(
        [Parameter(Mandatory)] [array]$Samples,
        [int]$StartSample = 0,
        [int]$Length
    )
    if ($StartSample -lt 0) { $StartSample = 0 }
    if ($StartSample -ge $Samples.Length) { return ,@() }
    $end = [Math]::Min($Samples.Length, $StartSample + $Length)
    $n = $end - $StartSample
    $out = New-Object 'single[]' $n
    [System.Array]::Copy($Samples, $StartSample, $out, 0, $n)
    return ,$out
}

function Get-NormalizedCrossCorrelation {
    param(
        $ArrA,
        $ArrB,
        [int]$MaxLagSamples = -1
    )
    if ($null -eq $ArrA -or $null -eq $ArrB) { return 0.0 }
    if ($ArrA.Length -eq 0 -or $ArrB.Length -eq 0) { return 0.0 }

    # Treat the shorter buffer as the template; slide it across the longer.
    if ($ArrB.Length -gt $ArrA.Length) { $swap = $ArrA; $ArrA = $ArrB; $ArrB = $swap }
    $na = [int]$ArrA.Length; $nb = [int]$ArrB.Length
    $maxLag = $na - $nb
    if ($MaxLagSamples -ge 0 -and $MaxLagSamples -lt $maxLag) { $maxLag = $MaxLagSamples }
    if ($maxLag -lt 0) { $maxLag = 0 }

    $energyB = 0.0
    for ($i = 0; $i -lt $nb; $i++) {
        $vb = [double]$ArrB[$i]
        $energyB = $energyB + $vb * $vb
    }
    if ($energyB -le 0) { return 0.0 }

    $best = 0.0
    $bestLag = 0
    $stride = [int][Math]::Max(1, [Math]::Floor($maxLag / 2048.0))
    for ($lag = 0; $lag -le $maxLag; $lag += $stride) {
        $dot = 0.0; $energyA = 0.0
        for ($i = 0; $i -lt $nb; $i++) {
            $va = [double]$ArrA[$lag + $i]
            $dot = $dot + $va * [double]$ArrB[$i]
            $energyA = $energyA + $va * $va
        }
        if ($energyA -le 0) { continue }
        $denom = [Math]::Sqrt($energyA * $energyB)
        if ($denom -le 0) { continue }
        $rr = [Math]::Abs($dot / $denom)
        if ($rr -gt $best) { $best = $rr; $bestLag = $lag }
    }
    if ($stride -gt 1) {
        $lo = [Math]::Max(0, $bestLag - $stride)
        $hi = [Math]::Min($maxLag, $bestLag + $stride)
        for ($lag = $lo; $lag -le $hi; $lag++) {
            $dot = 0.0; $energyA = 0.0
            for ($i = 0; $i -lt $nb; $i++) {
                $va = [double]$ArrA[$lag + $i]
                $dot = $dot + $va * [double]$ArrB[$i]
                $energyA = $energyA + $va * $va
            }
            if ($energyA -le 0) { continue }
            $denom = [Math]::Sqrt($energyA * $energyB)
            if ($denom -le 0) { continue }
            $rr = [Math]::Abs($dot / $denom)
            if ($rr -gt $best) { $best = $rr }
        }
    }
    return [double]$best
}

function Get-SeamZScore {
    <#
    .SYNOPSIS
        Measure sample-discontinuity and HF-band burst z-score at a seam.
        Returns the max of (|diff| z-score, HF-energy z-score) vs a
        baseline window of BaselineMs on either side of the seam.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)] [array]$Samples,
        [Parameter(Mandatory)] [int]$SeamSample,
        [int]$SampleRate = $script:DefaultSampleRate,
        [double]$SeamMs = 5.0,
        [double]$BaselineMs = 200.0
    )
    $seamHalf = [int]([double]$SeamMs * 0.001 * $SampleRate)
    $baseHalf = [int]([double]$BaselineMs * 0.001 * $SampleRate)
    $seamStart = [Math]::Max(1, $SeamSample - $seamHalf)
    $seamEnd   = [Math]::Min($Samples.Length - 1, $SeamSample + $seamHalf)

    # First-difference magnitude in the seam window
    $seamMaxDiff = 0.0
    for ($i = $seamStart; $i -le $seamEnd; $i++) {
        $d = [Math]::Abs([double]$Samples[$i] - [double]$Samples[$i - 1])
        if ($d -gt $seamMaxDiff) { $seamMaxDiff = $d }
    }

    # Baseline: samples within BaselineMs of seam but OUTSIDE seam window
    $baseLo = [Math]::Max(1, $SeamSample - $baseHalf)
    $baseHi = [Math]::Min($Samples.Length - 1, $SeamSample + $baseHalf)
    $sum = 0.0; $sumSq = 0.0; $n = 0
    for ($i = $baseLo; $i -le $baseHi; $i++) {
        if ($i -ge $seamStart -and $i -le $seamEnd) { continue }
        $d = [Math]::Abs([double]$Samples[$i] - [double]$Samples[$i - 1])
        $sum += $d; $sumSq += $d * $d; $n++
    }
    if ($n -lt 4) { return 0.0 }
    $mean = $sum / $n
    $var = ($sumSq / $n) - ($mean * $mean)
    $sd = if ($var -gt 1e-12) { [Math]::Sqrt($var) } else { 1e-6 }
    $zDiff = ($seamMaxDiff - $mean) / $sd

    # HF-band proxy: second-difference energy (cheap high-pass). Sufficient
    # for catching click/pop bursts without a full FFT.
    $seamHf = 0.0
    for ($i = $seamStart + 1; $i -le $seamEnd; $i++) {
        $d2 = [double]$Samples[$i] - 2.0 * [double]$Samples[$i - 1] + [double]$Samples[$i - 2]
        $seamHf += $d2 * $d2
    }
    $seamHf = if ($seamEnd - $seamStart -gt 1) { $seamHf / ($seamEnd - $seamStart - 1) } else { 0.0 }

    $bSum = 0.0; $bSumSq = 0.0; $bN = 0
    for ($i = $baseLo + 1; $i -le $baseHi; $i++) {
        if ($i -ge $seamStart -and $i -le $seamEnd) { continue }
        $d2 = [double]$Samples[$i] - 2.0 * [double]$Samples[$i - 1] + [double]$Samples[$i - 2]
        $e = $d2 * $d2
        $bSum += $e; $bSumSq += $e * $e; $bN++
    }
    $zHf = 0.0
    if ($bN -gt 4) {
        $bMean = $bSum / $bN
        $bVar = ($bSumSq / $bN) - ($bMean * $bMean)
        $bSd = if ($bVar -gt 1e-18) { [Math]::Sqrt($bVar) } else { 1e-9 }
        $zHf = ($seamHf - $bMean) / $bSd
    }

    return [double]([Math]::Max($zDiff, $zHf))
}

function Get-Wer {
    <#
    .SYNOPSIS
        Compute Word Error Rate (insertions + deletions + substitutions) / ref-len.
    #>
    param(
        $Reference,
        $Hypothesis
    )
    $refList = [System.Collections.Generic.List[string]]::new()
    foreach ($w in @($Reference)) { [void]$refList.Add(([string]$w).ToLowerInvariant()) }
    $hypList = [System.Collections.Generic.List[string]]::new()
    foreach ($w in @($Hypothesis)) { [void]$hypList.Add(([string]$w).ToLowerInvariant()) }
    [int]$m = $refList.Count
    [int]$n = $hypList.Count
    if ($m -eq 0) {
        $wer0 = if ($n -eq 0) { 0.0 } else { 1.0 }
        return [pscustomobject]@{ wer = [double]$wer0; sub = 0; ins = [int]$n; del = 0 }
    }
    $d = New-Object 'int[,]' ($m + 1), ($n + 1)
    for ([int]$ii = 0; $ii -le $m; $ii++) { $d.SetValue([int]$ii, $ii, 0) }
    for ([int]$jj = 0; $jj -le $n; $jj++) { $d.SetValue([int]$jj, 0, $jj) }
    for ([int]$ii = 1; $ii -le $m; $ii++) {
        for ([int]$jj = 1; $jj -le $n; $jj++) {
            $cost = if ($refList[$ii - 1] -eq $hypList[$jj - 1]) { 0 } else { 1 }
            [int]$delC = [int]$d.GetValue($ii - 1, $jj) + 1
            [int]$insC = [int]$d.GetValue($ii, $jj - 1) + 1
            [int]$subC = [int]$d.GetValue($ii - 1, $jj - 1) + $cost
            [int]$bestC = [Math]::Min([Math]::Min($delC, $insC), $subC)
            $d.SetValue($bestC, $ii, $jj)
        }
    }
    [int]$ii = $m; [int]$jj = $n
    [int]$subs = 0; [int]$inss = 0; [int]$dels = 0
    while ($ii -gt 0 -or $jj -gt 0) {
        [int]$cur = if ($ii -gt 0 -and $jj -gt 0) { [int]$d.GetValue($ii, $jj) } elseif ($ii -gt 0) { [int]$d.GetValue($ii, 0) } else { [int]$d.GetValue(0, $jj) }
        if ($ii -gt 0 -and $jj -gt 0 -and $refList[$ii - 1] -eq $hypList[$jj - 1] -and $cur -eq [int]$d.GetValue($ii - 1, $jj - 1)) {
            $ii = $ii - 1; $jj = $jj - 1
        } elseif ($ii -gt 0 -and $jj -gt 0 -and $cur -eq [int]$d.GetValue($ii - 1, $jj - 1) + 1) {
            $subs = $subs + 1; $ii = $ii - 1; $jj = $jj - 1
        } elseif ($jj -gt 0 -and $cur -eq [int]$d.GetValue($ii, $jj - 1) + 1) {
            $inss = $inss + 1; $jj = $jj - 1
        } else {
            $dels = $dels + 1; $ii = $ii - 1
        }
    }
    $wer = ($subs + $inss + $dels) / [double]$m
    return [pscustomobject]@{ wer = [double]$wer; sub = [int]$subs; ins = [int]$inss; del = [int]$dels }
}

function Get-SampleBoundaryError {
    <#
    .SYNOPSIS
        Given expected keep-segments (seconds) and an exported PCM length,
        compute per-boundary and total-length sample-error.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)] [object[]]$KeepSegments,  # array of @{start=..; end=..} in seconds
        [Parameter(Mandatory)] [int]$ExportedSampleCount,
        [int]$SampleRate = $script:DefaultSampleRate
    )
    $expectedTotal = 0
    $boundaries = New-Object 'System.Collections.Generic.List[object]'
    $cursor = 0
    foreach ($seg in $KeepSegments) {
        $startSamp = [int][Math]::Round([double]$seg.start * $SampleRate)
        $endSamp   = [int][Math]::Round([double]$seg.end   * $SampleRate)
        $len = $endSamp - $startSamp
        $boundaries.Add([pscustomobject]@{
            expected_start_sample  = $startSamp
            expected_end_sample    = $endSamp
            export_start_sample    = $cursor
            export_end_sample      = $cursor + $len
            length_samples         = $len
        })
        $cursor += $len
        $expectedTotal += $len
    }
    return [pscustomobject]@{
        expected_total_samples = $expectedTotal
        exported_total_samples = $ExportedSampleCount
        total_error_samples    = $ExportedSampleCount - $expectedTotal
        boundaries             = $boundaries.ToArray()
    }
}

Export-ModuleMember -Function `
    Assert-FFmpeg, `
    ConvertTo-F32MonoPcm, `
    Get-PcmSlice, `
    Get-NormalizedCrossCorrelation, `
    Get-SeamZScore, `
    Get-Wer, `
    Get-SampleBoundaryError

