-- Canonical PascalCase result statuses + immutable history columns/indexes.
-- Status vocabulary: Untested, Passed, Failed, Blocked, Skipped, Retest.
-- JUnit "error" maps to Failed (no separate Error status).

-- ── run_items ──────────────────────────────────────────────────────────────
ALTER TABLE run_items DROP CONSTRAINT IF EXISTS run_items_status_check;

UPDATE run_items SET status = CASE lower(status)
    WHEN 'untested' THEN 'Untested'
    WHEN 'passed' THEN 'Passed'
    WHEN 'failed' THEN 'Failed'
    WHEN 'blocked' THEN 'Blocked'
    WHEN 'skipped' THEN 'Skipped'
    WHEN 'retest' THEN 'Retest'
    WHEN 'error' THEN 'Failed'
    ELSE initcap(status)
END;

ALTER TABLE run_items ALTER COLUMN status SET DEFAULT 'Untested';

ALTER TABLE run_items
    ADD CONSTRAINT run_items_status_check CHECK (
        status IN ('Untested', 'Passed', 'Failed', 'Blocked', 'Skipped', 'Retest')
    );

-- ── test_results ───────────────────────────────────────────────────────────
ALTER TABLE test_results DROP CONSTRAINT IF EXISTS test_results_status_check;

UPDATE test_results SET status = CASE lower(status)
    WHEN 'untested' THEN 'Untested'
    WHEN 'passed' THEN 'Passed'
    WHEN 'failed' THEN 'Failed'
    WHEN 'blocked' THEN 'Blocked'
    WHEN 'skipped' THEN 'Skipped'
    WHEN 'retest' THEN 'Retest'
    WHEN 'error' THEN 'Failed'
    ELSE initcap(status)
END;

ALTER TABLE test_results
    ADD CONSTRAINT test_results_status_check CHECK (
        status IN ('Untested', 'Passed', 'Failed', 'Blocked', 'Skipped', 'Retest')
    );

ALTER TABLE test_results
    ADD COLUMN IF NOT EXISTS executed_by UUID REFERENCES users (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS case_version INT;

-- Backfill case_version from current case version when missing.
UPDATE test_results tr
SET case_version = tc.version
FROM test_cases tc
WHERE tr.case_id = tc.id AND tr.case_version IS NULL;

-- History query indexes (100k+ cases / multi-run timelines).
CREATE INDEX IF NOT EXISTS test_results_case_executed_idx
    ON test_results (case_id, executed_at DESC);
CREATE INDEX IF NOT EXISTS test_results_case_status_idx
    ON test_results (case_id, status);
CREATE INDEX IF NOT EXISTS test_results_case_run_idx
    ON test_results (case_id, run_id);
CREATE INDEX IF NOT EXISTS test_results_run_case_executed_idx
    ON test_results (run_id, case_id, executed_at DESC);
CREATE INDEX IF NOT EXISTS test_results_run_item_executed_idx
    ON test_results (run_item_id, executed_at DESC)
    WHERE run_item_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS test_results_executed_by_idx
    ON test_results (executed_by)
    WHERE executed_by IS NOT NULL;
CREATE INDEX IF NOT EXISTS test_results_case_executed_by_idx
    ON test_results (case_id, executed_by, executed_at DESC)
    WHERE executed_by IS NOT NULL;

-- ── result_steps ───────────────────────────────────────────────────────────
ALTER TABLE result_steps DROP CONSTRAINT IF EXISTS result_steps_status_check;

UPDATE result_steps SET status = CASE lower(status)
    WHEN 'untested' THEN 'Untested'
    WHEN 'passed' THEN 'Passed'
    WHEN 'failed' THEN 'Failed'
    WHEN 'blocked' THEN 'Blocked'
    WHEN 'skipped' THEN 'Skipped'
    WHEN 'retest' THEN 'Retest'
    WHEN 'error' THEN 'Failed'
    ELSE initcap(status)
END;

ALTER TABLE result_steps ALTER COLUMN status SET DEFAULT 'Untested';

ALTER TABLE result_steps
    ADD CONSTRAINT result_steps_status_check CHECK (
        status IN ('Untested', 'Passed', 'Failed', 'Blocked', 'Skipped')
    );

-- Immutable append-only: forbid UPDATE of result rows (DELETE still allowed for
-- cascade cleanup when a run/project is removed).
CREATE OR REPLACE FUNCTION test_results_forbid_update() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'test_results are immutable (append-only history)';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS test_results_no_update ON test_results;
CREATE TRIGGER test_results_no_update
    BEFORE UPDATE ON test_results
    FOR EACH ROW EXECUTE FUNCTION test_results_forbid_update();

DROP TRIGGER IF EXISTS result_steps_no_update ON result_steps;
CREATE TRIGGER result_steps_no_update
    BEFORE UPDATE ON result_steps
    FOR EACH ROW EXECUTE FUNCTION test_results_forbid_update();
