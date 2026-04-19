<!--
  retry-log-entry.md

  One-line format for the plan.md > "Retry log" block (AGENTS.md R4).
  Makes failed cargo/tauri retry spirals visible and accountable.
-->

- `<YYYY-MM-DD HH:MM CT>` — `<command>` failed (`<error-summary>`). Pivoted to `<strategy: cargo clean | scope-swap | live-app | ask-user>`.

<!-- Example:
- 2026-04-18 21:50 CT — `cargo test --lib` hit STATUS_ENTRYPOINT_NOT_FOUND 0xc0000139 on 3 rebuilds. Pivoted from test harness to live-app path.
-->
