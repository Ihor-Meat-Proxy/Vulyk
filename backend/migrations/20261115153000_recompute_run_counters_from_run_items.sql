-- Recompute denormalized test_runs.*_count from run_items (UI progress SoT).
-- Backfill *missing* run_items from latest test_results per case (historical CI
-- that only wrote test_results). Do not overwrite existing run_item statuses.

-- 1) Insert missing run_items only (latest result status per case).
INSERT INTO run_items (run_id, case_id, status, sort_order, comment, executed_at)
SELECT
    lr.run_id,
    lr.case_id,
    lr.status,
    COALESCE(
      (SELECT MAX(ri.sort_order) FROM run_items ri WHERE ri.run_id = lr.run_id),
      -1
    ) + ROW_NUMBER() OVER (PARTITION BY lr.run_id ORDER BY tc.key),
    COALESCE(lr.message, ''),
    lr.executed_at
FROM (
    SELECT DISTINCT ON (tr.run_id, tr.case_id)
           tr.run_id, tr.case_id, tr.status, tr.message, tr.executed_at
    FROM test_results tr
    WHERE tr.case_id IS NOT NULL
    ORDER BY tr.run_id, tr.case_id, tr.executed_at DESC NULLS LAST, tr.created_at DESC
) lr
INNER JOIN test_cases tc ON tc.id = lr.case_id
WHERE NOT EXISTS (
    SELECT 1 FROM run_items ri
    WHERE ri.run_id = lr.run_id AND ri.case_id = lr.case_id
);

-- 2) Set pass/fail/skip/blocked from run_items; keep duration from test_results.
UPDATE test_runs r SET
  pass_count = COALESCE((
    SELECT COUNT(*) FILTER (WHERE status = 'Passed') FROM run_items WHERE run_id = r.id
  ), 0),
  fail_count = COALESCE((
    SELECT COUNT(*) FILTER (WHERE status = 'Failed') FROM run_items WHERE run_id = r.id
  ), 0),
  skip_count = COALESCE((
    SELECT COUNT(*) FILTER (WHERE status = 'Skipped') FROM run_items WHERE run_id = r.id
  ), 0),
  blocked_count = COALESCE((
    SELECT COUNT(*) FILTER (WHERE status IN ('Blocked', 'Untestable'))
    FROM run_items WHERE run_id = r.id
  ), 0),
  duration_ms = COALESCE((
    SELECT SUM(COALESCE(duration_ms, 0))::bigint FROM test_results WHERE run_id = r.id
  ), 0),
  updated_at = NOW()
WHERE EXISTS (SELECT 1 FROM run_items ri WHERE ri.run_id = r.id)
   OR EXISTS (SELECT 1 FROM test_results tr WHERE tr.run_id = r.id);
