use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PageQuery {
    /// Max rows to return (default 50, max 500).
    pub limit: Option<i64>,
    /// Rows to skip (default 0).
    pub offset: Option<i64>,
}

impl PageQuery {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(50).clamp(1, 500)
    }

    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Page<T: Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl<T: Serialize> Page<T> {
    pub fn new(items: Vec<T>, total: i64, limit: i64, offset: i64) -> Self {
        Self {
            items,
            total,
            limit,
            offset,
        }
    }
}
