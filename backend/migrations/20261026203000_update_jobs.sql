-- In-app upgrade job history (Admin → Updates). Forward-only / additive.
CREATE TABLE IF NOT EXISTS update_jobs (
    id UUID PRIMARY KEY,
    target_version TEXT NOT NULL,
    status TEXT NOT NULL,
    progress INT NOT NULL DEFAULT 0,
    message TEXT NOT NULL DEFAULT '',
    log_text TEXT NOT NULL DEFAULT '',
    started_by UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS update_jobs_created_at_idx ON update_jobs (created_at DESC);

INSERT INTO system_settings (key, value)
VALUES
    ('updates_check_interval_hours', '"24"'::jsonb),
    ('updates_last_checked_at', '""'::jsonb),
    ('updates_latest_version', '""'::jsonb),
    ('updates_latest_notes', '""'::jsonb),
    ('updates_latest_url', '""'::jsonb)
ON CONFLICT (key) DO NOTHING;
