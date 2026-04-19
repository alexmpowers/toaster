---
name: Rust conventions
description: Rust coding standards and build environment rules for the Tauri backend
applyTo: "src-tauri/**/*.rs"
---

# Rust conventions — see [`src-tauri/AGENTS.md`](../../src-tauri/AGENTS.md)

The authoritative Rust conventions, Windows build environment, cargo runtime
expectations, and known DLL pitfalls for this project live in
[`src-tauri/AGENTS.md`](../../src-tauri/AGENTS.md) — nearest-AGENTS.md wins
per the [agents.md spec](https://agents.md/).

This file exists so GitHub Copilot picks the rules up path-scoped when
editing `src-tauri/**/*.rs`. If a rule needs to change, edit
`src-tauri/AGENTS.md`, not this file.
