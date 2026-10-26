//! Suite/folder helpers — recursive case counts in one query.

use uuid::Uuid;

use crate::error::AppResult;
use crate::models::Suite;

/// List suites for a project with recursive + direct case counts.
///
/// Counts are **unfiltered** (ignore list search/status/tag filters). The cases
/// list panel applies those filters separately; selecting a folder defaults to
/// including descendants unless `direct_only=true`.
///
/// Implementation: one recursive CTE of (root → descendant) edges, one
/// aggregation of direct counts from `test_cases`, then a join — O(folders×depth)
/// + O(cases) and stays fast for 100+ folders / 20k+ cases.
pub async fn list_suites_with_counts(
    pool: &sqlx::PgPool,
    project_id: Uuid,
) -> AppResult<Vec<Suite>> {
    let suites = sqlx::query_as::<_, Suite>(
        r#"
        WITH RECURSIVE suite_desc AS (
            SELECT id AS root_id, id AS descendant_id
            FROM suites
            WHERE project_id = $1
            UNION ALL
            SELECT sd.root_id, s.id
            FROM suites s
            INNER JOIN suite_desc sd ON s.parent_id = sd.descendant_id
            WHERE s.project_id = $1
        ),
        direct AS (
            SELECT suite_id, COUNT(*)::bigint AS n
            FROM test_cases
            WHERE project_id = $1 AND suite_id IS NOT NULL
            GROUP BY suite_id
        ),
        totals AS (
            SELECT sd.root_id, COALESCE(SUM(d.n), 0)::bigint AS total
            FROM suite_desc sd
            LEFT JOIN direct d ON d.suite_id = sd.descendant_id
            GROUP BY sd.root_id
        )
        SELECT
            s.id,
            s.project_id,
            s.parent_id,
            s.name,
            s.sort_order,
            s.created_at,
            s.updated_at,
            COALESCE(t.total, 0) AS case_count,
            COALESCE(d.n, 0) AS direct_case_count
        FROM suites s
        LEFT JOIN totals t ON t.root_id = s.id
        LEFT JOIN direct d ON d.suite_id = s.id
        WHERE s.project_id = $1
        ORDER BY s.sort_order, s.name
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(suites)
}

pub async fn get_suite_with_counts(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    suite_id: Uuid,
) -> AppResult<Option<Suite>> {
    let suites = list_suites_with_counts(pool, project_id).await?;
    Ok(suites.into_iter().find(|s| s.id == suite_id))
}

/// Empty counts for a freshly inserted/updated row when a full recount is deferred.
pub fn zero_counts(
    id: Uuid,
    project_id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    sort_order: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> Suite {
    Suite {
        id,
        project_id,
        parent_id,
        name,
        sort_order,
        created_at,
        updated_at,
        case_count: 0,
        direct_case_count: 0,
    }
}
