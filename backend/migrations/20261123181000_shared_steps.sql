-- Shared steps library (#235): project-scoped reusable step blocks.
CREATE TABLE shared_steps (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (project_id, name)
);
CREATE INDEX shared_steps_project_id_idx ON shared_steps (project_id);

CREATE TABLE shared_step_lines (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  shared_step_id UUID NOT NULL REFERENCES shared_steps (id) ON DELETE CASCADE,
  position INT NOT NULL,
  action TEXT NOT NULL DEFAULT '',
  expected_result TEXT NOT NULL DEFAULT '',
  data TEXT NOT NULL DEFAULT '',
  UNIQUE (shared_step_id, position)
);
CREATE INDEX shared_step_lines_shared_step_id_idx ON shared_step_lines (shared_step_id);

-- Deferred NO ACTION instead of RESTRICT: projects cascade to both shared_steps
-- and test_cases, and an immediate check would fail project deletion whenever
-- the shared_steps cascade fires before test_steps are gone (trigger order is
-- not stable across dump/restore). Deleting an in-use shared step still fails.
ALTER TABLE test_steps
  ADD COLUMN shared_step_id UUID NULL REFERENCES shared_steps (id)
    ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED;
CREATE INDEX test_steps_shared_step_id_idx ON test_steps (shared_step_id);
