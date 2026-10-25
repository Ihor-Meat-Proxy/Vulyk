//! Project roles and permission matrix.
//!
//! Roles (exactly five): Admin, Test Architect, Test Automation Engineer,
//! Developer, Viewer. Capabilities are permission-based (not a strict ladder),
//! except Admin implies every project permission and system admins bypass checks.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Per-project role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(type_name = "project_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ProjectRole {
    Admin,
    TestArchitect,
    TestAutomationEngineer,
    Developer,
    Viewer,
}

impl ProjectRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::TestArchitect => "test_architect",
            Self::TestAutomationEngineer => "test_automation_engineer",
            Self::Developer => "developer",
            Self::Viewer => "viewer",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Admin => "Admin",
            Self::TestArchitect => "Test Architect",
            Self::TestAutomationEngineer => "Test Automation Engineer",
            Self::Developer => "Developer",
            Self::Viewer => "Viewer",
        }
    }

    pub fn all() -> &'static [ProjectRole] {
        &[
            Self::Admin,
            Self::TestArchitect,
            Self::TestAutomationEngineer,
            Self::Developer,
            Self::Viewer,
        ]
    }

    /// Whether this role grants `permission` on a project.
    pub fn allows(self, permission: Permission) -> bool {
        if self == Self::Admin {
            return !permission.is_system_only();
        }
        self.permissions().contains(&permission)
    }

    pub fn permissions(self) -> &'static [Permission] {
        use Permission::*;
        match self {
            Self::Admin => &[
                ViewProject,
                UpdateProject,
                DeleteProject,
                ManageMemberships,
                ManageSuites,
                CreateEditCases,
                DeleteCases,
                ManageRequirements,
                ManagePlans,
                ImportCases,
                ViewReports,
                ExportReports,
                ExecuteRuns,
                UploadResults,
                ManageAttachments,
                Comment,
                TriggerCi,
                ManageCiConfig,
                LlmAssist,
                ManageLlmConfig,
                ManageApiTokens,
            ],
            Self::TestArchitect => &[
                ViewProject,
                UpdateProject,
                ManageSuites,
                CreateEditCases,
                DeleteCases,
                ManageRequirements,
                ManagePlans,
                ImportCases,
                ViewReports,
                ExportReports,
                ExecuteRuns,
                UploadResults,
                ManageAttachments,
                Comment,
                LlmAssist,
                ManageLlmConfig,
            ],
            Self::TestAutomationEngineer => &[
                ViewProject,
                ManageSuites,
                CreateEditCases,
                ViewReports,
                ExportReports,
                ExecuteRuns,
                UploadResults,
                ManageAttachments,
                Comment,
                TriggerCi,
                ManageCiConfig,
                LlmAssist,
                ManageApiTokens,
            ],
            Self::Developer => &[
                ViewProject,
                ViewReports,
                ExportReports,
                ExecuteRuns,
                UploadResults,
                ManageAttachments,
                Comment,
                TriggerCi,
            ],
            Self::Viewer => &[ViewProject, ViewReports, ExportReports],
        }
    }
}

impl std::fmt::Display for ProjectRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ProjectRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "test_architect" => Ok(Self::TestArchitect),
            "test_automation_engineer" => Ok(Self::TestAutomationEngineer),
            "developer" => Ok(Self::Developer),
            "viewer" => Ok(Self::Viewer),
            // Legacy aliases (pre-migration API clients)
            "project_lead" => Ok(Self::TestArchitect),
            "tester" => Ok(Self::TestAutomationEngineer),
            other => Err(format!("unknown project role: {other}")),
        }
    }
}

/// Fine-grained capabilities checked by the API and UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // System-only (is_system_admin)
    ManageUsers,
    ManageSystemSettings,
    ManageSso,
    ManageGlobalIntegrations,

    // Project
    ViewProject,
    UpdateProject,
    DeleteProject,
    ManageMemberships,
    ManageSuites,
    CreateEditCases,
    DeleteCases,
    ManageRequirements,
    ManagePlans,
    ImportCases,
    ViewReports,
    ExportReports,
    ExecuteRuns,
    UploadResults,
    ManageAttachments,
    Comment,
    TriggerCi,
    ManageCiConfig,
    LlmAssist,
    ManageLlmConfig,
    ManageApiTokens,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ManageUsers => "manage_users",
            Self::ManageSystemSettings => "manage_system_settings",
            Self::ManageSso => "manage_sso",
            Self::ManageGlobalIntegrations => "manage_global_integrations",
            Self::ViewProject => "view_project",
            Self::UpdateProject => "update_project",
            Self::DeleteProject => "delete_project",
            Self::ManageMemberships => "manage_memberships",
            Self::ManageSuites => "manage_suites",
            Self::CreateEditCases => "create_edit_cases",
            Self::DeleteCases => "delete_cases",
            Self::ManageRequirements => "manage_requirements",
            Self::ManagePlans => "manage_plans",
            Self::ImportCases => "import_cases",
            Self::ViewReports => "view_reports",
            Self::ExportReports => "export_reports",
            Self::ExecuteRuns => "execute_runs",
            Self::UploadResults => "upload_results",
            Self::ManageAttachments => "manage_attachments",
            Self::Comment => "comment",
            Self::TriggerCi => "trigger_ci",
            Self::ManageCiConfig => "manage_ci_config",
            Self::LlmAssist => "llm_assist",
            Self::ManageLlmConfig => "manage_llm_config",
            Self::ManageApiTokens => "manage_api_tokens",
        }
    }

    pub fn is_system_only(self) -> bool {
        matches!(
            self,
            Self::ManageUsers
                | Self::ManageSystemSettings
                | Self::ManageSso
                | Self::ManageGlobalIntegrations
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RoleRequirement {
    AnyMember,
    Permission(Permission),
    SystemAdmin,
}

/// Static matrix rows for docs / OpenAPI / admin UI.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RoleMatrixRow {
    pub role: ProjectRole,
    pub display_name: String,
    pub permissions: Vec<&'static str>,
}

pub fn role_matrix() -> Vec<RoleMatrixRow> {
    ProjectRole::all()
        .iter()
        .copied()
        .map(|role| RoleMatrixRow {
            role,
            display_name: role.display_name().to_string(),
            permissions: role.permissions().iter().map(|p| p.as_str()).collect(),
        })
        .collect()
}
