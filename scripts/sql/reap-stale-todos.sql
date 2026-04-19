-- reap-stale-todos.sql
--
-- Run at every checkpoint before writing a new one. Keeps the active
-- `todos` count under the ~15-row target from AGENTS.md session hygiene R2.
--
-- Usage from a session-SQL turn:
--   1. Replace <current-slug> with the active feature slug.
--   2. Run each statement; review row counts afterwards with:
--        SELECT status, COUNT(*) FROM todos GROUP BY status;
--
-- Safety:
--   - Does NOT delete `in_progress` rows.
--   - Does NOT delete `blocked` rows that don't mention "descoped".
--   - Preserves any done/blocked row tied to the CURRENT feature so the
--     Active-work template can still reference it.

-- 1) Reap done todos from prior features.
DELETE FROM todos
WHERE status = 'done'
  AND id NOT LIKE '<current-slug>-%';

-- 2) Reap explicitly-descoped blocked todos (not legitimately blocked ones).
DELETE FROM todos
WHERE status = 'blocked'
  AND description LIKE '%descoped%';

-- 3) Post-reap sanity check (this is a SELECT; no deletion effect).
SELECT status, COUNT(*) AS n
FROM todos
GROUP BY status
ORDER BY status;
