# userPromptSubmitted audit log. Output is IGNORED by the CLI - observational only.
$ErrorActionPreference = 'Stop'
try {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) { exit 0 }
    $payload = $raw | ConvertFrom-Json
    $prompt = [string]$payload.prompt
    if (-not $prompt) { exit 0 }

    $logDir = Join-Path $env:USERPROFILE '.copilot'
    if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir -Force | Out-Null }
    $logPath = Join-Path $logDir 'toaster-prompts.log'

    $ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $flat = ($prompt -replace "`r?`n", ' ')
    Add-Content -Path $logPath -Value "$ts $flat" -Encoding UTF8

    $triggers = @('launch toaster','stop toaster','run evals')
    foreach ($t in $triggers) {
        if ($prompt -match [regex]::Escape($t)) {
            Add-Content -Path $logPath -Value "# trigger-phrase: $t" -Encoding UTF8
        }
    }
} catch {
    [Console]::Error.WriteLine("log-prompt: soft-fail: $($_.Exception.Message)")
}
exit 0
