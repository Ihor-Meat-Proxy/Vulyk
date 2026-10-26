-- Case list: per-user per-project column preferences + sort/filter indexes.

CREATE TABLE IF NOT EXISTS user_project_preferences (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    case_list_columns JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, project_id)
);

COMMENT ON TABLE user_project_preferences IS
    'Per-user preferences scoped to a project (case list column layout, etc.)';
COMMENT ON COLUMN user_project_preferences.case_list_columns IS
    'Case list column order/visibility + optional sort: { columns: [{id, visible}], sort?, order? }';

CREATE INDEX IF NOT EXISTS user_project_preferences_project_idx
    ON user_project_preferences (project_id);

-- Sort helpers for the Cases table (server-side ORDER BY with pagination).
CREATE INDEX IF NOT EXISTS test_cases_project_priority_idx
    ON test_cases (project_id, priority);

CREATE INDEX IF NOT EXISTS test_cases_project_status_idx
    ON test_cases (project_id, status);

CREATE INDEX IF NOT EXISTS test_cases_project_title_idx
    ON test_cases (project_id, title);

CREATE INDEX IF NOT EXISTS test_cases_project_updated_at_idx
    ON test_cases (project_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS test_cases_project_created_at_idx
    ON test_cases (project_id, created_at DESC);

-- Automated = external_key present (linked to automation / import id).
CREATE INDEX IF NOT EXISTS test_cases_project_automated_idx
    ON test_cases (project_id)
    WHERE external_key IS NOT NULL;

-- Latest execution status lookups (manual results + automation ingest).
CREATE INDEX IF NOT EXISTS test_results_case_executed_idx
    ON test_results (case_id, executed_at DESC NULLS LAST, created_at DESC);

-- Custom-field value lookups for list enrichment / sort by cf:<uuid>.
CREATE INDEX IF NOT EXISTS custom_field_values_field_case_idx
    ON custom_field_values (field_id, case_id);
