# Post-tool-use reminders. Cannot deny; stderr-only notes.
$ErrorActionPreference = 'Stop'
try {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) { exit 0 }
    $payload = $raw | ConvertFrom-Json
    $toolName = [string]$payload.toolName
    $argsRaw = [string]$payload.toolArgs
    if (-not $argsRaw) { exit 0 }
    $toolArgs = $argsRaw | ConvertFrom-Json
    $path = [string]$toolArgs.path

    if ($toolName -eq 'edit' -and $path) {
        if ($path -match '\.rs$') {
            [Console]::Error.WriteLine("Reminder: cargo fmt -- $path before finishing.")
        }
        if ($path -match 'src[\\/]i18n[\\/]locales[\\/][^\\/]+[\\/]translation\.json$') {
            [Console]::Error.WriteLine("Reminder: run ``npm run check-translations`` after locale edits.")
        }
    }
} catch {
    [Console]::Error.WriteLine("posttool-reminders: soft-fail: $($_.Exception.Message)")
}
exit 0
