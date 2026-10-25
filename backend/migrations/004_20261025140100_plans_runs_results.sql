-- W3: test plans, enriched runs, run items, step-level results.

CREATE TABLE test_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX test_plans_project_id_idx ON test_plans (project_id);

CREATE TABLE test_plan_cases (
    plan_id UUID NOT NULL REFERENCES test_plans (id) ON DELETE CASCADE,
    case_id UUID NOT NULL REFERENCES test_cases (id) ON DELETE CASCADE,
    sort_order INT NOT NULL DEFAULT 0,
    PRIMARY KEY (plan_id, case_id)
);

CREATE INDEX test_plan_cases_case_id_idx ON test_plan_cases (case_id);

ALTER TABLE test_runs
    ADD COLUMN IF NOT EXISTS plan_id UUID REFERENCES test_plans (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS description TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS test_runs_plan_id_idx ON test_runs (plan_id);

CREATE TABLE run_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES test_runs (id) ON DELETE CASCADE,
    case_id UUID NOT NULL REFERENCES test_cases (id) ON DELETE CASCADE,
    assignee TEXT,
    status TEXT NOT NULL DEFAULT 'untested',
    sort_order INT NOT NULL DEFAULT 0,
    comment TEXT NOT NULL DEFAULT '',
    executed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_id, case_id),
    CONSTRAINT run_items_status_check CHECK (
        status IN ('untested', 'passed', 'failed', 'blocked', 'skipped', 'retest')
    )
);

CREATE INDEX run_items_run_id_idx ON run_items (run_id);
CREATE INDEX run_items_case_id_idx ON run_items (case_id);
CREATE INDEX run_items_status_idx ON run_items (run_id, status);
CREATE INDEX run_items_assignee_idx ON run_items (run_id, assignee);

ALTER TABLE test_results
    ADD COLUMN IF NOT EXISTS run_item_id UUID REFERENCES run_items (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS assignee TEXT,
    ADD COLUMN IF NOT EXISTS elapsed_ms INT;

CREATE INDEX IF NOT EXISTS test_results_run_item_id_idx ON test_results (run_item_id);

CREATE TABLE result_steps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    result_id UUID NOT NULL REFERENCES test_results (id) ON DELETE CASCADE,
    step_id UUID REFERENCES test_steps (id) ON DELETE SET NULL,
    position INT NOT NULL,
    action TEXT NOT NULL DEFAULT '',
    expected_result TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'untested',
    actual_result TEXT NOT NULL DEFAULT '',
    comment TEXT NOT NULL DEFAULT '',
    CONSTRAINT result_steps_status_check CHECK (
        status IN ('untested', 'passed', 'failed', 'blocked', 'skipped')
    )
);

CREATE INDEX result_steps_result_id_idx ON result_steps (result_id);

-- Keep result status vocabulary aligned with run items for manual execution.
ALTER TABLE test_results DROP CONSTRAINT IF EXISTS test_results_status_check;
ALTER TABLE test_results
    ADD CONSTRAINT test_results_status_check CHECK (
        status IN ('untested', 'passed', 'failed', 'blocked', 'skipped', 'retest', 'error')
    );
