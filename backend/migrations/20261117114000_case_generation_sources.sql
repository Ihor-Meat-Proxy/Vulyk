-- Provenance for AI-generated cases (PR/branch/commit) — issue #256.

CREATE TABLE case_generation_sources (
    case_id UUID PRIMARY KEY REFERENCES test_cases (id) ON DELETE CASCADE,
    source TEXT NOT NULL DEFAULT 'diff',
    repo_id UUID REFERENCES automation_repos (id) ON DELETE SET NULL,
    provider TEXT NOT NULL DEFAULT '',
    pull_number INT,
    pull_url TEXT,
    base_ref TEXT NOT NULL DEFAULT '',
    head_ref TEXT NOT NULL DEFAULT '',
    commit_sha TEXT,
    included_paths TEXT[] NOT NULL DEFAULT '{}',
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT case_generation_sources_source_check CHECK (
        source IN ('diff', 'requirements', 'chat', 'other')
    )
);

CREATE INDEX case_generation_sources_repo_idx
    ON case_generation_sources (repo_id)
    WHERE repo_id IS NOT NULL;
