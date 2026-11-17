-- First-class Forgejo / Gitea CI providers (Actions workflow_dispatch via /api/v1).
-- Upgrade-safe: drop + re-add CHECK; existing rows remain valid.

ALTER TABLE ci_configs DROP CONSTRAINT IF EXISTS ci_configs_provider_check;

ALTER TABLE ci_configs
  ADD CONSTRAINT ci_configs_provider_check
  CHECK (provider IN (
    'github', 'gitlab', 'jenkins', 'bitbucket', 'other', 'forgejo', 'gitea'
  ));
