-- Human-readable, never-reused display ids / URL slugs for campaigns, iterations, runs.
-- Counters live on projects (same pattern as next_case_number). Upgrade-safe from 0.4.0.

ALTER TABLE projects
  ADD COLUMN IF NOT EXISTS next_run_number INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS next_iteration_number INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS next_campaign_number INT NOT NULL DEFAULT 0;

ALTER TABLE campaigns
  ADD COLUMN IF NOT EXISTS display_id TEXT,
  ADD COLUMN IF NOT EXISTS seq_number INT;

ALTER TABLE iterations
  ADD COLUMN IF NOT EXISTS display_id TEXT,
  ADD COLUMN IF NOT EXISTS seq_number INT;

ALTER TABLE test_runs
  ADD COLUMN IF NOT EXISTS display_id TEXT,
  ADD COLUMN IF NOT EXISTS slug TEXT,
  ADD COLUMN IF NOT EXISTS seq_number INT;

-- --- Backfill campaigns (creation order) ---
WITH numbered AS (
  SELECT c.id,
         c.project_id,
         p.case_key_prefix AS prefix,
         ROW_NUMBER() OVER (
           PARTITION BY c.project_id ORDER BY c.created_at ASC, c.id ASC
         ) AS n
  FROM campaigns c
  JOIN projects p ON p.id = c.project_id
)
UPDATE campaigns c
SET seq_number = numbered.n,
    display_id = numbered.prefix || '-C-' || numbered.n
FROM numbered
WHERE c.id = numbered.id;

UPDATE projects p
SET next_campaign_number = COALESCE((
  SELECT MAX(seq_number) FROM campaigns c WHERE c.project_id = p.id
), 0);

-- --- Backfill iterations ---
WITH numbered AS (
  SELECT i.id,
         i.project_id,
         p.case_key_prefix AS prefix,
         ROW_NUMBER() OVER (
           PARTITION BY i.project_id ORDER BY i.created_at ASC, i.id ASC
         ) AS n
  FROM iterations i
  JOIN projects p ON p.id = i.project_id
)
UPDATE iterations i
SET seq_number = numbered.n,
    display_id = numbered.prefix || '-IT-' || numbered.n
FROM numbered
WHERE i.id = numbered.id;

UPDATE projects p
SET next_iteration_number = COALESCE((
  SELECT MAX(seq_number) FROM iterations i WHERE i.project_id = p.id
), 0);

-- --- Backfill runs (display_id + slug) ---
WITH numbered AS (
  SELECT r.id,
         r.project_id,
         r.created_at,
         r.source,
         r.ci_provider,
         r.lifecycle,
         p.case_key_prefix AS prefix,
         ROW_NUMBER() OVER (
           PARTITION BY r.project_id ORDER BY r.created_at ASC, r.id ASC
         ) AS n
  FROM test_runs r
  JOIN projects p ON p.id = r.project_id
),
sourced AS (
  SELECT id,
         project_id,
         prefix,
         n,
         created_at,
         CASE
           WHEN lower(coalesce(ci_provider, '')) IN ('jenkins', 'gitlab', 'github', 'bitbucket', 'other')
             THEN lower(ci_provider)
           WHEN lower(coalesce(source, '')) LIKE '%jenkins%' THEN 'jenkins'
           WHEN lower(coalesce(source, '')) LIKE '%gitlab%' THEN 'gitlab'
           WHEN lower(coalesce(source, '')) LIKE '%github%' THEN 'github'
           WHEN lower(coalesce(source, '')) LIKE '%bitbucket%' THEN 'bitbucket'
           WHEN coalesce(lifecycle, '') = 'ci'
             OR lower(coalesce(source, '')) LIKE 'ci%'
             OR lower(coalesce(source, '')) = 'ci'
             THEN 'other'
           ELSE 'manual'
         END AS src
  FROM numbered
)
UPDATE test_runs r
SET seq_number = s.n,
    display_id = s.prefix || '-R-' || s.n,
    slug = s.prefix || '-R-' || s.src || '-' || s.n || '-' ||
           to_char(s.created_at AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"')
FROM sourced s
WHERE r.id = s.id;

UPDATE projects p
SET next_run_number = COALESCE((
  SELECT MAX(seq_number) FROM test_runs r WHERE r.project_id = p.id
), 0);

-- Enforce after backfill
ALTER TABLE campaigns
  ALTER COLUMN display_id SET NOT NULL,
  ALTER COLUMN seq_number SET NOT NULL;

ALTER TABLE iterations
  ALTER COLUMN display_id SET NOT NULL,
  ALTER COLUMN seq_number SET NOT NULL;

ALTER TABLE test_runs
  ALTER COLUMN display_id SET NOT NULL,
  ALTER COLUMN slug SET NOT NULL,
  ALTER COLUMN seq_number SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS campaigns_project_display_id_uidx
  ON campaigns (project_id, display_id);
CREATE UNIQUE INDEX IF NOT EXISTS campaigns_project_seq_uidx
  ON campaigns (project_id, seq_number);

CREATE UNIQUE INDEX IF NOT EXISTS iterations_project_display_id_uidx
  ON iterations (project_id, display_id);
CREATE UNIQUE INDEX IF NOT EXISTS iterations_project_seq_uidx
  ON iterations (project_id, seq_number);

CREATE UNIQUE INDEX IF NOT EXISTS test_runs_project_display_id_uidx
  ON test_runs (project_id, display_id);
CREATE UNIQUE INDEX IF NOT EXISTS test_runs_project_slug_uidx
  ON test_runs (project_id, slug);
CREATE UNIQUE INDEX IF NOT EXISTS test_runs_project_seq_uidx
  ON test_runs (project_id, seq_number);

-- Global slug uniqueness (deep links without project in path)
CREATE UNIQUE INDEX IF NOT EXISTS test_runs_slug_uidx ON test_runs (slug);
CREATE UNIQUE INDEX IF NOT EXISTS iterations_display_id_uidx ON iterations (display_id);
CREATE UNIQUE INDEX IF NOT EXISTS campaigns_display_id_uidx ON campaigns (display_id);
