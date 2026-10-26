-- Campaigns / iterations (SquashTM-inspired executions tree) + automated suite metadata.
-- Status vocabulary extended with Ready, Running, Untestable (Ready ≈ Untested for dashboards).

-- ── campaigns (nested folders) ─────────────────────────────────────────────
CREATE TABLE campaigns (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    parent_id UUID REFERENCES campaigns (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX campaigns_project_id_idx ON campaigns (project_id);
CREATE INDEX campaigns_parent_id_idx ON campaigns (parent_id);

-- ── iterations ─────────────────────────────────────────────────────────────
CREATE TABLE iterations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    campaign_id UUID NOT NULL REFERENCES campaigns (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    reference TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    -- Undefined | Ready | Running | Completed
    status TEXT NOT NULL DEFAULT 'Undefined',
    -- automated | manual | mixed
    mode TEXT NOT NULL DEFAULT 'mixed',
    cadence TEXT NOT NULL DEFAULT '',
    scheduled_start TIMESTAMPTZ,
    scheduled_end TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT iterations_status_check CHECK (
        status IN ('Undefined', 'Ready', 'Running', 'Completed')
    ),
    CONSTRAINT iterations_mode_check CHECK (
        mode IN ('automated', 'manual', 'mixed')
    )
);

CREATE INDEX iterations_project_id_idx ON iterations (project_id);
CREATE INDEX iterations_campaign_id_idx ON iterations (campaign_id);

-- Cases assigned to an iteration (current status for dashboard donut).
CREATE TABLE iteration_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    iteration_id UUID NOT NULL REFERENCES iterations (id) ON DELETE CASCADE,
    case_id UUID NOT NULL REFERENCES test_cases (id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'Ready',
    sort_order INT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (iteration_id, case_id),
    CONSTRAINT iteration_items_status_check CHECK (
        status IN (
            'Ready',
            'Running',
            'Passed',
            'Failed',
            'Blocked',
            'Skipped',
            'Untestable',
            'Untested',
            'Retest'
        )
    )
);

CREATE INDEX iteration_items_iteration_id_idx ON iteration_items (iteration_id);
CREATE INDEX iteration_items_case_id_idx ON iteration_items (case_id);
CREATE INDEX iteration_items_status_idx ON iteration_items (iteration_id, status);

-- ── extend test_runs for automated suites under iterations ─────────────────
ALTER TABLE test_runs
    ADD COLUMN IF NOT EXISTS iteration_id UUID REFERENCES iterations (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS created_by_name TEXT,
    ADD COLUMN IF NOT EXISTS environment JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS artifacts JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS pass_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS fail_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS skip_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS blocked_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS duration_ms BIGINT;

CREATE INDEX IF NOT EXISTS test_runs_iteration_id_idx ON test_runs (iteration_id);
CREATE INDEX IF NOT EXISTS test_runs_iteration_created_idx
    ON test_runs (iteration_id, created_at DESC);

-- ── extend result status vocabulary ────────────────────────────────────────
ALTER TABLE run_items DROP CONSTRAINT IF EXISTS run_items_status_check;
ALTER TABLE run_items
    ADD CONSTRAINT run_items_status_check CHECK (
        status IN (
            'Untested',
            'Ready',
            'Running',
            'Passed',
            'Failed',
            'Blocked',
            'Skipped',
            'Retest',
            'Untestable'
        )
    );

ALTER TABLE test_results DROP CONSTRAINT IF EXISTS test_results_status_check;
ALTER TABLE test_results
    ADD CONSTRAINT test_results_status_check CHECK (
        status IN (
            'Untested',
            'Ready',
            'Running',
            'Passed',
            'Failed',
            'Blocked',
            'Skipped',
            'Retest',
            'Untestable'
        )
    );

ALTER TABLE result_steps DROP CONSTRAINT IF EXISTS result_steps_status_check;
ALTER TABLE result_steps
    ADD CONSTRAINT result_steps_status_check CHECK (
        status IN (
            'Untested',
            'Ready',
            'Running',
            'Passed',
            'Failed',
            'Blocked',
            'Skipped',
            'Untestable'
        )
    );

-- Optional environment / dataset fields on results (from CI metadata).
ALTER TABLE test_results
    ADD COLUMN IF NOT EXISTS environment TEXT,
    ADD COLUMN IF NOT EXISTS environment_vars JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS dataset TEXT;
