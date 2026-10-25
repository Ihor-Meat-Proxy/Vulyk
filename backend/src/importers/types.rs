use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::FieldMapping;

#[derive(Debug, Clone)]
pub struct ImportInput {
    pub format: String,
    pub content: String,
    pub field_mapping: FieldMapping,
    pub options: Value,
}

#[derive(Debug, Clone)]
pub struct ImportContext {
    pub pool: PgPool,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImporterDescriptor {
    pub id: String,
    pub name: String,
    pub supported_formats: Vec<String>,
}
