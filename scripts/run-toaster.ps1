[CmdletBinding()]
param(
    [Parameter()]
    [string]$AppPath,

    [Parameter()]
    [string]$WorkingDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'toaster-runner.ps1')

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

$process = Start-Process -FilePath $resolvedAppPath -WorkingDirectory $resolvedWorkingDirectory -PassThru

Write-Output ("Started Toaster (PID {0})." -f $process.Id)
Write-Output ("App path: {0}" -f $resolvedAppPath)
Write-Output ("Working directory: {0}" -f $resolvedWorkingDirectory)

Start-Sleep -Milliseconds 1500
$process.Refresh()

if ($process.HasExited) {
    throw "Toaster exited shortly after launch (PID $($process.Id), exit code $($process.ExitCode))."
}

Write-Output ("Startup check: process {0} is still running." -f $process.Id)
