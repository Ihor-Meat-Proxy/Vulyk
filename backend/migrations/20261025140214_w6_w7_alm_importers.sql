-- W6/W7: requirements traceability, ALM integrations, defect links.
-- Timestamped to avoid collisions with parallel workstream migrations.

ALTER TABLE requirements
    ADD COLUMN IF NOT EXISTS url TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS external_id TEXT,
    ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE UNIQUE INDEX IF NOT EXISTS requirements_project_source_external_key_uidx
    ON requirements (project_id, source, external_key)
    WHERE external_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS case_requirements (
    case_id UUID NOT NULL REFERENCES test_cases (id) ON DELETE CASCADE,
    requirement_id UUID NOT NULL REFERENCES requirements (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (case_id, requirement_id)
);

CREATE INDEX IF NOT EXISTS case_requirements_requirement_id_idx
    ON case_requirements (requirement_id);

CREATE TABLE IF NOT EXISTS integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL DEFAULT '',
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    secrets JSONB NOT NULL DEFAULT '{}'::jsonb,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT integrations_kind_check CHECK (
        kind IN ('openproject', 'jira_cloud', 'jira_dc')
    )
);

CREATE INDEX IF NOT EXISTS integrations_project_id_idx ON integrations (project_id);

CREATE TABLE IF NOT EXISTS defects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    integration_id UUID REFERENCES integrations (id) ON DELETE SET NULL,
    result_id UUID REFERENCES test_results (id) ON DELETE SET NULL,
    case_id UUID REFERENCES test_cases (id) ON DELETE SET NULL,
    external_key TEXT,
    external_url TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS defects_project_id_idx ON defects (project_id);
CREATE INDEX IF NOT EXISTS defects_result_id_idx ON defects (result_id);
