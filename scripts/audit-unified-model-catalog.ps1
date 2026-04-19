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

Write-Host "[STUB] unified-model-catalog audit ($Check): not yet implemented." -ForegroundColor Yellow
Write-Host "       Implement this check as part of the unified-model-catalog feature." -ForegroundColor Yellow
exit 2
