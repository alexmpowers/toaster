# Spec-driven development (Product Manager agent)

Toaster runs an afkode-inspired ([afkode.ai/docs](https://afkode.ai/docs)) spec-driven loop on top of the superpowers chain. Any work above a single-file fix should go through it.

## Lifecycle

```
Define -> Plan -> Execute -> Review -> Ship
 user    PM       superpowers:        superpowers:    finishing-a-
 (slug + agent    executing-plans /   code-reviewer + development-
 6-elt           subagent-driven-     code-review       branch
 REQUEST)        development           .instructions
```

State lives in `features/<slug>/STATE.md`, one of: `defined`, `planned`, `executing`, `reviewing`, `shipped`, `archived`. Run `pwsh scripts/feature/feature-board.ps1` for the terminal Kanban.

## Per-feature artifacts

Under `features/<slug>/`:

| File | Purpose | Tracked? |
|------|---------|----------|
| `STATE.md` | Lifecycle state (single line) | yes |
| `REQUEST.md` | Six-element user request (Problem & Goals / Outcome & AC / Scope / Code refs / Edge cases / Data model) | yes |
| `PRD.md` | Requirements with `R-NNN` IDs and `AC-NNN-x` acceptance criteria | yes |
| `BLUEPRINT.md` | Architecture decisions per R-ID, single-source-of-truth placement, risk register | yes |
| `tasks.sql` | `INSERT INTO todos / todo_deps` for the session SQL store | yes |
| `coverage.json` | Every AC -> verifier (skill / agent / cargo-test / script / manual live-app) | yes |
| `journal.md` | Operational journal (gitignored except for the example) | no |
| `tasks/<id>/context.md` | Curated per-task briefing for fresh subagents (gitignored except for the example) | no |

The `feature-pm` skill + `product-manager` agent generate this bundle; see [`features/example-pm-dryrun/`](../features/example-pm-dryrun/) for a worked reference.

## Coverage gate

`scripts/feature/check-feature-coverage.ps1 -Feature <slug>` (or `-All` in CI) verifies every `AC-NNN-x` in `PRD.md` has a real verifier in `coverage.json`. `scripts/feature/check-feature-tasks.ps1 -Feature <slug>` validates the `tasks.sql` schema (column list, status literals, forbidden columns). Both gates run inside `scripts/feature/promote-feature.ps1` and must exit 0 before `STATE.md` advances from `defined` to `planned`. This is the machine-enforced incarnation of the rule called out in the `transcript-precision-eval` skill ("must be machine-enforced, not agent-enforced").

## Curated context per task

Each `tasks/<id>/context.md` is the only file the dispatched subagent should load (plus the files it cites). This mirrors afkode's "fresh session per task" model so task 50 runs with the same precision as task 1, without dragging the full PRD into every context window.

## Toaster TDD scope

`superpowers:test-driven-development` requires a failing test before production code. Toaster's harness reality narrows this:

- **Backend (`src-tauri/`):** full TDD applies — write a failing `#[test]` (or extend an eval fixture under `src-tauri/tests/fixtures/`) first. Verify with `cargo test`.
- **Audio / timeline / export:** the real gate is the fixture-based eval harness (`transcript-precision-eval`, `audio-boundary-eval`). Extend fixtures first, run the relevant eval script, then implement.
- **Frontend-only UI / styling:** no unit-test framework exists. `npm run lint`, `npm run build`, and a live-app check per `superpowers:verification-before-completion` are the gates. Playwright E2E for user-visible flow changes.

## Project-wide testing knowledge

[`docs/testing-kb.md`](testing-kb.md) accumulates empirical testing facts across features (cargo timing, fixture regeneration, i18n parity, file-size cap, live-app verification). QC tasks should append discoveries here so feature N+1 does not re-hit feature N's walls.
