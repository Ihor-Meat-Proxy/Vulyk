-- W5: CI provider configs, dispatch history, webhook event log.

CREATE TABLE ci_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    name TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    webhook_secret TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ci_configs_provider_check CHECK (provider IN ('github', 'gitlab'))
);

CREATE INDEX ci_configs_project_id_idx ON ci_configs (project_id);

CREATE TABLE ci_dispatches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    ci_config_id UUID NOT NULL REFERENCES ci_configs (id) ON DELETE CASCADE,
    run_id UUID REFERENCES test_runs (id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    external_id TEXT,
    external_url TEXT,
    inputs JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ci_dispatches_status_check CHECK (
        status IN (
            'queued',
            'dispatched',
            'running',
            'succeeded',
            'failed',
            'cancelled'
        )
    )
);

CREATE INDEX ci_dispatches_project_id_idx ON ci_dispatches (project_id);
CREATE INDEX ci_dispatches_external_id_idx ON ci_dispatches (external_id);

CREATE TABLE webhook_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL,
    event_type TEXT NOT NULL DEFAULT '',
    delivery_id TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    ci_dispatch_id UUID REFERENCES ci_dispatches (id) ON DELETE SET NULL,
    processed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX webhook_events_delivery_idx ON webhook_events (provider, delivery_id);
