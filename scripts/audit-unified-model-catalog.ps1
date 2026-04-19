<#
.SYNOPSIS
    Audit stub for the unified-model-catalog feature.

.DESCRIPTION
    Verifier for AC-002-b (managers/llm/catalog.rs + managers/llm/download.rs
    deleted), AC-004-a (no `fn download` under managers/llm/), AC-005-a
    (single ModelDownloadProgress struct definition), and AC-008-a
    (local-models/LlmModelCatalog.tsx deleted or <= 40 lines).

    Planned until the unified-model-catalog feature is executed.
    Exits 2 so `scripts/check-feature-coverage.ps1` can validate the
    script path exists without pretending the audit is green.

.PARAMETER Check
    Which acceptance criterion slice to run. One of:
        deleted     - AC-002-b
        no-llm-dl   - AC-004-a
        single-ev   - AC-005-a
        shim-ui     - AC-008-a
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('deleted', 'no-llm-dl', 'single-ev', 'shim-ui')]
    [string]$Check
)

$repoRoot = Split-Path -Parent $PSScriptRoot
$srcRoot = Join-Path $repoRoot 'src-tauri\src'

switch ($Check) {
    'single-ev' {
        # AC-005-a: exactly one `struct ModelDownloadProgress` under src-tauri/src/
        # AND it must contain a `category` field.
        $files = Get-ChildItem -Path $srcRoot -Recurse -Filter *.rs -File
        $matches = $files | Select-String -Pattern 'struct\s+ModelDownloadProgress'
        $count = @($matches).Count
        if ($count -ne 1) {
            Write-Host "[FAIL] expected exactly 1 struct ModelDownloadProgress, found $count" -ForegroundColor Red
            $matches | ForEach-Object { Write-Host "   $_" }
            exit 1
        }
        $file = $matches[0].Path
        $content = Get-Content -Raw $file
        if ($content -notmatch '(?s)struct\s+ModelDownloadProgress\s*\{[^}]*\bcategory\s*:') {
            Write-Host "[FAIL] ModelDownloadProgress in $file is missing a 'category' field" -ForegroundColor Red
            exit 1
        }
        # Also ensure LlmDownloadProgress is gone (indicates collapse, not
        # coexistence).
        $llmStructs = $files | Select-String -Pattern 'struct\s+LlmDownloadProgress'
        if (@($llmStructs).Count -gt 0) {
            Write-Host "[FAIL] legacy LlmDownloadProgress struct still present:" -ForegroundColor Red
            $llmStructs | ForEach-Object { Write-Host "   $_" }
            exit 1
        }
        Write-Host "[PASS] single ModelDownloadProgress struct with category field ($file)" -ForegroundColor Green
        exit 0
    }
    default {
        Write-Host "[STUB] unified-model-catalog audit ($Check): not yet implemented." -ForegroundColor Yellow
        exit 2
    }
}
