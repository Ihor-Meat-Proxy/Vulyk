-- Denormalize latest test_results status onto test_cases for case-list sort/filter
-- at ~20k cases (code review #11). Correlated subqueries per row are replaced by
-- tc.last_run_status; writers refresh these columns after inserting results.

ALTER TABLE test_cases
    ADD COLUMN IF NOT EXISTS last_run_status TEXT,
    ADD COLUMN IF NOT EXISTS last_run_at TIMESTAMPTZ;

COMMENT ON COLUMN test_cases.last_run_status IS
    'Latest test_results.status for this case (manual + ingest), by executed_at';
COMMENT ON COLUMN test_cases.last_run_at IS
    'executed_at of the row that set last_run_status';

-- Backfill from existing results (DISTINCT ON keeps latest per case).
UPDATE test_cases AS tc
SET
    last_run_status = sub.status,
    last_run_at = sub.executed_at
FROM (
    SELECT DISTINCT ON (case_id)
        case_id,
        status,
        executed_at
    FROM test_results
    WHERE case_id IS NOT NULL
    ORDER BY case_id, executed_at DESC NULLS LAST, created_at DESC
) AS sub
WHERE tc.id = sub.case_id
  AND (
    tc.last_run_status IS DISTINCT FROM sub.status
    OR tc.last_run_at IS DISTINCT FROM sub.executed_at
  );

CREATE INDEX IF NOT EXISTS test_cases_project_last_run_status_idx
    ON test_cases (project_id, last_run_status);
