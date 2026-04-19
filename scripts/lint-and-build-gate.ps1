<#
.SYNOPSIS
    Stub lint + build gate for caption-settings-handlers (AC-004-b).

.DESCRIPTION
    Runs `npm run lint` and reports exit 0 on success.  This is a stub
    that exits 2 (not-implemented) until the implementing task
    (csh-static-gates) materialises the real gate.  Committed so the
    coverage gate can validate the verifier path on disk.
#>

[CmdletBinding()]
param()

Write-Host "[stub] scripts/lint-and-build-gate.ps1 not implemented yet." -ForegroundColor Yellow
Write-Host "       Intended to run: npm run lint && npm run build" -ForegroundColor Yellow
exit 2
