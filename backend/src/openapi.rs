use utoipa::OpenApi;

use crate::models::{
    CreateCase, CreateProject, CreateStep, CreateSuite, Project, Suite, TestCase, TestCaseDetail,
    TestStep, UpdateCase, UpdateProject, UpdateSuite,
};
use crate::routes::health::{HealthResponse, ReadyResponse};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Vulyk API",
        version = "0.1.0",
        description = "Open-source test management system (beehive). Phase 1 foundation.",
        license(name = "AGPL-3.0")
    ),
    paths(
        crate::routes::health::health,
        crate::routes::health::ready,
        crate::routes::projects::list_projects,
        crate::routes::projects::create_project,
        crate::routes::projects::get_project,
        crate::routes::projects::update_project,
        crate::routes::projects::delete_project,
        crate::routes::suites::list_suites,
        crate::routes::suites::create_suite,
        crate::routes::suites::get_suite,
        crate::routes::suites::update_suite,
        crate::routes::suites::delete_suite,
        crate::routes::cases::list_cases,
        crate::routes::cases::create_case,
        crate::routes::cases::get_case,
        crate::routes::cases::update_case,
        crate::routes::cases::delete_case,
    ),
    components(schemas(
        HealthResponse,
        ReadyResponse,
        Project,
        CreateProject,
        UpdateProject,
        Suite,
        CreateSuite,
        UpdateSuite,
        TestCase,
        TestStep,
        TestCaseDetail,
        CreateCase,
        CreateStep,
        UpdateCase,
    )),
    tags(
        (name = "system", description = "Health and readiness"),
        (name = "projects", description = "Projects"),
        (name = "suites", description = "Suites / sections"),
        (name = "cases", description = "Test cases and steps"),
    )
)]
pub struct ApiDoc;
