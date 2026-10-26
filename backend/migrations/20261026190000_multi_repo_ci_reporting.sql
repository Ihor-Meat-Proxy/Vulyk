-- Multi-repo automation links (M2M) + project CI tokens + CI run metadata.
-- Migrates existing single-repo-per-project rows without data loss.

-- ── automation_repos: app-level named repos ────────────────────────────────
ALTER TABLE automation_repos
    ADD COLUMN IF NOT EXISTS name TEXT NOT NULL DEFAULT '';

-- Backfill names from URL basename when empty.
UPDATE automation_repos
SET name = COALESCE(
    NULLIF(trim(name), ''),
    NULLIF(
        regexp_replace(
            regexp_replace(remote_url, '/$', ''),
            '^.*/',
            ''
        ),
        ''
    ),
    'Automation repo'
)
WHERE trim(name) = '';

ALTER TABLE automation_repos DROP CONSTRAINT IF EXISTS automation_repos_project_unique;

-- Keep project_id temporarily for rollback clarity; junction is source of truth.
CREATE TABLE IF NOT EXISTS project_automation_repos (
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    repo_id UUID NOT NULL REFERENCES automation_repos (id) ON DELETE CASCADE,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, repo_id)
);

CREATE INDEX IF NOT EXISTS project_automation_repos_repo_idx
    ON project_automation_repos (repo_id);

-- Migrate existing 1:1 links.
INSERT INTO project_automation_repos (project_id, repo_id, is_default, sort_order)
SELECT project_id, id, TRUE, 0
FROM automation_repos
WHERE project_id IS NOT NULL
ON CONFLICT DO NOTHING;

-- Ensure at most one default per project.
CREATE UNIQUE INDEX IF NOT EXISTS project_automation_repos_one_default_idx
    ON project_automation_repos (project_id)
    WHERE is_default;

-- project_id on automation_repos becomes optional (legacy / display only).
ALTER TABLE automation_repos
    ALTER COLUMN project_id DROP NOT NULL;

-- ── project CI API tokens (hashed at rest) ─────────────────────────────────
CREATE TABLE IF NOT EXISTS project_ci_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_prefix TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_by UUID REFERENCES users (id) ON DELETE SET NULL,
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS project_ci_tokens_project_idx
    ON project_ci_tokens (project_id);
CREATE INDEX IF NOT EXISTS project_ci_tokens_prefix_idx
    ON project_ci_tokens (token_prefix);

-- ── CI metadata on test_runs ───────────────────────────────────────────────
ALTER TABLE test_runs
    ADD COLUMN IF NOT EXISTS suite_name TEXT,
    ADD COLUMN IF NOT EXISTS ci_provider TEXT,
    ADD COLUMN IF NOT EXISTS ci_repo TEXT,
    ADD COLUMN IF NOT EXISTS ci_branch TEXT,
    ADD COLUMN IF NOT EXISTS ci_commit TEXT,
    ADD COLUMN IF NOT EXISTS pipeline_url TEXT,
    ADD COLUMN IF NOT EXISTS job_url TEXT,
    ADD COLUMN IF NOT EXISTS lifecycle TEXT NOT NULL DEFAULT 'legacy';

-- lifecycle: legacy | ci — controls auto-close vs open/in_progress/closed
ALTER TABLE test_runs DROP CONSTRAINT IF EXISTS test_runs_lifecycle_check;
ALTER TABLE test_runs
    ADD CONSTRAINT test_runs_lifecycle_check CHECK (
        lifecycle IN ('legacy', 'ci')
    );

-- Idempotent CI open: same external_ref returns the same run.
CREATE UNIQUE INDEX IF NOT EXISTS test_runs_project_external_ref_uidx
    ON test_runs (project_id, external_ref)
    WHERE external_ref IS NOT NULL AND external_ref <> '';

-- ── latest automation status on cases ──────────────────────────────────────
ALTER TABLE test_cases
    ADD COLUMN IF NOT EXISTS last_automation_status TEXT,
    ADD COLUMN IF NOT EXISTS last_automation_run_id UUID REFERENCES test_runs (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS last_automation_at TIMESTAMPTZ;
