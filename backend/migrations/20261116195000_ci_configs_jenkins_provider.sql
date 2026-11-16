-- Widen ci_configs.provider to first-class Jenkins (and inbound-friendly bitbucket/other).
-- Routes already accept these values; the CHECK constraint from W5 blocked inserts.
-- Upgrade-safe: drop + re-add constraint; existing github/gitlab rows remain valid.

ALTER TABLE ci_configs DROP CONSTRAINT IF EXISTS ci_configs_provider_check;

ALTER TABLE ci_configs
  ADD CONSTRAINT ci_configs_provider_check
  CHECK (provider IN ('github', 'gitlab', 'jenkins', 'bitbucket', 'other'));
