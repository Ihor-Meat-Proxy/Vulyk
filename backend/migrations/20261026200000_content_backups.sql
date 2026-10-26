-- Application-level content backup destinations, policies, runs, watermarks, notifications.

CREATE TABLE IF NOT EXISTS backup_destinations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    secrets_ciphertext BYTEA,
    secrets_nonce BYTEA,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT backup_destinations_kind_check CHECK (
        kind IN ('local', 'github', 'gitlab', 'nextcloud', 'gdrive')
    ),
    CONSTRAINT backup_destinations_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE INDEX IF NOT EXISTS backup_destinations_kind_idx ON backup_destinations (kind);

CREATE TABLE IF NOT EXISTS backup_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES projects (id) ON DELETE CASCADE,
    destination_id UUID NOT NULL REFERENCES backup_destinations (id) ON DELETE CASCADE,
    schedule TEXT NOT NULL DEFAULT 'daily',
    debounce_minutes INT NOT NULL DEFAULT 30,
    retain_count INT,
    retain_days INT,
    pack_mode TEXT NOT NULL DEFAULT 'zip',
    include_attachments BOOLEAN NOT NULL DEFAULT TRUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_scheduled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT backup_policies_schedule_check CHECK (
        schedule IN ('hourly', 'daily', 'manual_only')
    ),
    CONSTRAINT backup_policies_pack_mode_check CHECK (
        pack_mode IN ('zip', 'unpacked')
    ),
    CONSTRAINT backup_policies_debounce_nonneg CHECK (debounce_minutes >= 0)
);

CREATE INDEX IF NOT EXISTS backup_policies_destination_idx ON backup_policies (destination_id);
CREATE INDEX IF NOT EXISTS backup_policies_project_idx ON backup_policies (project_id);

CREATE TABLE IF NOT EXISTS backup_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_id UUID REFERENCES backup_policies (id) ON DELETE SET NULL,
    destination_id UUID REFERENCES backup_destinations (id) ON DELETE SET NULL,
    project_id UUID REFERENCES projects (id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    trigger TEXT NOT NULL DEFAULT 'manual',
    remote_path TEXT,
    local_path TEXT,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    duration_ms INT,
    error TEXT,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    checksum_sha256 TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT backup_runs_status_check CHECK (
        status IN ('pending', 'running', 'success', 'failed', 'skipped')
    ),
    CONSTRAINT backup_runs_trigger_check CHECK (
        trigger IN ('schedule', 'manual', 'restore')
    )
);

CREATE INDEX IF NOT EXISTS backup_runs_started_idx ON backup_runs (started_at DESC);
CREATE INDEX IF NOT EXISTS backup_runs_project_idx ON backup_runs (project_id);
CREATE INDEX IF NOT EXISTS backup_runs_status_idx ON backup_runs (status);

CREATE TABLE IF NOT EXISTS backup_change_watermarks (
    project_id UUID PRIMARY KEY REFERENCES projects (id) ON DELETE CASCADE,
    cases_updated_at TIMESTAMPTZ,
    requirements_updated_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    content_seq BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users (id) ON DELETE CASCADE,
    kind TEXT NOT NULL DEFAULT 'info',
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS notifications_user_unread_idx
    ON notifications (user_id, created_at DESC)
    WHERE read_at IS NULL;
