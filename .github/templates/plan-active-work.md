<!--
  plan-active-work.md
  
  Drop-in template for the plan.md > "Active work" block referenced by
  AGENTS.md session hygiene R3. Rewrite every checkpoint.
  Timezone convention: user-local (CT). No 2-hour gap on an active day.
-->

## Active work (YYYY-MM-DD HH:MM CT)

- Feature: `<slug>`  (STATE=`<defined|planned|executing|reviewing|shipped>`)
- In-flight task: `<task-id>` — `<one-line title>`
- Last commit: `<sha>`
- Next action: `<one line>`
- Blockers: `<none | list>`

### Retry log

<!-- One-line entries per AGENTS.md R4. Append when pivoting away from a
     failing > 2 min cargo command. Use `.github/templates/retry-log-entry.md`
     format. -->

### Live shells

<!-- Per R5: record any async shell still running at turn-end with
     shellId + PID + purpose. List gets reaped on session resume. -->
