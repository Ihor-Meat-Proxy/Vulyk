-- Core entities for Vulyk Phase 1.

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT projects_key_format CHECK (key ~ '^[A-Z][A-Z0-9_]{1,31}$')
);

CREATE TABLE suites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    parent_id UUID REFERENCES suites (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX suites_project_id_idx ON suites (project_id);
CREATE INDEX suites_parent_id_idx ON suites (parent_id);

CREATE TABLE test_cases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    suite_id UUID REFERENCES suites (id) ON DELETE SET NULL,
    key TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    precondition TEXT NOT NULL DEFAULT '',
    priority TEXT NOT NULL DEFAULT 'medium',
    status TEXT NOT NULL DEFAULT 'draft',
    version INT NOT NULL DEFAULT 1,
    search_vector TSVECTOR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, key)
);

CREATE INDEX test_cases_project_id_idx ON test_cases (project_id);
CREATE INDEX test_cases_suite_id_idx ON test_cases (suite_id);
CREATE INDEX test_cases_title_trgm_idx ON test_cases USING GIN (title gin_trgm_ops);
CREATE INDEX test_cases_search_vector_idx ON test_cases USING GIN (search_vector);

CREATE TABLE test_steps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES test_cases (id) ON DELETE CASCADE,
    position INT NOT NULL,
    action TEXT NOT NULL DEFAULT '',
    expected_result TEXT NOT NULL DEFAULT '',
    data TEXT NOT NULL DEFAULT '',
    UNIQUE (case_id, position)
);

CREATE INDEX test_steps_case_id_idx ON test_steps (case_id);

CREATE TABLE requirements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    external_key TEXT,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX requirements_project_id_idx ON requirements (project_id);

CREATE TABLE test_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX test_runs_project_id_idx ON test_runs (project_id);

CREATE TABLE test_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES test_runs (id) ON DELETE CASCADE,
    case_id UUID REFERENCES test_cases (id) ON DELETE SET NULL,
    status TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT 'manual',
    raw JSONB,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX test_results_run_id_idx ON test_results (run_id);
CREATE INDEX test_results_case_id_idx ON test_results (case_id);
CREATE INDEX test_results_status_idx ON test_results (status);

-- Keep FTS vector in sync with case title/description.
CREATE OR REPLACE FUNCTION test_cases_search_vector_update() RETURNS trigger AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('english', coalesce(NEW.title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(NEW.description, '')), 'B') ||
        setweight(to_tsvector('english', coalesce(NEW.precondition, '')), 'C');
    NEW.updated_at := NOW();
    RETURN NEW;
END
$$ LANGUAGE plpgsql;

CREATE TRIGGER test_cases_search_vector_trigger
    BEFORE INSERT OR UPDATE OF title, description, precondition ON test_cases
    FOR EACH ROW EXECUTE FUNCTION test_cases_search_vector_update();
