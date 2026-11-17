-- Per-project policy for unmatched CI ingest results (#228).
-- Values: off | create_passed | create_all | fail_unmatched
ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS ci_unmatched_policy TEXT NOT NULL DEFAULT 'off';

ALTER TABLE projects
    DROP CONSTRAINT IF EXISTS projects_ci_unmatched_policy_check;

ALTER TABLE projects
    ADD CONSTRAINT projects_ci_unmatched_policy_check
    CHECK (ci_unmatched_policy IN ('off', 'create_passed', 'create_all', 'fail_unmatched'));
