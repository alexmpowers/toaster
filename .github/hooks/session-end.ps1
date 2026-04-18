# Session-end marker.
$ErrorActionPreference = 'Stop'
try {
    $raw = [Console]::In.ReadToEnd()
    $payload = if ([string]::IsNullOrWhiteSpace($raw)) { $null } else { $raw | ConvertFrom-Json }
    $reason = if ($payload) { [string]$payload.reason } else { '' }

    $logDir = Join-Path $env:USERPROFILE '.copilot'
    if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir -Force | Out-Null }
    $logPath = Join-Path $logDir 'toaster-prompts.log'
    $ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    Add-Content -Path $logPath -Value "$ts session-end reason=$reason" -Encoding UTF8
} catch {
    [Console]::Error.WriteLine("session-end: soft-fail: $($_.Exception.Message)")
}
exit 0
