//! Pluggable test-case importers (Xray, Zephyr, Squash, TestLink, TestRail, …).

pub mod csv_generic;
pub mod kiwi;
pub mod mapping;
pub mod persist;
pub mod qase;
pub mod qtest;
pub mod squash;
pub mod testlink;
pub mod testrail;
pub mod types;
pub mod xray;
pub mod zephyr;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::OnceLock;

pub use types::*;

use crate::error::{AppError, AppResult};
use crate::models::{FieldMapping, ImportPreview, ImportReport};

#[async_trait]
pub trait Importer: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &str;
    fn supported_formats(&self) -> &[&str];
    async fn preview(&self, input: ImportInput) -> AppResult<ImportPreview>;
    async fn run(&self, ctx: ImportContext, input: ImportInput) -> AppResult<ImportReport>;
}

fn registry() -> &'static HashMap<&'static str, &'static dyn Importer> {
    static REGISTRY: OnceLock<HashMap<&'static str, &'static dyn Importer>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut m: HashMap<&'static str, &'static dyn Importer> = HashMap::new();
        let importers: &[&'static dyn Importer] = &[
            &csv_generic::GenericCsvImporter,
            &xray::XrayImporter,
            &zephyr::ZephyrImporter,
            &squash::SquashImporter,
            &testlink::TestLinkImporter,
            &testrail::TestRailImporter,
            &qase::QaseImporter,
            &qtest::QtestImporter,
            &kiwi::KiwiImporter,
        ];
        for imp in importers {
            m.insert(imp.id(), *imp);
        }
        m
    })
}

pub fn get(id: &str) -> AppResult<&'static dyn Importer> {
    registry()
        .get(id)
        .copied()
        .ok_or_else(|| AppError::BadRequest(format!("unknown importer: {id}")))
}

pub fn list() -> Vec<ImporterDescriptor> {
    let mut items: Vec<_> = registry()
        .values()
        .map(|i| ImporterDescriptor {
            id: i.id().to_string(),
            name: i.name().to_string(),
            supported_formats: i
                .supported_formats()
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        })
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

pub fn default_mapping_for(importer_id: &str) -> FieldMapping {
    match importer_id {
        "csv" => FieldMapping::default(),
        "xray" => FieldMapping {
            title: "summary".into(),
            key: "key".into(),
            description: "description".into(),
            precondition: "precondition".into(),
            priority: "priority".into(),
            status: "status".into(),
            suite: "labels".into(),
            steps: "steps".into(),
        },
        "zephyr" => FieldMapping {
            title: "name".into(),
            key: "key".into(),
            description: "objective".into(),
            precondition: "precondition".into(),
            priority: "priority".into(),
            status: "status".into(),
            suite: "folder".into(),
            steps: "testScript".into(),
        },
        "testrail" => FieldMapping {
            title: "title".into(),
            key: "id".into(),
            description: "custom_steps".into(),
            precondition: "custom_preconds".into(),
            priority: "priority_id".into(),
            status: "status".into(),
            suite: "section".into(),
            steps: "custom_steps_separated".into(),
        },
        _ => FieldMapping::default(),
    }
}
