-- Make campaign/iteration find-or-create race-safe (pipeline ingest / CI open).
-- Dedupe existing duplicates (keep oldest id), then add unique indexes.

-- --- campaigns: merge duplicate (project_id, parent_id, name) ---
WITH ranked AS (
  SELECT id,
         project_id,
         parent_id,
         name,
         ROW_NUMBER() OVER (
           PARTITION BY project_id, parent_id, name
           ORDER BY created_at ASC, id ASC
         ) AS rn
  FROM campaigns
),
keepers AS (
  SELECT id, project_id, parent_id, name FROM ranked WHERE rn = 1
),
dupes AS (
  SELECT r.id AS dupe_id, k.id AS keep_id
  FROM ranked r
  JOIN keepers k
    ON k.project_id = r.project_id
   AND k.name = r.name
   AND ((k.parent_id IS NULL AND r.parent_id IS NULL) OR k.parent_id = r.parent_id)
  WHERE r.rn > 1
)
UPDATE iterations i
SET campaign_id = d.keep_id
FROM dupes d
WHERE i.campaign_id = d.dupe_id;

WITH ranked AS (
  SELECT id,
         project_id,
         parent_id,
         name,
         ROW_NUMBER() OVER (
           PARTITION BY project_id, parent_id, name
           ORDER BY created_at ASC, id ASC
         ) AS rn
  FROM campaigns
),
keepers AS (
  SELECT id, project_id, parent_id, name FROM ranked WHERE rn = 1
),
dupes AS (
  SELECT r.id AS dupe_id, k.id AS keep_id
  FROM ranked r
  JOIN keepers k
    ON k.project_id = r.project_id
   AND k.name = r.name
   AND ((k.parent_id IS NULL AND r.parent_id IS NULL) OR k.parent_id = r.parent_id)
  WHERE r.rn > 1
)
UPDATE campaigns c
SET parent_id = d.keep_id
FROM dupes d
WHERE c.parent_id = d.dupe_id;

WITH ranked AS (
  SELECT id,
         ROW_NUMBER() OVER (
           PARTITION BY project_id, parent_id, name
           ORDER BY created_at ASC, id ASC
         ) AS rn
  FROM campaigns
)
DELETE FROM campaigns WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

-- --- iterations: merge duplicate (project_id, campaign_id, name) ---
WITH ranked AS (
  SELECT id,
         project_id,
         campaign_id,
         name,
         ROW_NUMBER() OVER (
           PARTITION BY project_id, campaign_id, name
           ORDER BY created_at ASC, id ASC
         ) AS rn
  FROM iterations
),
keepers AS (
  SELECT id, project_id, campaign_id, name FROM ranked WHERE rn = 1
),
dupes AS (
  SELECT r.id AS dupe_id, k.id AS keep_id
  FROM ranked r
  JOIN keepers k
    ON k.project_id = r.project_id
   AND k.campaign_id = r.campaign_id
   AND k.name = r.name
  WHERE r.rn > 1
)
UPDATE test_runs tr
SET iteration_id = d.keep_id
FROM dupes d
WHERE tr.iteration_id = d.dupe_id;

WITH ranked AS (
  SELECT id,
         project_id,
         campaign_id,
         name,
         ROW_NUMBER() OVER (
           PARTITION BY project_id, campaign_id, name
           ORDER BY created_at ASC, id ASC
         ) AS rn
  FROM iterations
),
keepers AS (
  SELECT id, project_id, campaign_id, name FROM ranked WHERE rn = 1
),
dupes AS (
  SELECT r.id AS dupe_id, k.id AS keep_id
  FROM ranked r
  JOIN keepers k
    ON k.project_id = r.project_id
   AND k.campaign_id = r.campaign_id
   AND k.name = r.name
  WHERE r.rn > 1
)
UPDATE iteration_items ii
SET iteration_id = d.keep_id
FROM dupes d
WHERE ii.iteration_id = d.dupe_id
  AND NOT EXISTS (
    SELECT 1 FROM iteration_items x
    WHERE x.iteration_id = d.keep_id AND x.case_id = ii.case_id
  );

DELETE FROM iteration_items
WHERE iteration_id IN (
  SELECT id FROM (
    SELECT id,
           ROW_NUMBER() OVER (
             PARTITION BY project_id, campaign_id, name
             ORDER BY created_at ASC, id ASC
           ) AS rn
    FROM iterations
  ) r WHERE rn > 1
);

WITH ranked AS (
  SELECT id,
         ROW_NUMBER() OVER (
           PARTITION BY project_id, campaign_id, name
           ORDER BY created_at ASC, id ASC
         ) AS rn
  FROM iterations
)
DELETE FROM iterations WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

-- Reset orphaned Running iterations with no runs / no items to Undefined
UPDATE iterations i
SET status = 'Undefined', updated_at = NOW()
WHERE i.status = 'Running'
  AND NOT EXISTS (SELECT 1 FROM test_runs tr WHERE tr.iteration_id = i.id)
  AND NOT EXISTS (SELECT 1 FROM iteration_items ii WHERE ii.iteration_id = i.id);

CREATE UNIQUE INDEX campaigns_project_parent_name_uidx
  ON campaigns (project_id, parent_id, name)
  NULLS NOT DISTINCT;

CREATE UNIQUE INDEX iterations_project_campaign_name_uidx
  ON iterations (project_id, campaign_id, name);
