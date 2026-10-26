-- Automatic case keys + folder/case ordering support.
-- Projects gain case_key_prefix + next_case_number.
-- Deleted case keys are tombstoned so numbers are never reused.
-- test_cases.external_key stores original import IDs.

ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS case_key_prefix TEXT,
    ADD COLUMN IF NOT EXISTS next_case_number INT NOT NULL DEFAULT 0;

-- Prefer the most common PREFIX from existing PREFIX-N case keys.
UPDATE projects p
SET case_key_prefix = sub.prefix
FROM (
    SELECT
        tc.project_id,
        UPPER((regexp_match(tc.key, '^([A-Z][A-Z0-9]{0,7})-[0-9]+$'))[1]) AS prefix,
        COUNT(*) AS cnt,
        ROW_NUMBER() OVER (
            PARTITION BY tc.project_id
            ORDER BY COUNT(*) DESC, UPPER((regexp_match(tc.key, '^([A-Z][A-Z0-9]{0,7})-[0-9]+$'))[1])
        ) AS rn
    FROM test_cases tc
    WHERE tc.key ~ '^[A-Z][A-Z0-9]{0,7}-[0-9]+$'
    GROUP BY tc.project_id, UPPER((regexp_match(tc.key, '^([A-Z][A-Z0-9]{0,7})-[0-9]+$'))[1])
) AS sub
WHERE p.id = sub.project_id
  AND sub.rn = 1
  AND p.case_key_prefix IS NULL;

-- Fallback: first 2–3 alphanumeric chars of the project key.
UPDATE projects
SET case_key_prefix = UPPER(LEFT(regexp_replace(key, '[^A-Za-z0-9]', '', 'g') || 'PR', 3))
WHERE case_key_prefix IS NULL OR length(case_key_prefix) < 2;

-- Normalize length/format before uniqueness pass.
UPDATE projects
SET case_key_prefix = UPPER(LEFT(regexp_replace(case_key_prefix, '[^A-Za-z0-9]', '', 'g'), 8))
WHERE case_key_prefix IS NOT NULL;

UPDATE projects
SET case_key_prefix = 'PR'
WHERE case_key_prefix IS NULL
   OR case_key_prefix !~ '^[A-Z][A-Z0-9]{1,7}$';

-- Deduplicate prefixes (keep earliest project by created_at).
DO $$
DECLARE
    r RECORD;
    stem TEXT;
    attempt TEXT;
    n INT;
BEGIN
    FOR r IN
        SELECT id, case_key_prefix
        FROM projects
        ORDER BY created_at, id
    LOOP
        stem := left(r.case_key_prefix, 6);
        attempt := r.case_key_prefix;
        n := 2;
        WHILE EXISTS (
            SELECT 1 FROM projects
            WHERE case_key_prefix = attempt AND id <> r.id
        ) LOOP
            attempt := left(stem, greatest(1, 8 - length(n::text))) || n::text;
            n := n + 1;
            IF n > 10000 THEN
                RAISE EXCEPTION 'unable to dedupe case_key_prefix for %', r.id;
            END IF;
        END LOOP;
        IF attempt <> r.case_key_prefix THEN
            UPDATE projects SET case_key_prefix = attempt WHERE id = r.id;
        END IF;
    END LOOP;
END $$;

ALTER TABLE projects
    ALTER COLUMN case_key_prefix SET NOT NULL;

ALTER TABLE projects
    DROP CONSTRAINT IF EXISTS projects_case_key_prefix_format;
ALTER TABLE projects
    ADD CONSTRAINT projects_case_key_prefix_format
    CHECK (case_key_prefix ~ '^[A-Z][A-Z0-9]{1,7}$');

CREATE UNIQUE INDEX IF NOT EXISTS projects_case_key_prefix_uidx
    ON projects (case_key_prefix);

-- Counter = max numeric suffix among PREFIX-N keys for this project.
UPDATE projects p
SET next_case_number = COALESCE(
    (
        SELECT MAX((regexp_match(tc.key, '^[A-Z][A-Z0-9]{1,7}-([0-9]+)$'))[1]::INT)
        FROM test_cases tc
        WHERE tc.project_id = p.id
          AND tc.key ~ ('^' || p.case_key_prefix || '-[0-9]+$')
    ),
    next_case_number,
    0
);

ALTER TABLE test_cases
    ADD COLUMN IF NOT EXISTS external_key TEXT,
    ADD COLUMN IF NOT EXISTS sort_order INT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS test_cases_external_key_idx
    ON test_cases (project_id, external_key)
    WHERE external_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS test_cases_suite_sort_idx
    ON test_cases (suite_id, sort_order, key);

CREATE TABLE IF NOT EXISTS case_key_tombstones (
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    case_number INT NOT NULL,
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, key)
);

CREATE INDEX IF NOT EXISTS case_key_tombstones_project_number_idx
    ON case_key_tombstones (project_id, case_number);
