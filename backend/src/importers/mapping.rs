use serde_json::Value;

use crate::models::{FieldMapping, ImportCaseDraft, ImportStepDraft};

pub fn get_mapped_owned(row: &Value, mapping_key: &str) -> Option<String> {
    if mapping_key.is_empty() {
        return None;
    }
    let v = if mapping_key.contains('.') {
        row.pointer(&format!("/{}", mapping_key.replace('.', "/")))
    } else {
        row.get(mapping_key)
    }?;
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(_) | Value::Object(_) => Some(v.to_string()),
        Value::Null => None,
    }
}

pub fn draft_from_row(
    row: &Value,
    mapping: &FieldMapping,
    defaults: DraftDefaults,
) -> ImportCaseDraft {
    let title = get_mapped_owned(row, &mapping.title)
        .filter(|s| !s.is_empty())
        .or(defaults.title)
        .unwrap_or_else(|| "Untitled case".into());
    let key = get_mapped_owned(row, &mapping.key).filter(|s| !s.is_empty());
    let description = get_mapped_owned(row, &mapping.description).unwrap_or_default();
    let precondition = get_mapped_owned(row, &mapping.precondition).unwrap_or_default();
    let priority = get_mapped_owned(row, &mapping.priority)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| defaults.priority.unwrap_or_else(|| "medium".into()));
    let status = get_mapped_owned(row, &mapping.status)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| defaults.status.unwrap_or_else(|| "draft".into()));
    let suite_name = get_mapped_owned(row, &mapping.suite).filter(|s| !s.is_empty());
    let steps = parse_steps_field(row, &mapping.steps);

    ImportCaseDraft {
        key,
        title,
        description,
        precondition,
        priority,
        status,
        suite_name,
        steps,
        external_id: defaults.external_id,
        source_fields: row.clone(),
    }
}

#[derive(Default)]
pub struct DraftDefaults {
    pub title: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub external_id: Option<String>,
}

fn parse_steps_field(row: &Value, mapping_key: &str) -> Vec<ImportStepDraft> {
    if mapping_key.is_empty() {
        return Vec::new();
    }
    let Some(v) = (if mapping_key.contains('.') {
        row.pointer(&format!("/{}", mapping_key.replace('.', "/")))
    } else {
        row.get(mapping_key)
    }) else {
        return Vec::new();
    };

    match v {
        Value::Array(arr) => arr
            .iter()
            .map(|step| {
                let action = step
                    .get("action")
                    .or_else(|| step.get("step"))
                    .or_else(|| step.get("description"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let expected_result = step
                    .get("expected_result")
                    .or_else(|| step.get("expectedResult"))
                    .or_else(|| step.get("expected"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let data = step
                    .get("data")
                    .or_else(|| step.get("testData"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                ImportStepDraft {
                    action,
                    expected_result,
                    data,
                }
            })
            .filter(|s| !s.action.is_empty())
            .collect(),
        Value::String(s) => s
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| ImportStepDraft {
                action: l.to_string(),
                expected_result: String::new(),
                data: String::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub fn normalize_priority(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "1" | "critical" | "highest" | "p1" => "critical".into(),
        "2" | "high" | "p2" => "high".into(),
        "3" | "medium" | "normal" | "p3" => "medium".into(),
        "4" | "low" | "p4" | "lowest" => "low".into(),
        other if !other.is_empty() => other.to_string(),
        _ => "medium".into(),
    }
}
