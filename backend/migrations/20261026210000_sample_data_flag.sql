-- Mark seeded demo entities so admins can list / remove / re-seed them safely.

ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS is_sample BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE test_runs
    ADD COLUMN IF NOT EXISTS is_sample BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE campaigns
    ADD COLUMN IF NOT EXISTS is_sample BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS projects_is_sample_idx ON projects (is_sample)
    WHERE is_sample = TRUE;
CREATE INDEX IF NOT EXISTS test_runs_is_sample_idx ON test_runs (is_sample)
    WHERE is_sample = TRUE;

-- Backfill known sample project keys (Kyiv concert + Vulyk QA).
UPDATE projects
SET is_sample = TRUE
WHERE key IN ('KYIV_CONCERT_2026', 'VULYK_QA');

-- All runs under sample projects.
UPDATE test_runs r
SET is_sample = TRUE
FROM projects p
WHERE r.project_id = p.id
  AND p.is_sample = TRUE;

-- Legacy seed markers on non-flagged projects (name / source / creator).
UPDATE test_runs
SET is_sample = TRUE
WHERE is_sample = FALSE
  AND (
    source = 'sample'
    OR created_by_name IN ('WALL-E-rror', 'Sample CI')
    OR name ILIKE 'CI #%% — wall-e %%'
    OR name ILIKE 'Sample nightly #%%'
    OR name IN ('Ticketing smoke run', 'Dress rehearsal run')
    OR name ILIKE 'Vulyk QA smoke%'
    OR name ILIKE 'Vulyk QA platform%'
  );

-- Campaigns under sample projects (executions tree).
UPDATE campaigns c
SET is_sample = TRUE
FROM projects p
WHERE c.project_id = p.id
  AND p.is_sample = TRUE;
