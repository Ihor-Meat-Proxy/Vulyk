//! W8: run summaries, trends, requirement coverage, flaky detection, export.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::AppResult;
use crate::models::{
    ExportQuery, FlakyQuery, FlakyReport, PassFailTrends, ProjectDashboard,
    RequirementCoverageReport, RunSummaryReport, TrendsQuery,
};
use crate::routes::AppState;
use crate::services::cases::ensure_project;
use crate::services::reports;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/reports/dashboard",
            get(project_dashboard),
        )
        .route(
            "/api/v1/projects/{project_id}/reports/runs/{run_id}/summary",
            get(run_summary),
        )
        .route("/api/v1/projects/{project_id}/reports/trends", get(trends))
        .route(
            "/api/v1/projects/{project_id}/reports/coverage",
            get(coverage),
        )
        .route("/api/v1/projects/{project_id}/reports/flaky", get(flaky))
        .route(
            "/api/v1/projects/{project_id}/reports/export",
            get(export_report),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/reports/dashboard",
    tag = "reports",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "Per-project dashboard", body = ProjectDashboard)),
    security(("bearer_auth" = []))
)]
pub async fn project_dashboard(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<ProjectDashboard>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewReports,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(
        reports::project_dashboard(&state.pool, project_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/reports/runs/{run_id}/summary",
    tag = "reports",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("run_id" = Uuid, Path, description = "Run id"),
    ),
    responses((status = 200, description = "Run summary", body = RunSummaryReport)),
    security(("bearer_auth" = []))
)]
pub async fn run_summary(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<RunSummaryReport>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewReports,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(
        reports::run_summary(&state.pool, project_id, run_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/reports/trends",
    tag = "reports",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("granularity" = Option<String>, Query, description = "run or day"),
        ("limit" = Option<i64>, Query, description = "Max points"),
    ),
    responses((status = 200, description = "Pass/fail trends", body = PassFailTrends)),
    security(("bearer_auth" = []))
)]
pub async fn trends(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(q): Query<TrendsQuery>,
) -> AppResult<Json<PassFailTrends>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewReports,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let granularity = q.granularity.as_deref().unwrap_or("run");
    let limit = q.limit.unwrap_or(20);
    Ok(Json(
        reports::pass_fail_trends(&state.pool, project_id, granularity, limit).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/reports/coverage",
    tag = "reports",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "Requirement coverage", body = RequirementCoverageReport)),
    security(("bearer_auth" = []))
)]
pub async fn coverage(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<RequirementCoverageReport>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewReports,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(
        reports::requirement_coverage(&state.pool, project_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/reports/flaky",
    tag = "reports",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("lookback_runs" = Option<i64>, Query, description = "Recent runs to inspect"),
        ("limit" = Option<i64>, Query, description = "Max flaky cases"),
    ),
    responses((status = 200, description = "Flaky cases", body = FlakyReport)),
    security(("bearer_auth" = []))
)]
pub async fn flaky(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(q): Query<FlakyQuery>,
) -> AppResult<Json<FlakyReport>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewReports,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(
        reports::flaky_cases(
            &state.pool,
            project_id,
            q.lookback_runs.unwrap_or(20),
            q.limit.unwrap_or(50),
        )
        .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/reports/export",
    tag = "reports",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("format" = Option<String>, Query, description = "csv or pdf"),
        ("report" = Option<String>, Query, description = "dashboard|coverage|trends|flaky|run"),
        ("run_id" = Option<Uuid>, Query, description = "Required when report=run"),
    ),
    responses(
        (status = 200, description = "Exported report file"),
        (status = 400, description = "Bad request")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_report(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ExportQuery>,
) -> AppResult<impl IntoResponse> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ExportReports,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let format = q.format.as_deref().unwrap_or("csv").to_ascii_lowercase();
    let report = q
        .report
        .as_deref()
        .unwrap_or("dashboard")
        .to_ascii_lowercase();

    let (filename, content_type, body) = match (format.as_str(), report.as_str()) {
        ("csv", "dashboard") => {
            let dash = reports::project_dashboard(&state.pool, project_id).await?;
            (
                "vulyk-dashboard.csv",
                "text/csv; charset=utf-8",
                reports::dashboard_to_csv(&dash).into_bytes(),
            )
        }
        ("csv", "coverage") => {
            let cov = reports::requirement_coverage(&state.pool, project_id).await?;
            (
                "vulyk-coverage.csv",
                "text/csv; charset=utf-8",
                reports::coverage_to_csv(&cov).into_bytes(),
            )
        }
        ("csv", "trends") => {
            let trends = reports::pass_fail_trends(&state.pool, project_id, "run", 50).await?;
            (
                "vulyk-trends.csv",
                "text/csv; charset=utf-8",
                reports::trends_to_csv(&trends).into_bytes(),
            )
        }
        ("csv", "flaky") => {
            let flaky = reports::flaky_cases(&state.pool, project_id, 20, 100).await?;
            (
                "vulyk-flaky.csv",
                "text/csv; charset=utf-8",
                reports::flaky_to_csv(&flaky).into_bytes(),
            )
        }
        ("csv", "run") => {
            let run_id = q
                .run_id
                .ok_or_else(|| crate::error::AppError::BadRequest("run_id required".into()))?;
            let summary = reports::run_summary(&state.pool, project_id, run_id).await?;
            (
                "vulyk-run-summary.csv",
                "text/csv; charset=utf-8",
                reports::run_summary_to_csv(&summary).into_bytes(),
            )
        }
        ("pdf", _) => {
            let dash = reports::project_dashboard(&state.pool, project_id).await?;
            let lines = reports::dashboard_pdf_lines(&dash);
            (
                "vulyk-report.pdf",
                "application/pdf",
                reports::text_report_to_pdf("Vulyk project report", &lines),
            )
        }
        _ => {
            return Err(crate::error::AppError::BadRequest(format!(
                "unsupported export format={format} report={report}"
            )));
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type).unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).unwrap(),
    );
    Ok((StatusCode::OK, headers, body))
}
