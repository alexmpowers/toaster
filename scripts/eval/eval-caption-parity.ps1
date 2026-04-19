<#
.SYNOPSIS
    Caption parity eval harness (STUB — planned by
    features/caption-parity-eval/).

.DESCRIPTION
    Planned harness that exercises a fixed set of caption-heavy
    fixtures through both the preview CaptionLayout path and the
    ASS-burn export path, asserting geometry parity within 1 px
    and timing parity within 1 sample @ 48 kHz, and emitting the
    standard pass/fail JSON shape:

        { pass: bool,
          fixtures: [ { id, pass, diffs: [...] } ],
          summary: { total, passed, failed } }

    This file is a planning stub committed so that the feature
    bundle's coverage gate (scripts/feature/check-feature-coverage.ps1)
    resolves the 'script' kind verifier for AC-001-a, AC-001-c, and
    AC-002-b without pretending the eval is green. It exits 2
    (not implemented) until the executing subagents implement it
    under features/caption-parity-eval/PRD.md + BLUEPRINT.md.

    DO NOT wire this stub into CI as a pass gate. See
    features/caption-parity-eval/BLUEPRINT.md § "CI integration plan".
#>

[CmdletBinding()]
param(
    [string]$Fixture,
    [switch]$ForceDrift,
    [string]$OutputRoot
)

Write-Host "[caption-parity-eval] STUB — not yet implemented." -ForegroundColor Yellow
Write-Host "  Plan: features/caption-parity-eval/PRD.md" -ForegroundColor DarkGray
Write-Host "  Blueprint: features/caption-parity-eval/BLUEPRINT.md" -ForegroundColor DarkGray
Write-Host "  Implement per AC-001-a / AC-001-b / AC-001-c / AC-002-a / AC-002-b / AC-003-a." -ForegroundColor DarkGray
exit 2
