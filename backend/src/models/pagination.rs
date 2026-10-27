use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Hard ceiling for list/export page sizes (review #15).
pub const PAGE_MAX_LIMIT: i64 = 500;
/// Default page size when `limit` is omitted.
pub const PAGE_DEFAULT_LIMIT: i64 = 50;

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PageQuery {
    /// Max rows to return (default 50, max 500).
    pub limit: Option<i64>,
    /// Rows to skip (default 0).
    pub offset: Option<i64>,
}

impl PageQuery {
    pub fn limit(&self) -> i64 {
        self.limit
            .unwrap_or(PAGE_DEFAULT_LIMIT)
            .clamp(1, PAGE_MAX_LIMIT)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_query_clamps_to_max() {
        let q = PageQuery {
            limit: Some(10_000),
            offset: Some(-5),
        };
        assert_eq!(q.limit(), PAGE_MAX_LIMIT);
        assert_eq!(q.offset(), 0);
    }

    #[test]
    fn page_query_defaults() {
        let q = PageQuery {
            limit: None,
            offset: None,
        };
        assert_eq!(q.limit(), PAGE_DEFAULT_LIMIT);
        assert_eq!(q.offset(), 0);
    }
}
