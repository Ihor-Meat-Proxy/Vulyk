-- Project- and permission-scoped personal API tokens (#229).
-- Existing active tokens are revoked so upgrades force recreation with a scope.

ALTER TABLE api_tokens
    ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES projects (id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS permissions TEXT[] NOT NULL DEFAULT '{}';

CREATE INDEX IF NOT EXISTS api_tokens_project_id_idx ON api_tokens (project_id);

UPDATE api_tokens
SET revoked_at = NOW()
WHERE revoked_at IS NULL;
