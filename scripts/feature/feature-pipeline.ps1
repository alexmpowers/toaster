<#
.SYNOPSIS
    Feature pipeline runner — sole state-machine driver for Toaster feature
    lifecycle.  Modeled on task-flows' run-pipeline.py.

.DESCRIPTION
    Manages pipeline-state.json for each feature under features/<slug>/.
    This script is the SOLE WRITER of pipeline-state.json.  No agent or
    skill should edit that file directly (enforced by a pretool-policy
    hook).

    Lifecycle:  scaffold → analyze → design → sign-off → engineer → qa → report

    Phase→skill routing is registry-driven from
    .github/_shared/registry/pipeline-registry.json.

.PARAMETER Command
    Subcommand: start, status, next, advance, reset, reconcile, board.

.PARAMETER Slug
    Feature slug (kebab-case).

.PARAMETER Problem
    One-line problem statement (required for 'start').

.PARAMETER Worktree
    Pass-through to scaffold-feature.ps1 (start only).

.PARAMETER Approve
    Approve a human gate (advance at sign-off).

.PARAMETER Revise
    Request revision at sign-off (loops back to analyze).

.PARAMETER Feedback
    Revision feedback string (used with -Revise).

.PARAMETER DeployMode
    Reserved for future use (live | artifacts_only).

.PARAMETER Phase
    Target phase for 'reset' subcommand.

.PARAMETER Quiet
    Suppress non-signal output (for auto-chain parsing).

.EXAMPLE
    pwsh scripts/feature/feature-pipeline.ps1 start -Slug my-feature -Problem "Add export presets"
    pwsh scripts/feature/feature-pipeline.ps1 status -Slug my-feature
    pwsh scripts/feature/feature-pipeline.ps1 next -Slug my-feature
    pwsh scripts/feature/feature-pipeline.ps1 advance -Slug my-feature
    pwsh scripts/feature/feature-pipeline.ps1 advance -Slug my-feature -Approve
    pwsh scripts/feature/feature-pipeline.ps1 advance -Slug my-feature -Revise -Feedback "Needs fewer ACs"
    pwsh scripts/feature/feature-pipeline.ps1 reset -Slug my-feature -Phase design
    pwsh scripts/feature/feature-pipeline.ps1 reconcile -Slug my-feature
    pwsh scripts/feature/feature-pipeline.ps1 board
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('start', 'status', 'next', 'advance', 'reset', 'reconcile', 'board')]
    [string]$Command,

    [string]$Slug,
    [string]$Problem,
    [switch]$Worktree,
    [switch]$Approve,
    [switch]$Revise,
    [string]$Feedback,
    [string]$Phase,
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'

# ═══════════════════════════════════════════════════════════════════════════
# Paths
# ═══════════════════════════════════════════════════════════════════════════

$repoRoot     = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$featuresDir  = Join-Path $repoRoot 'features'
$registryPath = Join-Path $repoRoot '.github' '_shared' 'registry' 'pipeline-registry.json'
$scriptDir    = $PSScriptRoot

# ═══════════════════════════════════════════════════════════════════════════
# Registry
# ═══════════════════════════════════════════════════════════════════════════

function Load-Registry {
    if (-not (Test-Path $registryPath)) {
        Write-Error "Pipeline registry not found: $registryPath"
        exit 2
    }
    return Get-Content -Raw $registryPath | ConvertFrom-Json
}

# ═══════════════════════════════════════════════════════════════════════════
# State helpers
# ═══════════════════════════════════════════════════════════════════════════

function Get-FeatureDir([string]$slug) {
    return Join-Path $featuresDir $slug
}

function Get-StatePath([string]$slug) {
    return Join-Path (Get-FeatureDir $slug) 'pipeline-state.json'
}

function Read-State([string]$slug) {
    $path = Get-StatePath $slug
    if (-not (Test-Path $path)) {
        Write-Error "pipeline-state.json not found for '$slug'. Run 'start' first or 'reconcile' to bootstrap."
        exit 1
    }
    return Get-Content -Raw $path | ConvertFrom-Json
}

function Write-State([string]$slug, [object]$state) {
    $path = Get-StatePath $slug
    $state | ConvertTo-Json -Depth 10 | Set-Content -Path $path -NoNewline
    # Also write derived STATE.md for backward compat with feature-board.ps1
    $reg = Load-Registry
    $phaseObj = $reg.phases | Select-Object -ExpandProperty $state.current_phase -ErrorAction SilentlyContinue
    $stateMd = if ($phaseObj -and $phaseObj.state_md) { $phaseObj.state_md } else { 'defined' }
    Set-Content -Path (Join-Path (Get-FeatureDir $slug) 'STATE.md') -Value $stateMd -NoNewline
}

function New-PipelineState([string]$slug, [string]$problem) {
    $reg = Load-Registry
    $phases = [ordered]@{}
    $first = $true
    foreach ($p in $reg.phase_order) {
        $phases[$p] = [ordered]@{
            status = if ($first) { 'in_progress' } else { 'pending' }
        }
        $first = $false
    }
    return [ordered]@{
        slug                = $slug
        problem             = $problem
        current_phase       = $reg.phase_order[0]
        sign_off_revisions  = 0
        fix_loop_iterations = 0
        phases              = $phases
    }
}

function Get-PhaseIndex([string]$phase) {
    $reg = Load-Registry
    $idx = [array]::IndexOf($reg.phase_order, $phase)
    if ($idx -lt 0) { Write-Error "Unknown phase: $phase"; exit 2 }
    return $idx
}

function Get-NextPhase([string]$current) {
    $reg = Load-Registry
    $idx = Get-PhaseIndex $current
    if ($idx -ge ($reg.phase_order.Count - 1)) { return $null }
    return $reg.phase_order[$idx + 1]
}

function Get-Transition([string]$from, [string]$to) {
    $reg = Load-Registry
    foreach ($t in $reg.transitions) {
        if ($t.from -eq $from -and $t.to -eq $to) { return $t }
    }
    return $null
}

function Get-AutoTransition([string]$from) {
    $reg = Load-Registry
    foreach ($t in $reg.transitions) {
        if ($t.from -eq $from -and $t.auto -eq $true) { return $t }
    }
    return $null
}

# ═══════════════════════════════════════════════════════════════════════════
# Output verification
# ═══════════════════════════════════════════════════════════════════════════

function Test-PhaseOutput([string]$slug, [string]$phase) {
    $reg = Load-Registry
    $phaseObj = $reg.phases.$phase
    if (-not $phaseObj -or -not $phaseObj.output) { return $true }

    $featureDir = Get-FeatureDir $slug
    $missing = @()
    foreach ($file in $phaseObj.output) {
        $path = Join-Path $featureDir $file
        if (-not (Test-Path $path)) {
            $missing += $file
            continue
        }
        # Check it's not still a template (> 200 bytes and no TBD markers)
        $size = (Get-Item $path).Length
        if ($size -lt 200) {
            $content = Get-Content -Raw $path
            if ($content -match '\{\{SLUG\}\}' -or $content -match '\{\{TITLE\}\}') {
                $missing += "$file (still a template)"
            }
        }
    }
    if ($missing.Count -gt 0) {
        Write-Warning "Phase '$phase' output verification failed. Missing/incomplete: $($missing -join ', ')"
        return $false
    }
    return $true
}

# ═══════════════════════════════════════════════════════════════════════════
# Signal helpers
# ═══════════════════════════════════════════════════════════════════════════

function Emit-AutoChain([string]$skill) {
    Write-Host "🟢 AUTO-CHAIN → $skill" -ForegroundColor Green
}

function Emit-HumanGate([string]$phase) {
    Write-Host "🛑 HUMAN GATE — $phase" -ForegroundColor Yellow
    Write-Host "   Run: pwsh scripts/feature/feature-pipeline.ps1 advance -Slug $Slug -Approve" -ForegroundColor DarkGray
    Write-Host "   Or:  pwsh scripts/feature/feature-pipeline.ps1 advance -Slug $Slug -Revise -Feedback `"...`"" -ForegroundColor DarkGray
}

function Emit-Done([string]$slug) {
    Write-Host "✅ PIPELINE COMPLETE — $slug" -ForegroundColor Green
}

# ═══════════════════════════════════════════════════════════════════════════
# Subcommands
# ═══════════════════════════════════════════════════════════════════════════

# ── START ─────────────────────────────────────────────────────────────────
function Invoke-Start {
    if (-not $Slug) { Write-Error "start requires -Slug"; exit 2 }
    if (-not $Problem) { Write-Error "start requires -Problem"; exit 2 }

    $featureDir = Get-FeatureDir $Slug

    # Run scaffold-feature.ps1 (fast-forward: deterministic, no LLM)
    $scaffoldScript = Join-Path $scriptDir 'scaffold-feature.ps1'
    $scaffoldArgs = @('-Slug', $Slug)
    if ($Worktree) { $scaffoldArgs += '-Worktree' }
    if (Test-Path $featureDir) { $scaffoldArgs += '-Force' }

    & pwsh $scaffoldScript @scaffoldArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error "scaffold-feature.ps1 failed (exit $LASTEXITCODE)"
        exit 1
    }

    # Create pipeline-state.json
    $state = New-PipelineState -slug $Slug -problem $Problem
    # Scaffold is fast-forward — mark it complete immediately, advance to analyze
    $state.phases.scaffold.status = 'complete'
    $state.current_phase = 'analyze'
    $state.phases.analyze.status = 'in_progress'

    Write-State -slug $Slug -state $state

    if (-not $Quiet) {
        Write-Host ""
        Write-Host "Pipeline initialized for '$Slug'" -ForegroundColor Green
        Write-Host "  Problem:       $Problem"
        Write-Host "  Current phase: analyze"
        Write-Host ""
    }

    # Emit auto-chain signal
    $reg = Load-Registry
    $analyzePhase = $reg.phases.analyze
    if ($analyzePhase.skill) {
        Emit-AutoChain $analyzePhase.skill
    }
}

# ── STATUS ────────────────────────────────────────────────────────────────
function Invoke-Status {
    if (-not $Slug) { Write-Error "status requires -Slug"; exit 2 }

    $state = Read-State $Slug
    $reg = Load-Registry

    Write-Host ""
    Write-Host "Pipeline status: $Slug" -ForegroundColor White
    Write-Host ("=" * 50) -ForegroundColor DarkGray
    Write-Host "  Problem:    $($state.problem)"
    Write-Host "  Phase:      $($state.current_phase)"
    Write-Host "  Revisions:  $($state.sign_off_revisions)"
    Write-Host "  Fix loops:  $($state.fix_loop_iterations)"
    Write-Host ""

    foreach ($p in $reg.phase_order) {
        $status = $state.phases.$p.status
        $icon = switch ($status) {
            'complete'    { '✅' }
            'in_progress' { '🔄' }
            'pending'     { '⬜' }
            default       { '❓' }
        }
        $color = switch ($status) {
            'complete'    { 'Green' }
            'in_progress' { 'Yellow' }
            'pending'     { 'DarkGray' }
            default       { 'Gray' }
        }
        $phaseInfo = $reg.phases.$p
        $skill = if ($phaseInfo.skill) { " ($($phaseInfo.skill))" }
                 elseif ($phaseInfo.gate) { " [HUMAN GATE]" }
                 elseif ($phaseInfo.fast_forward) { " [fast-forward]" }
                 elseif ($phaseInfo.dispatch) { " (per-task)" }
                 else { '' }
        Write-Host "  $icon $p$skill" -ForegroundColor $color
    }
    Write-Host ""
}

# ── NEXT ──────────────────────────────────────────────────────────────────
function Invoke-Next {
    if (-not $Slug) { Write-Error "next requires -Slug"; exit 2 }

    $state = Read-State $Slug
    $reg = Load-Registry
    $current = $state.current_phase
    $phaseInfo = $reg.phases.$current

    if ($phaseInfo.skill) {
        Write-Host ""
        Write-Host "Next: invoke skill '$($phaseInfo.skill)' for phase '$current'" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Context:" -ForegroundColor White
        Write-Host "  Feature:  $Slug"
        Write-Host "  Problem:  $($state.problem)"
        Write-Host "  Phase:    $current"
        if ($state.sign_off_revisions -gt 0) {
            Write-Host "  Revision: #$($state.sign_off_revisions)" -ForegroundColor Yellow
            $feedbackFile = Join-Path (Get-FeatureDir $Slug) 'sign-off-feedback.md'
            if (Test-Path $feedbackFile) {
                Write-Host "  Feedback: see features/$Slug/sign-off-feedback.md" -ForegroundColor Yellow
            }
        }
        Write-Host ""
        Write-Host "After completing this phase, run:" -ForegroundColor DarkGray
        Write-Host "  pwsh scripts/feature/feature-pipeline.ps1 advance -Slug $Slug" -ForegroundColor DarkGray
    }
    elseif ($phaseInfo.gate -eq 'human') {
        Write-Host ""
        Write-Host "Next: HUMAN GATE — sign-off required" -ForegroundColor Yellow
        Write-Host "  Approve: pwsh scripts/feature/feature-pipeline.ps1 advance -Slug $Slug -Approve"
        Write-Host "  Revise:  pwsh scripts/feature/feature-pipeline.ps1 advance -Slug $Slug -Revise -Feedback `"...`""
    }
    elseif ($phaseInfo.fast_forward) {
        Write-Host ""
        Write-Host "Next: fast-forward phase '$current' (deterministic)" -ForegroundColor Cyan
        Write-Host "  Run: pwsh scripts/feature/feature-pipeline.ps1 advance -Slug $Slug"
    }
    elseif ($phaseInfo.dispatch -eq 'per-task') {
        Write-Host ""
        Write-Host "Next: dispatch per-task engineering for '$Slug'" -ForegroundColor Cyan
        Write-Host "  Generate task briefings: bun scripts/feature/generate-task-context.ts $Slug"
        Write-Host "  Then advance: pwsh scripts/feature/feature-pipeline.ps1 advance -Slug $Slug"
    }
    else {
        Write-Host "Phase '$current' has no routing info in the registry." -ForegroundColor Red
    }
}

# ── ADVANCE ───────────────────────────────────────────────────────────────
function Invoke-Advance {
    if (-not $Slug) { Write-Error "advance requires -Slug"; exit 2 }

    $state = Read-State $Slug
    $reg = Load-Registry
    $current = $state.current_phase
    $phaseInfo = $reg.phases.$current

    # ── Revision path (sign-off only) ────────────────────────────────────
    if ($Revise) {
        if ($current -ne 'sign-off') {
            Write-Error "-Revise is only valid at the sign-off phase (current: $current)"
            exit 1
        }
        if ($state.sign_off_revisions -ge 3) {
            Write-Error "Maximum revision cycles (3) reached. Cannot revise further."
            exit 1
        }

        # Save feedback
        if ($Feedback) {
            $feedbackFile = Join-Path (Get-FeatureDir $Slug) 'sign-off-feedback.md'
            $timestamp = Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ'
            $entry = "## Revision #$($state.sign_off_revisions + 1) — $timestamp`n`n$Feedback`n`n"
            if (Test-Path $feedbackFile) {
                $existing = Get-Content -Raw $feedbackFile
                Set-Content -Path $feedbackFile -Value "$existing$entry" -NoNewline
            } else {
                Set-Content -Path $feedbackFile -Value "# Sign-off feedback`n`n$entry" -NoNewline
            }
        }

        # Reset analyze, design, sign-off back to pending
        $state.sign_off_revisions++
        foreach ($p in @('analyze', 'design', 'sign-off')) {
            $state.phases.$p.status = 'pending'
        }
        $state.current_phase = 'analyze'
        $state.phases.analyze.status = 'in_progress'

        Write-State -slug $Slug -state $state

        if (-not $Quiet) {
            Write-Host "Revision #$($state.sign_off_revisions) — looping back to analyze" -ForegroundColor Yellow
        }

        $analyzeSkill = $reg.phases.analyze.skill
        if ($analyzeSkill) { Emit-AutoChain $analyzeSkill }
        return
    }

    # ── Human gate check ─────────────────────────────────────────────────
    if ($phaseInfo.gate -eq 'human' -and -not $Approve) {
        # Run precompute (promote-feature.ps1) for sign-off readiness check
        if ($phaseInfo.precompute -and $current -eq 'sign-off') {
            if (-not $Quiet) {
                Write-Host "Running sign-off precompute (promotion gates)..." -ForegroundColor Cyan
            }
            $preScript = Join-Path $repoRoot $phaseInfo.precompute
            & pwsh $preScript -Slug $Slug
            if ($LASTEXITCODE -ne 0) {
                Write-Error "Promotion gates failed. Fix coverage/tasks issues and retry."
                exit 1
            }
        }
        Emit-HumanGate $current
        return
    }

    # ── Fix-loop check (qa → engineer) ───────────────────────────────────
    if ($current -eq 'qa') {
        # Check if QA passed — if not, loop back to engineer
        # The caller should set exit code or we check QA output
        # For now, advance means QA passed. Fix-loop is handled by
        # mission-control reading the QA gate result.
    }

    # ── Output verification ──────────────────────────────────────────────
    if (-not (Test-PhaseOutput -slug $Slug -phase $current)) {
        Write-Error "Phase '$current' output verification failed. Complete the phase before advancing."
        exit 1
    }

    # ── Advance state ────────────────────────────────────────────────────
    $state.phases.$current.status = 'complete'

    $next = Get-NextPhase $current
    if (-not $next) {
        # Pipeline complete
        $state.current_phase = $current  # stays on last phase
        Write-State -slug $Slug -state $state
        Emit-Done $Slug
        return
    }

    $state.current_phase = $next
    $state.phases.$next.status = 'in_progress'

    # ── Fast-forward check ───────────────────────────────────────────────
    $nextInfo = $reg.phases.$next
    if ($nextInfo.fast_forward) {
        # Run precompute if present
        if ($nextInfo.precompute) {
            $preScript = Join-Path $repoRoot $nextInfo.precompute
            & pwsh $preScript -Slug $Slug
        }
        # Mark fast-forward phase complete and continue advancing
        $state.phases.$next.status = 'complete'
        $nextNext = Get-NextPhase $next
        if ($nextNext) {
            $state.current_phase = $nextNext
            $state.phases.$nextNext.status = 'in_progress'
            Write-State -slug $Slug -state $state

            $nextNextInfo = $reg.phases.$nextNext
            if ($nextNextInfo.skill) {
                Emit-AutoChain $nextNextInfo.skill
            } elseif ($nextNextInfo.gate -eq 'human') {
                Emit-HumanGate $nextNext
            }
        } else {
            Write-State -slug $Slug -state $state
            Emit-Done $Slug
        }
        return
    }

    Write-State -slug $Slug -state $state

    # ── Emit signal for next phase ───────────────────────────────────────
    $transition = Get-AutoTransition $current
    if ($transition) {
        if ($nextInfo.skill) {
            Emit-AutoChain $nextInfo.skill
        } elseif ($nextInfo.gate -eq 'human') {
            Emit-HumanGate $next
        } elseif ($nextInfo.dispatch -eq 'per-task') {
            if (-not $Quiet) {
                Write-Host "🟢 AUTO-CHAIN → per-task dispatch (engineer)" -ForegroundColor Green
            }
        }
    } else {
        Emit-HumanGate $next
    }
}

# ── RESET ─────────────────────────────────────────────────────────────────
function Invoke-Reset {
    if (-not $Slug) { Write-Error "reset requires -Slug"; exit 2 }
    if (-not $Phase) { Write-Error "reset requires -Phase"; exit 2 }

    $state = Read-State $Slug
    $reg = Load-Registry
    $targetIdx = Get-PhaseIndex $Phase

    # Reset target and all downstream phases to pending
    for ($i = $targetIdx; $i -lt $reg.phase_order.Count; $i++) {
        $p = $reg.phase_order[$i]
        $state.phases.$p.status = 'pending'
    }

    $state.current_phase = $Phase
    $state.phases.$Phase.status = 'in_progress'

    Write-State -slug $Slug -state $state

    if (-not $Quiet) {
        Write-Host "Reset '$Slug' to phase '$Phase'. All downstream phases set to pending." -ForegroundColor Yellow
    }
}

# ── RECONCILE ─────────────────────────────────────────────────────────────
function Invoke-Reconcile {
    if (-not $Slug) { Write-Error "reconcile requires -Slug"; exit 2 }

    $featureDir = Get-FeatureDir $Slug
    if (-not (Test-Path $featureDir)) {
        Write-Error "Feature directory not found: $featureDir"
        exit 2
    }

    $reg = Load-Registry
    $statePath = Get-StatePath $Slug

    # Bootstrap problem from existing REQUEST.md if possible
    $problem = ''
    $requestFile = Join-Path $featureDir 'REQUEST.md'
    if (Test-Path $requestFile) {
        $lines = Get-Content $requestFile -TotalCount 5
        foreach ($line in $lines) {
            if ($line -match '^#\s+(.+)') { $problem = $Matches[1]; break }
        }
    }
    if (-not $problem) { $problem = "(reconciled from files)" }

    # Determine phase completion from file evidence
    $state = New-PipelineState -slug $Slug -problem $problem

    # scaffold: always complete if directory exists
    $state.phases.scaffold.status = 'complete'

    # analyze: complete if journal.md has content (Analysis section)
    $journalFile = Join-Path $featureDir 'journal.md'
    if ((Test-Path $journalFile) -and ((Get-Item $journalFile).Length -gt 50)) {
        $journalContent = Get-Content -Raw $journalFile
        if ($journalContent -match '##\s+Analysis') {
            $state.phases.analyze.status = 'complete'
        }
    }

    # design: complete if PRD.md + BLUEPRINT.md + coverage.json exist and aren't templates
    $designFiles = @('PRD.md', 'BLUEPRINT.md', 'coverage.json')
    $designComplete = $true
    foreach ($f in $designFiles) {
        $path = Join-Path $featureDir $f
        if (-not (Test-Path $path) -or (Get-Item $path).Length -lt 200) {
            $designComplete = $false
            break
        }
        $content = Get-Content -Raw $path
        if ($content -match '\{\{SLUG\}\}' -or $content -match '\{\{TITLE\}\}') {
            $designComplete = $false
            break
        }
    }
    if ($designComplete) { $state.phases.design.status = 'complete' }

    # sign-off: complete if STATE.md says 'planned' or later
    $stateFile = Join-Path $featureDir 'STATE.md'
    $currentStateMd = if (Test-Path $stateFile) { (Get-Content -Raw $stateFile).Trim() } else { 'defined' }
    $postPlanned = @('planned', 'executing', 'reviewing', 'shipped')
    if ($postPlanned -contains $currentStateMd) {
        $state.phases.'sign-off'.status = 'complete'
    }

    # engineer: complete if STATE.md is executing or later
    $postExecuting = @('executing', 'reviewing', 'shipped')
    if ($postExecuting -contains $currentStateMd) {
        $state.phases.engineer.status = 'complete'
    }

    # qa: complete if STATE.md is reviewing or shipped
    $postReviewing = @('reviewing', 'shipped')
    if ($postReviewing -contains $currentStateMd) {
        $state.phases.qa.status = 'complete'
    }

    # report: complete if shipped
    if ($currentStateMd -eq 'shipped') {
        $state.phases.report.status = 'complete'
    }

    # Find current phase (first non-complete)
    $state.current_phase = $reg.phase_order[-1]  # default to last
    foreach ($p in $reg.phase_order) {
        if ($state.phases.$p.status -ne 'complete') {
            $state.current_phase = $p
            $state.phases.$p.status = 'in_progress'
            break
        }
    }

    # Read existing sign_off_revisions and fix_loop_iterations if state exists
    if (Test-Path $statePath) {
        try {
            $existing = Get-Content -Raw $statePath | ConvertFrom-Json
            $state.sign_off_revisions = [int]$existing.sign_off_revisions
            $state.fix_loop_iterations = [int]$existing.fix_loop_iterations
        } catch {}
    }

    Write-State -slug $Slug -state $state

    Write-Host ""
    Write-Host "Reconciled pipeline state for '$Slug'" -ForegroundColor Green
    Write-Host "  Current phase: $($state.current_phase)"
    Write-Host "  Phases:" -ForegroundColor White
    foreach ($p in $reg.phase_order) {
        $s = $state.phases.$p.status
        $icon = switch ($s) { 'complete' { '✅' }; 'in_progress' { '🔄' }; default { '⬜' } }
        Write-Host "    $icon $p = $s"
    }
    Write-Host ""
}

# ── BOARD ─────────────────────────────────────────────────────────────────
function Invoke-Board {
    $boardScript = Join-Path $scriptDir 'feature-board.ps1'
    & pwsh $boardScript
}

# ═══════════════════════════════════════════════════════════════════════════
# Dispatch
# ═══════════════════════════════════════════════════════════════════════════

switch ($Command) {
    'start'     { Invoke-Start }
    'status'    { Invoke-Status }
    'next'      { Invoke-Next }
    'advance'   { Invoke-Advance }
    'reset'     { Invoke-Reset }
    'reconcile' { Invoke-Reconcile }
    'board'     { Invoke-Board }
}
