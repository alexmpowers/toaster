-- Task graph for caption-parity-eval.
-- Ingest into the session SQL store with the `sql` tool.
--
-- Schema: todos(id TEXT, title TEXT, description TEXT, status TEXT).
-- Allowed status values: 'pending', 'in_progress', 'done', 'blocked'.
-- todo_deps schema: (todo_id TEXT, depends_on TEXT).

INSERT INTO todos (id, title, description, status) VALUES
  ('caption-parity-eval-fixtures',
   'Author three caption parity fixtures',
   'Create eval/caption-parity/fixtures/{single_line_01,multi_line_wrap_01,rapid_fire_01}/ each with input.json (Word[] + keep_segments + video_dims + caption_profile + timeline_domain) and expected.json (CaptionBlock[] geometry + timing snapshot). Author a brief eval/caption-parity/README.md describing snapshot regeneration. Verifier: AC-001-b (coverage.json, kind: manual).',
   'pending'),

  ('caption-parity-eval-dump-bin',
   'Add CaptionBlock JSON dump test binary',
   'Add src-tauri/tests/caption_parity_dump.rs (or equivalent) that reads a fixture input.json, runs managers::captions::build_blocks + compute_caption_layout, and writes the resulting CaptionBlock[] as JSON. Invoked from the harness via `cargo test --test caption_parity_dump -- --nocapture` or a small --dump-layout CLI. Stays read-only against managers/captions/. Verifier: feeds AC-001-a via the harness script.',
   'pending'),

  ('caption-parity-eval-harness',
   'Implement eval-caption-parity.ps1 harness body',
   'Replace the exit-2 stub in scripts/eval/eval-caption-parity.ps1 with the real harness: drive the dump binary for the preview side, run managers::captions::blocks_to_ass + FFmpeg subtitles filter for the export side (alpha-threshold bounds on a transparent canvas), diff both against expected.json within 1 px / 1 sample, and emit {pass, fixtures[{id,pass,diffs[]}], summary{total,passed,failed}} to eval/output/caption-parity/<ts>/report.json. Non-zero exit on any failure. Headless; FFmpeg-shell-out allowed. Factor helpers into scripts/lib/CaptionParity.psm1 if it crosses ~400 lines. Verifier: AC-001-a (coverage.json, kind: script).',
   'pending'),

  ('caption-parity-eval-tolerance',
   'Wire tolerance config + ForceDrift negative test',
   'Declare tolerance (time 1 sample @ 48 kHz, geometry 1 px) in a single config block. Add -ForceDrift <field>=<delta> flag that synthetically perturbs one side before comparison to prove gates fire. Ensure diff entries cite {field, expected, actual, tolerance}. Verifier: AC-001-c and AC-002-b (coverage.json, kind: script).',
   'pending'),

  ('caption-parity-eval-runner',
   'Register harness with eval-harness-runner agent',
   'Edit .github/agents/eval-harness-runner.agent.md to list scripts/eval/eval-caption-parity.ps1 alongside the other wrapped evals. No logic changes elsewhere; the harness JSON already matches the runner schema. Verifier: AC-002-a (coverage.json, kind: agent).',
   'pending'),

  ('caption-parity-eval-ci-plan',
   'Confirm CI integration plan section',
   'Verify features/caption-parity-eval/BLUEPRINT.md contains a "CI integration plan" markdown heading with target workflow, job/step placement, trigger events, runner setup (FFmpeg 7 on PATH), and success/failure expectation. Actual YAML edit is a follow-up slug. Verifier: AC-003-a (coverage.json, kind: doc-section).',
   'pending'),

  ('caption-parity-eval-harness-qc',
   'QC: AC-001-a harness emits standard JSON',
   'Run pwsh -NoProfile -File scripts/eval/eval-caption-parity.ps1 with all three fixtures present and implemented. Confirm exit 0 and that eval/output/caption-parity/<ts>/report.json matches the declared shape.',
   'pending'),

  ('caption-parity-eval-fixtures-qc',
   'QC: AC-001-b three fixtures present and valid',
   'Follow the manual steps in coverage.json AC-001-b: list fixtures dir, validate input.json + expected.json for each.',
   'pending'),

  ('caption-parity-eval-tolerance-qc',
   'QC: AC-001-c tolerances enforced',
   'Run with -ForceDrift injecting a 2 px / 2 sample perturbation; confirm the harness exits non-zero and diffs cite field name + tolerance.',
   'pending'),

  ('caption-parity-eval-runner-qc',
   'QC: AC-002-a eval-harness-runner wraps new harness',
   'Invoke eval-harness-runner; confirm it picks up scripts/eval/eval-caption-parity.ps1 and surfaces its JSON without per-script special casing.',
   'pending'),

  ('caption-parity-eval-regression-qc',
   'QC: AC-002-b deliberate regression produces readable diff',
   'Apply a 2 px padding bump in src-tauri/src/managers/captions/layout.rs on a scratch branch (or use -ForceDrift); confirm report.json diffs array cites the regressed field with expected/actual/tolerance.',
   'pending'),

  ('caption-parity-eval-ci-plan-qc',
   'QC: AC-003-a CI integration plan section exists',
   'Run pwsh scripts/feature/check-feature-coverage.ps1 -Feature caption-parity-eval; confirm doc-section check for "CI integration plan" in BLUEPRINT.md passes.',
   'pending'),

  ('caption-parity-eval-feature-qc',
   'Feature QC: run eval-harness-runner end-to-end',
   'Invoke the eval-harness-runner agent to run every wrapped eval, including the new caption parity harness, and confirm a green JSON report bundle. Final feature-level gate.',
   'pending');

INSERT INTO todo_deps (todo_id, depends_on) VALUES
  ('caption-parity-eval-dump-bin',          'caption-parity-eval-fixtures'),
  ('caption-parity-eval-harness',           'caption-parity-eval-dump-bin'),
  ('caption-parity-eval-harness',           'caption-parity-eval-fixtures'),
  ('caption-parity-eval-tolerance',         'caption-parity-eval-harness'),
  ('caption-parity-eval-runner',            'caption-parity-eval-harness'),
  ('caption-parity-eval-harness-qc',        'caption-parity-eval-harness'),
  ('caption-parity-eval-fixtures-qc',       'caption-parity-eval-fixtures'),
  ('caption-parity-eval-tolerance-qc',      'caption-parity-eval-tolerance'),
  ('caption-parity-eval-runner-qc',         'caption-parity-eval-runner'),
  ('caption-parity-eval-regression-qc',     'caption-parity-eval-tolerance'),
  ('caption-parity-eval-ci-plan-qc',        'caption-parity-eval-ci-plan'),
  ('caption-parity-eval-feature-qc',        'caption-parity-eval-harness-qc'),
  ('caption-parity-eval-feature-qc',        'caption-parity-eval-fixtures-qc'),
  ('caption-parity-eval-feature-qc',        'caption-parity-eval-tolerance-qc'),
  ('caption-parity-eval-feature-qc',        'caption-parity-eval-runner-qc'),
  ('caption-parity-eval-feature-qc',        'caption-parity-eval-regression-qc'),
  ('caption-parity-eval-feature-qc',        'caption-parity-eval-ci-plan-qc');
