---
name: caveman-review
description: >
  Opt-in terse code review comments. Use when the user explicitly asks for terse review
  feedback, paste-ready review comments, or invokes /caveman-review.
---

Write code review comments terse and actionable. One line per finding. Location, problem, fix. No throat-clearing.

## Rules

**Format:** `L<line>: <problem>. <fix>.` - or `<file>:L<line>: ...` when reviewing multi-file diffs.

**Severity prefix (optional):**
- `bug:` - broken behavior, incident risk
- `risk:` - works but fragile (race, missing null check, swallowed error)
- `nit:` - style, naming, micro-optimization
- `q:` - genuine question, not a suggestion

**Drop:**
- "I noticed that..."
- "It seems like..."
- "You might want to consider..."
- "This is just a suggestion but..."
- repeated praise or throat-clearing
- restating what the code already does
- hedging unless using `q:`

**Keep:**
- Exact line numbers
- Exact symbol, function, or variable names in backticks
- Concrete fix, not vague advice
- The why when the fix is not obvious from the problem statement

## Examples

Bad: "I noticed that on line 42 you're not checking if the user object is null before accessing the email property."

Good: `L42: bug: user can be null after .find(). Add guard before .email.`

Bad: "It looks like this function is doing a lot of things and might benefit from being broken up."

Good: `L88-140: nit: 50-line fn does 4 things. Extract validate/normalize/persist.`

Bad: "Have you considered what happens if the API returns a 429?"

Good: `L23: risk: no retry on 429. Wrap in withBackoff(3).`

## Auto-Clarity

Drop terse mode for:
- security findings that need full explanation and references
- architectural disagreements that need rationale
- onboarding contexts where the author needs the why

In those cases, write a normal paragraph first, then resume terse mode for the rest.

## Boundaries

Reviews only. Do not write the code fix, approve or request changes, or run linters.

Output comments ready to paste into the review.
