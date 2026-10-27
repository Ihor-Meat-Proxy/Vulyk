-- Encrypted GitHub Actions write token + workflow_dispatch settings per automation repo.
-- Used by "Run in CI" (Vulyk → workflow_dispatch with vulyk_run_id / case_keys).

ALTER TABLE automation_repos
    ADD COLUMN IF NOT EXISTS actions_token_ciphertext BYTEA,
    ADD COLUMN IF NOT EXISTS actions_token_nonce BYTEA,
    ADD COLUMN IF NOT EXISTS ci_workflow_file TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS ci_workflow_ref TEXT NOT NULL DEFAULT 'main';

COMMENT ON COLUMN automation_repos.actions_token_ciphertext IS
    'Encrypted GitHub token with Actions write (workflow_dispatch); separate from read/index token';
COMMENT ON COLUMN automation_repos.ci_workflow_file IS
    'Workflow file path or id for workflow_dispatch (e.g. .github/workflows/vulyk.yml)';
COMMENT ON COLUMN automation_repos.ci_workflow_ref IS
    'Git ref passed to workflow_dispatch (branch or tag)';
