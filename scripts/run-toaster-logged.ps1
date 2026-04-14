[CmdletBinding()]
param(
    [Parameter()]
    [string]$AppPath,

    [Parameter()]
    [string]$WorkingDirectory,

    [Parameter()]
    [string]$LogPath,

    [Parameter()]
    [switch]$RequireAutomationMedia
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'toaster-runner.ps1')

function Write-MonitorLine {
    param(
        [string]$MonitorLogPath,
        [string]$Status,
        [Nullable[int]]$ProcessId = $null
    )

    $timestamp = Get-Date -Format 'o'
    $pidText = if ($null -ne $ProcessId) { $ProcessId } else { 'n/a' }
    Add-Content -LiteralPath $MonitorLogPath -Value ("{0} PID={1} STATUS={2}" -f $timestamp, $pidText, $Status)
}

$repoRoot = Split-Path -Path $PSScriptRoot -Parent

if ([string]::IsNullOrWhiteSpace($AppPath)) {
    $AppPath = Join-Path $repoRoot 'build\bin\toaster-app.exe'
}

$resolvedAppPath = Resolve-ToasterPath -Path $AppPath
$resolvedWorkingDirectory = if ([string]::IsNullOrWhiteSpace($WorkingDirectory)) {
    Split-Path -Parent $resolvedAppPath
} else {
    Resolve-ToasterPath -Path $WorkingDirectory -AllowMissing
}

if (-not (Test-Path -LiteralPath $resolvedWorkingDirectory)) {
    New-Item -ItemType Directory -Path $resolvedWorkingDirectory -Force | Out-Null
}

$logsDirectory = Join-Path $repoRoot 'build\logs'
New-Item -ItemType Directory -Path $logsDirectory -Force | Out-Null

if ([string]::IsNullOrWhiteSpace($LogPath)) {
    $logStamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
    $monitorLogPath = Join-Path $logsDirectory ("toaster-app-{0}-monitor.log" -f $logStamp)
    $stdoutLogPath = Join-Path $logsDirectory ("toaster-app-{0}-stdout.log" -f $logStamp)
    $stderrLogPath = Join-Path $logsDirectory ("toaster-app-{0}-stderr.log" -f $logStamp)
} else {
    $monitorLogPath = Resolve-ToasterPath -Path $LogPath -AllowMissing
    Ensure-ToasterParentDirectory -Path $monitorLogPath
    $logDirectory = Split-Path -Parent $monitorLogPath
    $logBaseName = [System.IO.Path]::GetFileNameWithoutExtension($monitorLogPath)

    if ([string]::IsNullOrWhiteSpace($logBaseName)) {
        $logBaseName = [System.IO.Path]::GetFileName($monitorLogPath)
    }

    $stdoutLogPath = Join-Path $logDirectory ($logBaseName + '.stdout.log')
    $stderrLogPath = Join-Path $logDirectory ($logBaseName + '.stderr.log')
}

Write-MonitorLine -MonitorLogPath $monitorLogPath -Status ("launching file={0}" -f $resolvedAppPath)
Write-Output ("App path: {0}" -f $resolvedAppPath)
Write-Output ("Working directory: {0}" -f $resolvedWorkingDirectory)
Write-Output ("Stdout log: {0}" -f $stdoutLogPath)
Write-Output ("Stderr log: {0}" -f $stderrLogPath)
Write-Output ("Monitor log: {0}" -f $monitorLogPath)

$previousQtLogging = $env:QT_FORCE_STDERR_LOGGING
$env:QT_FORCE_STDERR_LOGGING = '1'

try {
    if ($RequireAutomationMedia -or -not [string]::IsNullOrWhiteSpace($env:TOASTER_AUTOMATION_MEDIA)) {
        $exitCode = Invoke-ToasterRun `
            -AppPath $resolvedAppPath `
            -WorkingDirectory $resolvedWorkingDirectory `
            -RequireAutomationMedia:$RequireAutomationMedia `
            -StdoutPath $stdoutLogPath `
            -StderrPath $stderrLogPath

        Write-MonitorLine -MonitorLogPath $monitorLogPath -Status ("automation_exit_code={0}" -f $exitCode)
        if ($exitCode -eq 125) {
            exit 125
        }

        if ($exitCode -ne 0) {
            throw "Toaster automation failed with exit code $exitCode. See '$stderrLogPath' and '$monitorLogPath'."
        }

        $projectPath = if ([string]::IsNullOrWhiteSpace($env:TOASTER_AUTOMATION_PROJECT)) {
            Join-Path $resolvedWorkingDirectory 'automation\frontend-smoke.toaster'
        } else {
            Resolve-ToasterPath -Path $env:TOASTER_AUTOMATION_PROJECT -AllowMissing
        }

        if (Test-Path -LiteralPath $projectPath) {
            $wordCount = (Select-String -Path $projectPath -Pattern '^WORD\t').Count
            Write-Output ("Automation project: {0}" -f $projectPath)
            Write-Output ("Word count: {0}" -f $wordCount)
            Write-MonitorLine -MonitorLogPath $monitorLogPath -Status ("automation_word_count={0}" -f $wordCount)
        }

        exit 0
    }

    $process = Start-Process `
        -FilePath $resolvedAppPath `
        -WorkingDirectory $resolvedWorkingDirectory `
        -RedirectStandardOutput $stdoutLogPath `
        -RedirectStandardError $stderrLogPath `
        -PassThru

    Write-MonitorLine -MonitorLogPath $monitorLogPath -ProcessId $process.Id -Status 'started'
    Write-Output ("Started Toaster (PID {0})." -f $process.Id)

    Start-Sleep -Milliseconds 1500
    $process.Refresh()

    if ($process.HasExited) {
        Write-MonitorLine -MonitorLogPath $monitorLogPath -ProcessId $process.Id -Status ("exited_early exit_code={0}" -f $process.ExitCode)
        throw "Toaster exited shortly after launch (PID $($process.Id), exit code $($process.ExitCode)). See the log files above."
    }

    Write-MonitorLine -MonitorLogPath $monitorLogPath -ProcessId $process.Id -Status 'running_after_startup_check'
    Write-Output ("Startup check: process {0} is still running." -f $process.Id)
} finally {
    if ([string]::IsNullOrWhiteSpace($previousQtLogging)) {
        Remove-Item Env:QT_FORCE_STDERR_LOGGING -ErrorAction SilentlyContinue
    } else {
        $env:QT_FORCE_STDERR_LOGGING = $previousQtLogging
    }
}
