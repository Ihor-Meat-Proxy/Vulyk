-- W4: background jobs queue + ingest-oriented columns on runs/results.

CREATE TABLE jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    last_error TEXT,
    run_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    locked_at TIMESTAMPTZ,
    locked_by TEXT,
    result JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    CONSTRAINT jobs_status_check CHECK (
        status IN ('pending', 'running', 'completed', 'failed', 'cancelled')
    )
);

CREATE INDEX jobs_claim_idx ON jobs (run_at)
    WHERE status = 'pending';
CREATE INDEX jobs_status_idx ON jobs (status);

ALTER TABLE test_runs
    ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'manual',
    ADD COLUMN IF NOT EXISTS external_ref TEXT,
    ADD COLUMN IF NOT EXISTS meta JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE test_results
    ADD COLUMN IF NOT EXISTS external_id TEXT,
    ADD COLUMN IF NOT EXISTS duration_ms INT;

CREATE INDEX IF NOT EXISTS test_results_run_external_idx
    ON test_results (run_id, external_id);
