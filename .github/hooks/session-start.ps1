# Session-start hook. Observational only (stderr + log file).
# Rules observed: AGENTS.md Windows rebuild-lock + Launch protocol.
$ErrorActionPreference = 'Stop'

function Warn([string]$msg) { [Console]::Error.WriteLine($msg) }

try {
    $raw = [Console]::In.ReadToEnd()
    $payload = if ([string]::IsNullOrWhiteSpace($raw)) { $null } else { $raw | ConvertFrom-Json }

    $logDir = Join-Path $env:USERPROFILE '.copilot'
    if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir -Force | Out-Null }
    $logPath = Join-Path $logDir 'toaster-prompts.log'

    $ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $cwd = if ($payload) { [string]$payload.cwd } else { '' }
    $src = if ($payload) { [string]$payload.source } else { '' }
    $prompt = if ($payload) { [string]$payload.initialPrompt } else { '' }
    $line = "$ts session-start source=$src cwd=$cwd"
    Add-Content -Path $logPath -Value $line -Encoding UTF8
    if ($prompt) {
        Add-Content -Path $logPath -Value "# initialPrompt: $prompt" -Encoding UTF8
    }

    # Rebuild-lock check.
    $procs = Get-Process -Name toaster-app,toaster -ErrorAction Ignore
    if ($procs) {
        $ids = ($procs | ForEach-Object { $_.Id }) -join ','
        Warn "WARNING: toaster-app/toaster is running (PID $ids). Per AGENTS.md, stop it before rebuilding to avoid DLL/link lock failures."
    }

    if ($prompt -and $prompt -match '(?i)launch toaster') {
        Warn "Recognized 'launch toaster' - monitored launcher is .\scripts\launch-toaster-monitored.ps1 -ObservationSeconds 120."
    }
} catch {
    [Console]::Error.WriteLine("session-start: soft-fail: $($_.Exception.Message)")
}
exit 0
