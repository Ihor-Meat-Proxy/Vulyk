-- Nullable automation_id on test cases (unique per project when set).
-- Ingest matches case key first, then automation_id, then external_key.
ALTER TABLE test_cases ADD COLUMN IF NOT EXISTS automation_id TEXT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS test_cases_project_automation_id_uidx
    ON test_cases (project_id, automation_id)
    WHERE automation_id IS NOT NULL;
