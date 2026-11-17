-- Widen automation_repos providers for multi-host projects (issue #255).
-- Backfill any legacy singular project_id rows missing a junction link.

ALTER TABLE automation_repos
    DROP CONSTRAINT IF EXISTS automation_repos_provider_check;

ALTER TABLE automation_repos
    ADD CONSTRAINT automation_repos_provider_check CHECK (
        provider IN (
            'github',
            'gitlab',
            'bitbucket',
            'forgejo',
            'gitea',
            'generic'
        )
    );

-- Ensure every legacy 1:1 row is visible via project_automation_repos.
INSERT INTO project_automation_repos (project_id, repo_id, is_default, sort_order)
SELECT ar.project_id, ar.id, TRUE, 0
FROM automation_repos ar
WHERE ar.project_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1
      FROM project_automation_repos par
      WHERE par.repo_id = ar.id
        AND par.project_id = ar.project_id
  )
ON CONFLICT DO NOTHING;

-- If a project has linked repos but none marked default, promote the first.
WITH first_repo AS (
    SELECT DISTINCT ON (project_id) project_id, repo_id
    FROM project_automation_repos
    ORDER BY project_id, sort_order ASC, repo_id ASC
),
needs AS (
    SELECT fr.project_id, fr.repo_id
    FROM first_repo fr
    WHERE NOT EXISTS (
        SELECT 1
        FROM project_automation_repos p
        WHERE p.project_id = fr.project_id
          AND p.is_default
    )
)
UPDATE project_automation_repos par
SET is_default = TRUE
FROM needs n
WHERE par.project_id = n.project_id
  AND par.repo_id = n.repo_id;
