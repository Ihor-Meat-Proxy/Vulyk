use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Per-project role (W1). Hierarchy: admin > project_lead > tester > viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(type_name = "project_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ProjectRole {
    Admin,
    ProjectLead,
    Tester,
    Viewer,
}

impl ProjectRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::ProjectLead => "project_lead",
            Self::Tester => "tester",
            Self::Viewer => "viewer",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Viewer => 1,
            Self::Tester => 2,
            Self::ProjectLead => 3,
            Self::Admin => 4,
        }
    }

    pub fn at_least(self, required: ProjectRole) -> bool {
        self.rank() >= required.rank()
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
            "project_lead" => Ok(Self::ProjectLead),
            "tester" => Ok(Self::Tester),
            "viewer" => Ok(Self::Viewer),
            other => Err(format!("unknown project role: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RoleRequirement {
    /// Any authenticated membership (or system admin).
    AnyMember,
    /// Role must be at least this level.
    AtLeast(ProjectRole),
    /// System admin only.
    SystemAdmin,
}
