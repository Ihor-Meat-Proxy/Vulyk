-- Archive-based indexing helpers: webhook auto-reindex, poll interval, index trigger metadata.

ALTER TABLE automation_repos
    ADD COLUMN IF NOT EXISTS webhook_secret TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS poll_interval_minutes INT NOT NULL DEFAULT 15,
    ADD COLUMN IF NOT EXISTS last_index_trigger TEXT,
    ADD COLUMN IF NOT EXISTS poll_etag TEXT,
    ADD COLUMN IF NOT EXISTS last_polled_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS seen_branches TEXT[] NOT NULL DEFAULT '{}';

-- Backfill webhook secrets for existing rows (operators can rotate from the UI).
UPDATE automation_repos
SET webhook_secret = encode(gen_random_bytes(24), 'hex')
WHERE webhook_secret IS NULL OR webhook_secret = '';

ALTER TABLE automation_repos
    DROP CONSTRAINT IF EXISTS automation_repos_poll_interval_check;

ALTER TABLE automation_repos
    ADD CONSTRAINT automation_repos_poll_interval_check
    CHECK (poll_interval_minutes >= 0);

ALTER TABLE automation_repos
    DROP CONSTRAINT IF EXISTS automation_repos_index_trigger_check;

ALTER TABLE automation_repos
    ADD CONSTRAINT automation_repos_index_trigger_check
    CHECK (
        last_index_trigger IS NULL
        OR last_index_trigger IN ('manual', 'webhook', 'poll')
    );

COMMENT ON COLUMN automation_repos.webhook_secret IS
    'Shared secret for GitHub HMAC / GitLab token webhook verification (shown in UI)';
COMMENT ON COLUMN automation_repos.poll_interval_minutes IS
    'Branch-head poll interval in minutes; 0 disables polling (default 15)';
COMMENT ON COLUMN automation_repos.last_index_trigger IS
    'How the last successful (or attempted) index was triggered: manual|webhook|poll';
COMMENT ON COLUMN automation_repos.poll_etag IS
    'ETag from last branch-head poll (If-None-Match) to avoid burning rate limit';
COMMENT ON COLUMN automation_repos.seen_branches IS
    'Branch names previously listed; used to flag newly appeared branches in the UI';
