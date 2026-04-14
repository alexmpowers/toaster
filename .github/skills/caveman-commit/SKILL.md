---
name: caveman-commit
description: >
  Opt-in terse commit message generator. Use when the user explicitly asks for a commit
  message, a terse conventional commit, or invokes /caveman-commit.
---

Write commit messages terse and exact. Conventional Commits format. No fluff. Why over what.

## Rules

**Subject line:**
- `<type>(<scope>): <imperative summary>` - `<scope>` optional
- Types: `feat`, `fix`, `refactor`, `perf`, `docs`, `test`, `chore`, `build`, `ci`, `style`, `revert`
- Imperative mood: "add", "fix", "remove" - not "added", "adds", "adding"
- <= 50 chars when possible, hard cap 72
- No trailing period
- Match project convention for capitalization after the colon

**Body (only if needed):**
- Skip entirely when subject is self-explanatory
- Add body only for: non-obvious *why*, breaking changes, migration notes, linked issues
- Wrap at 72 chars
- Bullets `-` not `*`
- Reference issues or PRs at end: `Closes #42`, `Refs #17`

**What NEVER goes in:**
- "This commit does X", "I", "we", "now", "currently" - the diff says what
- "As requested by..." - use Co-authored-by trailer if needed
- AI attribution
- Emoji unless project convention requires them
- Restating the file name when scope already says it

## Examples

Diff: new endpoint for user profile with body explaining the why
- Bad: "feat: add a new endpoint to get user profile information from the database"
- Good:
  ```
  feat(api): add GET /users/:id/profile

  Mobile client needs profile data without the full user payload
  to reduce LTE bandwidth on cold-launch screens.

  Closes #128
  ```

Diff: breaking API change
- Good:
  ```
  feat(api)!: rename /v1/orders to /v1/checkout

  BREAKING CHANGE: clients on /v1/orders must migrate to /v1/checkout
  before 2026-06-01. Old route returns 410 after that date.
  ```

## Auto-Clarity

Always include a body for:
- breaking changes
- security fixes
- data migrations
- reverts

Never compress those into subject-only output.

## Boundaries

Only generate the commit message. Do not run `git commit`, stage files, or amend commits.

Output the message as a code block ready to paste.
