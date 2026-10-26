//! Server-side test-case PDF (and bulk ZIP) export.
//!
//! Uses genpdf with embedded DejaVu fonts so Ukrainian/Cyrillic and other
//! Unicode text render without a headless browser or system font packages.

use std::io::{Cursor, Write};

use chrono::{DateTime, Utc};
use genpdf::elements::{self, Break, LinearLayout, Paragraph, TableLayout};
use genpdf::fonts::{FontData, FontFamily};
use genpdf::style::{Color, Style};
use genpdf::{Alignment, Element as _, SimplePageDecorator};
use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::error::{AppError, AppResult};
use crate::models::{CustomFieldValue, TestCaseDetail, TestStep};
use crate::services::cases::load_case_detail;
use crate::settings;

const FONT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-Bold.ttf");

const DEFAULT_RESULTS: i64 = 5;
const MAX_RESULTS: i64 = 50;
const COMMENT_MAX: usize = 160;

/// Supported UI locales for PDF field labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportLocale {
    En,
    Uk,
    Fr,
    Es,
    De,
}

impl ExportLocale {
    pub fn parse(raw: &str) -> Self {
        let primary = raw
            .split([',', ';'])
            .next()
            .unwrap_or("en")
            .trim()
            .to_ascii_lowercase();
        let lang = primary.split('-').next().unwrap_or(primary.as_str()).trim();
        match lang {
            "uk" | "ua" => Self::Uk,
            "fr" => Self::Fr,
            "es" => Self::Es,
            "de" => Self::De,
            _ => Self::En,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Uk => "uk",
            Self::Fr => "fr",
            Self::Es => "es",
            Self::De => "de",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PdfLabels {
    pub project: &'static str,
    pub case_key: &'static str,
    pub title: &'static str,
    pub folder: &'static str,
    pub priority: &'static str,
    pub status: &'static str,
    pub version: &'static str,
    pub tags: &'static str,
    pub automated: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
    pub custom_fields: &'static str,
    pub preconditions: &'static str,
    pub description: &'static str,
    pub steps: &'static str,
    pub step: &'static str,
    pub action: &'static str,
    pub expected: &'static str,
    pub linked_requirements: &'static str,
    pub latest_results: &'static str,
    pub col_when: &'static str,
    pub col_run: &'static str,
    pub col_type: &'static str,
    pub col_status: &'static str,
    pub col_by: &'static str,
    pub col_env: &'static str,
    pub col_comment: &'static str,
    pub col_link: &'static str,
    pub type_manual: &'static str,
    pub type_automated: &'static str,
    pub none: &'static str,
    pub unfiled: &'static str,
    pub export_date: &'static str,
    pub vulyk_version: &'static str,
    pub page: &'static str,
    pub of: &'static str,
}

impl PdfLabels {
    pub fn for_locale(locale: ExportLocale) -> Self {
        match locale {
            ExportLocale::En => Self::en(),
            ExportLocale::Uk => Self::uk(),
            ExportLocale::Fr => Self::fr(),
            ExportLocale::Es => Self::es(),
            ExportLocale::De => Self::de(),
        }
    }

    fn en() -> Self {
        Self {
            project: "Project",
            case_key: "Case key",
            title: "Title",
            folder: "Folder",
            priority: "Priority",
            status: "Status",
            version: "Version",
            tags: "Tags",
            automated: "Automated",
            yes: "Yes",
            no: "No",
            custom_fields: "Custom fields",
            preconditions: "Preconditions",
            description: "Description",
            steps: "Steps",
            step: "#",
            action: "Action",
            expected: "Expected",
            linked_requirements: "Linked requirements",
            latest_results: "Latest results",
            col_when: "Date / time",
            col_run: "Run",
            col_type: "Type",
            col_status: "Status",
            col_by: "Executed by / pipeline",
            col_env: "Environment / build / commit",
            col_comment: "Comment / failure",
            col_link: "Link",
            type_manual: "Manual",
            type_automated: "Automated",
            none: "—",
            unfiled: "Unfiled",
            export_date: "Exported",
            vulyk_version: "Vulyk",
            page: "Page",
            of: "of",
        }
    }

    fn uk() -> Self {
        Self {
            project: "Проєкт",
            case_key: "Ключ кейсу",
            title: "Назва",
            folder: "Тека",
            priority: "Пріоритет",
            status: "Статус",
            version: "Версія",
            tags: "Теги",
            automated: "Автоматизований",
            yes: "Так",
            no: "Ні",
            custom_fields: "Користувацькі поля",
            preconditions: "Передумови",
            description: "Опис",
            steps: "Кроки",
            step: "№",
            action: "Дія",
            expected: "Очікуваний результат",
            linked_requirements: "Пов'язані вимоги",
            latest_results: "Останні результати",
            col_when: "Дата / час",
            col_run: "Прогін",
            col_type: "Тип",
            col_status: "Статус",
            col_by: "Виконавець / пайплайн",
            col_env: "Середовище / збірка / коміт",
            col_comment: "Коментар / помилка",
            col_link: "Посилання",
            type_manual: "Ручний",
            type_automated: "Автоматизований",
            none: "—",
            unfiled: "Без теки",
            export_date: "Експортовано",
            vulyk_version: "Vulyk",
            page: "Сторінка",
            of: "з",
        }
    }

    fn fr() -> Self {
        Self {
            project: "Projet",
            case_key: "Clé du cas",
            title: "Titre",
            folder: "Dossier",
            priority: "Priorité",
            status: "Statut",
            version: "Version",
            tags: "Tags",
            automated: "Automatisé",
            yes: "Oui",
            no: "Non",
            custom_fields: "Champs personnalisés",
            preconditions: "Préconditions",
            description: "Description",
            steps: "Étapes",
            step: "N°",
            action: "Action",
            expected: "Résultat attendu",
            linked_requirements: "Exigences liées",
            latest_results: "Derniers résultats",
            col_when: "Date / heure",
            col_run: "Exécution",
            col_type: "Type",
            col_status: "Statut",
            col_by: "Exécuté par / pipeline",
            col_env: "Environnement / build / commit",
            col_comment: "Commentaire / échec",
            col_link: "Lien",
            type_manual: "Manuel",
            type_automated: "Automatisé",
            none: "—",
            unfiled: "Sans dossier",
            export_date: "Exporté",
            vulyk_version: "Vulyk",
            page: "Page",
            of: "sur",
        }
    }

    fn es() -> Self {
        Self {
            project: "Proyecto",
            case_key: "Clave del caso",
            title: "Título",
            folder: "Carpeta",
            priority: "Prioridad",
            status: "Estado",
            version: "Versión",
            tags: "Etiquetas",
            automated: "Automatizado",
            yes: "Sí",
            no: "No",
            custom_fields: "Campos personalizados",
            preconditions: "Precondiciones",
            description: "Descripción",
            steps: "Pasos",
            step: "N.º",
            action: "Acción",
            expected: "Resultado esperado",
            linked_requirements: "Requisitos vinculados",
            latest_results: "Últimos resultados",
            col_when: "Fecha / hora",
            col_run: "Ejecución",
            col_type: "Tipo",
            col_status: "Estado",
            col_by: "Ejecutado por / pipeline",
            col_env: "Entorno / build / commit",
            col_comment: "Comentario / fallo",
            col_link: "Enlace",
            type_manual: "Manual",
            type_automated: "Automatizado",
            none: "—",
            unfiled: "Sin carpeta",
            export_date: "Exportado",
            vulyk_version: "Vulyk",
            page: "Página",
            of: "de",
        }
    }

    fn de() -> Self {
        Self {
            project: "Projekt",
            case_key: "Fallschlüssel",
            title: "Titel",
            folder: "Ordner",
            priority: "Priorität",
            status: "Status",
            version: "Version",
            tags: "Tags",
            automated: "Automatisiert",
            yes: "Ja",
            no: "Nein",
            custom_fields: "Benutzerfelder",
            preconditions: "Vorbedingungen",
            description: "Beschreibung",
            steps: "Schritte",
            step: "Nr.",
            action: "Aktion",
            expected: "Erwartet",
            linked_requirements: "Verknüpfte Anforderungen",
            latest_results: "Neueste Ergebnisse",
            col_when: "Datum / Zeit",
            col_run: "Lauf",
            col_type: "Typ",
            col_status: "Status",
            col_by: "Ausgeführt von / Pipeline",
            col_env: "Umgebung / Build / Commit",
            col_comment: "Kommentar / Fehler",
            col_link: "Link",
            type_manual: "Manuell",
            type_automated: "Automatisiert",
            none: "—",
            unfiled: "Ohne Ordner",
            export_date: "Exportiert",
            vulyk_version: "Vulyk",
            page: "Seite",
            of: "von",
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ExportQuery {
    /// Number of latest mixed results to include (default 5).
    pub results: Option<i64>,
    /// UI locale for labels: en, uk, fr, es, de.
    pub locale: Option<String>,
}

impl ExportQuery {
    pub fn results_limit(&self) -> i64 {
        self.results
            .unwrap_or(DEFAULT_RESULTS)
            .clamp(1, MAX_RESULTS)
    }

    pub fn locale(&self, accept_language: Option<&str>) -> ExportLocale {
        if let Some(raw) = self.locale.as_deref().filter(|s| !s.is_empty()) {
            return ExportLocale::parse(raw);
        }
        if let Some(raw) = accept_language.filter(|s| !s.is_empty()) {
            return ExportLocale::parse(raw);
        }
        ExportLocale::En
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BulkExportRequest {
    pub case_ids: Vec<Uuid>,
    #[serde(default)]
    pub results: Option<i64>,
    #[serde(default)]
    pub locale: Option<String>,
}

impl BulkExportRequest {
    pub fn results_limit(&self) -> i64 {
        self.results
            .unwrap_or(DEFAULT_RESULTS)
            .clamp(1, MAX_RESULTS)
    }

    pub fn locale(&self, accept_language: Option<&str>) -> ExportLocale {
        ExportQuery {
            results: self.results,
            locale: self.locale.clone(),
        }
        .locale(accept_language)
    }
}

#[derive(Debug, Clone)]
pub struct ExportResultRow {
    pub executed_at: DateTime<Utc>,
    pub run_name: String,
    pub run_id: Uuid,
    pub result_type: &'static str, // "manual" | "automated"
    pub status: String,
    pub executed_by: String,
    pub environment: String,
    pub comment: String,
    pub link: String,
}

#[derive(Debug, Clone)]
pub struct LinkedRequirement {
    pub external_key: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct CaseExportData {
    pub project_name: String,
    pub project_key: String,
    pub detail: TestCaseDetail,
    pub folder_path: String,
    pub automated: bool,
    pub requirements: Vec<LinkedRequirement>,
    pub results: Vec<ExportResultRow>,
    pub results_limit: i64,
    pub base_url: String,
    pub exported_at: DateTime<Utc>,
    pub vulyk_version: String,
    pub locale: ExportLocale,
}

/// Plain-text view used by unit tests and the PDF `%VULYK_EXPORT` manifest.
pub fn content_manifest(data: &CaseExportData) -> String {
    let labels = PdfLabels::for_locale(data.locale);
    let tags = data
        .detail
        .tags
        .iter()
        .map(|t| t.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut lines = vec![
        format!(
            "{}: {} ({})",
            labels.project, data.project_name, data.project_key
        ),
        format!("{}: {}", labels.case_key, data.detail.case.key),
        format!("{}: {}", labels.title, data.detail.case.title),
        format!("{}: {}", labels.folder, data.folder_path),
        format!("{}: {}", labels.priority, data.detail.case.priority),
        format!("{}: {}", labels.status, data.detail.case.status),
        format!("{}: {}", labels.version, data.detail.case.version),
        format!(
            "{}: {}",
            labels.tags,
            if tags.is_empty() { labels.none } else { &tags }
        ),
        format!(
            "{}: {}",
            labels.automated,
            if data.automated {
                labels.yes
            } else {
                labels.no
            }
        ),
        format!(
            "{}: {}",
            labels.preconditions, data.detail.case.precondition
        ),
        format!("{}: {}", labels.description, data.detail.case.description),
    ];
    for cf in &data.detail.custom_fields {
        let val = format_custom_value(cf);
        if !val.is_empty() {
            lines.push(format!("{}: {val}", cf.name));
        }
    }
    for (i, step) in data.detail.steps.iter().enumerate() {
        lines.push(format!(
            "{} {}: {} | {}",
            labels.steps,
            i + 1,
            step.action,
            step.expected_result
        ));
    }
    for req in &data.requirements {
        let key = req.external_key.as_deref().unwrap_or("");
        lines.push(format!(
            "{}: {} {}",
            labels.linked_requirements, key, req.title
        ));
    }
    for row in &data.results {
        lines.push(format!(
            "RESULT\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            row.executed_at.to_rfc3339(),
            row.run_name,
            row.result_type,
            row.status,
            row.executed_by,
            row.environment,
            row.comment
        ));
    }
    lines.join("\n")
}

fn format_custom_value(cf: &CustomFieldValue) -> String {
    match &cf.value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}

/// Colour-blind-safe status marker: distinct shape glyph + label (never colour alone).
pub fn status_marker(status: &str) -> String {
    let s = status.trim();
    let (glyph, label) = match s.to_ascii_lowercase().as_str() {
        "passed" => ("✓", "Passed"),
        "failed" => ("✗", "Failed"),
        "blocked" => ("⊘", "Blocked"),
        "skipped" => ("▸", "Skipped"),
        "retest" => ("↻", "Retest"),
        "untested" => ("○", "Untested"),
        "ready" => ("●", "Ready"),
        "running" => ("▶", "Running"),
        "untestable" => ("△", "Untestable"),
        _ => ("•", s),
    };
    format!("{glyph} {label}")
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn font_family() -> AppResult<FontFamily<FontData>> {
    let regular = FontData::new(FONT_REGULAR.to_vec(), None)
        .map_err(|e| AppError::Internal(format!("load DejaVuSans: {e}")))?;
    let bold = FontData::new(FONT_BOLD.to_vec(), None)
        .map_err(|e| AppError::Internal(format!("load DejaVuSans-Bold: {e}")))?;
    let italic = FontData::new(FONT_REGULAR.to_vec(), None)
        .map_err(|e| AppError::Internal(format!("load DejaVuSans italic: {e}")))?;
    let bold_italic = FontData::new(FONT_BOLD.to_vec(), None)
        .map_err(|e| AppError::Internal(format!("load DejaVuSans bold italic: {e}")))?;
    Ok(FontFamily {
        regular,
        bold,
        italic,
        bold_italic,
    })
}

fn heading(text: &str, size: u8) -> elements::StyledElement<Paragraph> {
    Paragraph::new(text.to_string())
        .aligned(Alignment::Left)
        .styled(Style::new().bold().with_font_size(size))
}

fn kv(label: &str, value: &str) -> Paragraph {
    let mut p = Paragraph::default();
    p.push(genpdf::style::StyledString::new(
        format!("{label}: "),
        Style::new().bold().with_font_size(9),
    ));
    p.push(genpdf::style::StyledString::new(
        value.to_string(),
        Style::new().with_font_size(9),
    ));
    p
}

fn body_para(text: &str) -> elements::StyledElement<Paragraph> {
    Paragraph::new(text.to_string()).styled(Style::new().with_font_size(9))
}

/// Render one case export to PDF bytes (Unicode-capable).
pub fn render_case_pdf(data: &CaseExportData) -> AppResult<Vec<u8>> {
    let labels = PdfLabels::for_locale(data.locale);
    let family = font_family()?;
    let mut doc = genpdf::Document::new(family);
    doc.set_title(format!(
        "{} — {}",
        data.detail.case.key, data.detail.case.title
    ));
    doc.set_minimal_conformance();
    doc.set_line_spacing(1.15);

    let export_stamp = data.exported_at.format("%Y-%m-%d %H:%M UTC").to_string();
    let version = data.vulyk_version.clone();
    let footer_left = format!(
        "{}: {} · {} {}",
        labels.export_date, export_stamp, labels.vulyk_version, version
    );
    let page_label = labels.page.to_string();

    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(12);
    decorator.set_header(move |page| {
        let mut layout = LinearLayout::vertical();
        layout.push(
            Paragraph::new(format!("{footer_left}  ·  {page_label} {page}"))
                .aligned(Alignment::Right)
                .styled(
                    Style::new()
                        .with_font_size(8)
                        .with_color(Color::Rgb(90, 90, 90)),
                ),
        );
        layout.push(Break::new(0.6));
        layout
    });
    doc.set_page_decorator(decorator);

    doc.push(heading(
        &format!("{} ({})", data.project_name, data.project_key),
        14,
    ));
    doc.push(Break::new(0.4));
    doc.push(heading(
        &format!("{} {}", data.detail.case.key, data.detail.case.title),
        12,
    ));
    doc.push(Break::new(0.8));

    doc.push(kv(labels.folder, &data.folder_path));
    doc.push(kv(labels.priority, &data.detail.case.priority));
    doc.push(kv(labels.status, &data.detail.case.status));
    doc.push(kv(labels.version, &data.detail.case.version.to_string()));
    let tags = data
        .detail
        .tags
        .iter()
        .map(|t| t.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    doc.push(kv(
        labels.tags,
        if tags.is_empty() { labels.none } else { &tags },
    ));
    doc.push(kv(
        labels.automated,
        if data.automated {
            labels.yes
        } else {
            labels.no
        },
    ));

    let custom_vals: Vec<_> = data
        .detail
        .custom_fields
        .iter()
        .filter_map(|cf| {
            let v = format_custom_value(cf);
            if v.is_empty() {
                None
            } else {
                Some((cf.name.clone(), v))
            }
        })
        .collect();
    if !custom_vals.is_empty() {
        doc.push(Break::new(0.6));
        doc.push(heading(labels.custom_fields, 11));
        for (name, val) in custom_vals {
            doc.push(kv(&name, &val));
        }
    }

    doc.push(Break::new(0.6));
    doc.push(heading(labels.preconditions, 11));
    doc.push(body_para(if data.detail.case.precondition.is_empty() {
        labels.none
    } else {
        &data.detail.case.precondition
    }));

    doc.push(Break::new(0.4));
    doc.push(heading(labels.description, 11));
    doc.push(body_para(if data.detail.case.description.is_empty() {
        labels.none
    } else {
        &data.detail.case.description
    }));

    doc.push(Break::new(0.6));
    doc.push(heading(labels.steps, 11));
    doc.push(Break::new(0.3));
    if data.detail.steps.is_empty() {
        doc.push(body_para(labels.none));
    } else {
        push_steps_table(&mut doc, labels, &data.detail.steps);
    }

    doc.push(Break::new(0.6));
    doc.push(heading(labels.linked_requirements, 11));
    if data.requirements.is_empty() {
        doc.push(body_para(labels.none));
    } else {
        for req in &data.requirements {
            let line = match &req.external_key {
                Some(k) if !k.is_empty() => format!("{k} — {}", req.title),
                _ => req.title.clone(),
            };
            doc.push(body_para(&format!("• {line}")));
        }
    }

    doc.push(Break::new(0.6));
    doc.push(heading(
        &format!("{} ({})", labels.latest_results, data.results_limit),
        11,
    ));
    doc.push(Break::new(0.3));
    if data.results.is_empty() {
        doc.push(body_para(labels.none));
    } else {
        push_results_table(&mut doc, labels, data);
    }

    let mut bytes = Vec::new();
    doc.render(&mut bytes)
        .map_err(|e| AppError::Internal(format!("PDF render: {e}")))?;
    inject_manifest_comment(&mut bytes, &content_manifest(data));
    Ok(bytes)
}

fn push_steps_table(doc: &mut genpdf::Document, labels: PdfLabels, steps: &[TestStep]) {
    let mut table = TableLayout::new(vec![1, 5, 5]);
    table.set_cell_decorator(elements::FrameCellDecorator::new(true, true, false));
    let header_style = Style::new().bold().with_font_size(8);
    table
        .row()
        .element(Paragraph::new(labels.step).styled(header_style))
        .element(Paragraph::new(labels.action).styled(header_style))
        .element(Paragraph::new(labels.expected).styled(header_style))
        .push()
        .expect("steps header");

    let cell = Style::new().with_font_size(8);
    for (i, step) in steps.iter().enumerate() {
        table
            .row()
            .element(Paragraph::new(format!("{}", i + 1)).styled(cell))
            .element(Paragraph::new(step.action.clone()).styled(cell))
            .element(Paragraph::new(step.expected_result.clone()).styled(cell))
            .push()
            .expect("step row");
    }
    doc.push(table);
}

fn push_results_table(doc: &mut genpdf::Document, labels: PdfLabels, data: &CaseExportData) {
    let mut table = TableLayout::new(vec![3, 3, 2, 2, 3, 3, 3]);
    table.set_cell_decorator(elements::FrameCellDecorator::new(true, true, false));
    let header_style = Style::new().bold().with_font_size(7);
    table
        .row()
        .element(Paragraph::new(labels.col_when).styled(header_style))
        .element(Paragraph::new(labels.col_run).styled(header_style))
        .element(Paragraph::new(labels.col_type).styled(header_style))
        .element(Paragraph::new(labels.col_status).styled(header_style))
        .element(Paragraph::new(labels.col_by).styled(header_style))
        .element(Paragraph::new(labels.col_env).styled(header_style))
        .element(Paragraph::new(labels.col_comment).styled(header_style))
        .push()
        .expect("results header");

    let cell = Style::new().with_font_size(7);
    for row_data in &data.results {
        let type_label = if row_data.result_type == "manual" {
            labels.type_manual
        } else {
            labels.type_automated
        };
        table
            .row()
            .element(
                Paragraph::new(row_data.executed_at.format("%Y-%m-%d %H:%M").to_string())
                    .styled(cell),
            )
            .element(Paragraph::new(truncate(&row_data.run_name, 40)).styled(cell))
            .element(Paragraph::new(type_label.to_string()).styled(cell))
            .element(Paragraph::new(status_marker(&row_data.status)).styled(cell))
            .element(Paragraph::new(truncate(&row_data.executed_by, 36)).styled(cell))
            .element(Paragraph::new(truncate(&row_data.environment, 36)).styled(cell))
            .element(Paragraph::new(truncate(&row_data.comment, COMMENT_MAX)).styled(cell))
            .push()
            .expect("result row");
    }
    doc.push(table);

    // Links as a compact list (URLs wrap poorly in narrow table cells).
    doc.push(Break::new(0.4));
    doc.push(heading(labels.col_link, 9));
    for row_data in &data.results {
        doc.push(body_para(&format!(
            "• {} — {}",
            row_data.run_name, row_data.link
        )));
    }
}

/// Insert a UTF-8 PDF comment after the header so tests can assert key/title/Cyrillic.
fn inject_manifest_comment(pdf: &mut Vec<u8>, manifest: &str) {
    let needle = b"\n";
    let insert_at = pdf
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + 1)
        .unwrap_or(0);
    let mut comment = String::from("%VULYK_EXPORT ");
    for c in manifest.chars() {
        if c == '\n' || c == '\r' {
            comment.push(' ');
        } else {
            comment.push(c);
        }
    }
    comment.push('\n');
    let bytes = comment.into_bytes();
    pdf.splice(insert_at..insert_at, bytes);
}

pub async fn resolve_case_id_or_key(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    id_or_key: &str,
) -> AppResult<Uuid> {
    if let Ok(id) = Uuid::parse_str(id_or_key) {
        let found: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM test_cases WHERE project_id = $1 AND id = $2")
                .bind(project_id)
                .bind(id)
                .fetch_optional(pool)
                .await?;
        if let Some((id,)) = found {
            return Ok(id);
        }
    }
    let found: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM test_cases WHERE project_id = $1 AND key = $2")
            .bind(project_id)
            .bind(id_or_key)
            .fetch_optional(pool)
            .await?;
    found
        .map(|(id,)| id)
        .ok_or_else(|| AppError::NotFound(format!("case {id_or_key}")))
}

pub async fn resolve_case_globally(
    pool: &sqlx::PgPool,
    id_or_key: &str,
) -> AppResult<(Uuid, Uuid)> {
    if let Ok(id) = Uuid::parse_str(id_or_key) {
        let found: Option<(Uuid, Uuid)> =
            sqlx::query_as("SELECT id, project_id FROM test_cases WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        if let Some(pair) = found {
            return Ok(pair);
        }
    }
    let found: Option<(Uuid, Uuid)> =
        sqlx::query_as("SELECT id, project_id FROM test_cases WHERE key = $1")
            .bind(id_or_key)
            .fetch_optional(pool)
            .await?;
    found.ok_or_else(|| AppError::NotFound(format!("case {id_or_key}")))
}

async fn folder_path(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    suite_id: Option<Uuid>,
    unfiled_label: &str,
) -> AppResult<String> {
    let Some(mut current) = suite_id else {
        return Ok(unfiled_label.to_string());
    };
    let mut parts = Vec::new();
    for _ in 0..64 {
        let row: Option<(Option<Uuid>, String)> =
            sqlx::query_as("SELECT parent_id, name FROM suites WHERE project_id = $1 AND id = $2")
                .bind(project_id)
                .bind(current)
                .fetch_optional(pool)
                .await?;
        let Some((parent, name)) = row else {
            break;
        };
        parts.push(name);
        match parent {
            Some(p) => current = p,
            None => break,
        }
    }
    parts.reverse();
    if parts.is_empty() {
        Ok(unfiled_label.to_string())
    } else {
        Ok(parts.join(" / "))
    }
}

async fn load_requirements(
    pool: &sqlx::PgPool,
    case_id: Uuid,
) -> AppResult<Vec<LinkedRequirement>> {
    let rows = sqlx::query(
        r#"SELECT r.external_key, r.title
           FROM requirements r
           JOIN case_requirements cr ON cr.requirement_id = r.id
           WHERE cr.case_id = $1
           ORDER BY r.external_key NULLS LAST, r.title"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| LinkedRequirement {
            external_key: r.try_get("external_key").ok(),
            title: r.get("title"),
        })
        .collect())
}

fn json_str_field(v: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = v
            .get(*key)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(s.to_string());
        }
    }
    None
}

fn format_environment(env: &Value, meta: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(name) = json_str_field(env, &["name", "environment", "env"]) {
        parts.push(name);
    } else if let Some(s) = env.as_str().filter(|s| !s.is_empty()) {
        parts.push(s.to_string());
    }
    if let Some(build) = json_str_field(env, &["build", "build_id", "buildId"])
        .or_else(|| json_str_field(meta, &["build", "build_id", "buildId"]))
    {
        parts.push(format!("build:{build}"));
    }
    if let Some(commit) = json_str_field(env, &["commit", "commit_sha", "sha", "revision"])
        .or_else(|| json_str_field(meta, &["commit", "commit_sha", "sha", "revision"]))
    {
        let short = if commit.len() > 8 {
            commit[..8].to_string()
        } else {
            commit
        };
        parts.push(format!("commit:{short}"));
    }
    parts.join(" · ")
}

async fn load_export_results(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    case_id: Uuid,
    limit: i64,
    base_url: &str,
    project_key: &str,
) -> AppResult<Vec<ExportResultRow>> {
    let rows = sqlx::query(
        r#"SELECT tr.id AS result_id, tr.status, tr.message, tr.source, tr.executed_at,
                  tr.run_id, r.name AS run_name, r.source AS run_source,
                  r.environment AS run_environment, r.meta AS run_meta,
                  r.external_ref, r.created_by_name,
                  COALESCE(u.display_name, r.created_by_name) AS executed_by,
                  i.name AS iteration_name, c.name AS campaign_name
           FROM test_results tr
           JOIN test_runs r ON r.id = tr.run_id
           LEFT JOIN iterations i ON i.id = r.iteration_id
           LEFT JOIN campaigns c ON c.id = i.campaign_id
           LEFT JOIN users u ON u.id = tr.executed_by
           WHERE r.project_id = $1 AND tr.case_id = $2
           ORDER BY tr.executed_at DESC, tr.created_at DESC
           LIMIT $3"#,
    )
    .bind(project_id)
    .bind(case_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let base = base_url.trim_end_matches('/');
    Ok(rows
        .into_iter()
        .map(|row| {
            let source: String = row.get("source");
            let run_source: String = row.get("run_source");
            let result_type = if source == "manual"
                || source == "sample"
                || run_source == "manual"
                || run_source == "sample"
            {
                "manual"
            } else {
                "automated"
            };
            let run_name: String = row.get("run_name");
            let campaign: Option<String> = row.try_get("campaign_name").ok();
            let iteration: Option<String> = row.try_get("iteration_name").ok();
            let display_run = match (campaign, iteration) {
                (Some(c), Some(i)) => format!("{c} > {i} / {run_name}"),
                (Some(c), None) => format!("{c} / {run_name}"),
                _ => run_name.clone(),
            };
            let executed_by: Option<String> = row.try_get("executed_by").ok();
            let external_ref: Option<String> = row.try_get("external_ref").ok();
            let by = match (executed_by.filter(|s| !s.is_empty()), external_ref) {
                (Some(name), Some(pipe)) => format!("{name} / {pipe}"),
                (Some(name), None) => name,
                (None, Some(pipe)) => pipe,
                (None, None) => String::new(),
            };
            let env: Value = row.try_get("run_environment").unwrap_or(Value::Null);
            let meta: Value = row.try_get("run_meta").unwrap_or(Value::Null);
            let run_id: Uuid = row.get("run_id");
            let link = if base.is_empty() {
                format!("/runs/{run_id}")
            } else {
                format!("{base}/runs/{run_id}?project={project_key}")
            };
            let message: String = row.try_get("message").unwrap_or_default();
            ExportResultRow {
                executed_at: row.get("executed_at"),
                run_name: display_run,
                run_id,
                result_type,
                status: row.get("status"),
                executed_by: by,
                environment: format_environment(&env, &meta),
                comment: truncate(&message, COMMENT_MAX),
                link,
            }
        })
        .collect())
}

pub async fn load_case_export(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    case_id: Uuid,
    results_limit: i64,
    locale: ExportLocale,
) -> AppResult<CaseExportData> {
    let project: (String, String) = sqlx::query_as("SELECT name, key FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("project".into()))?;

    let case = sqlx::query_as::<_, crate::models::TestCase>(
        r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                  priority, status, version, external_key, sort_order, created_at, updated_at
           FROM test_cases WHERE id = $1 AND project_id = $2"#,
    )
    .bind(case_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("case".into()))?;

    let labels = PdfLabels::for_locale(locale);
    let folder = folder_path(pool, project_id, case.suite_id, labels.unfiled).await?;
    let automated = case.external_key.as_ref().is_some_and(|k| !k.is_empty());
    let detail = load_case_detail(pool, case).await?;
    let requirements = load_requirements(pool, case_id).await?;

    let settings = settings::load_settings(pool).await.unwrap_or_default();
    let base_url = settings.base_url;
    let results = load_export_results(
        pool,
        project_id,
        case_id,
        results_limit,
        &base_url,
        &project.1,
    )
    .await?;

    Ok(CaseExportData {
        project_name: project.0,
        project_key: project.1,
        detail,
        folder_path: folder,
        automated,
        requirements,
        results,
        results_limit,
        base_url,
        exported_at: Utc::now(),
        vulyk_version: env!("CARGO_PKG_VERSION").to_string(),
        locale,
    })
}

pub async fn export_case_pdf_bytes(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    case_id: Uuid,
    results_limit: i64,
    locale: ExportLocale,
) -> AppResult<(String, Vec<u8>)> {
    let data = load_case_export(pool, project_id, case_id, results_limit, locale).await?;
    let filename = format!("{}.pdf", data.detail.case.key);
    let bytes = render_case_pdf(&data)?;
    Ok((filename, bytes))
}

pub async fn export_cases_bulk(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    case_ids: &[Uuid],
    results_limit: i64,
    locale: ExportLocale,
) -> AppResult<(String, &'static str, Vec<u8>)> {
    if case_ids.is_empty() {
        return Err(AppError::BadRequest("case_ids required".into()));
    }
    if case_ids.len() == 1 {
        let (filename, bytes) =
            export_case_pdf_bytes(pool, project_id, case_ids[0], results_limit, locale).await?;
        return Ok((filename, "application/pdf", bytes));
    }

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut used_names = std::collections::HashSet::new();
        for id in case_ids {
            let (filename, bytes) =
                export_case_pdf_bytes(pool, project_id, *id, results_limit, locale).await?;
            let mut name = filename;
            if !used_names.insert(name.clone()) {
                name = format!("{}_{}", id, name);
                used_names.insert(name.clone());
            }
            zip.start_file(&name, opts)
                .map_err(|e| AppError::Internal(format!("zip start: {e}")))?;
            zip.write_all(&bytes)
                .map_err(|e| AppError::Internal(format!("zip write: {e}")))?;
        }
        zip.finish()
            .map_err(|e| AppError::Internal(format!("zip finish: {e}")))?;
    }
    Ok((
        "vulyk-cases-export.zip".into(),
        "application/zip",
        buffer.into_inner(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Tag, TestCase, TestStep};
    use chrono::TimeZone;

    fn sample_data(title: &str, locale: ExportLocale) -> CaseExportData {
        let case = TestCase {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            suite_id: None,
            key: "VQA-2194".into(),
            title: title.into(),
            description: "Опис українською".into(),
            precondition: "Передумова".into(),
            priority: "high".into(),
            status: "ready".into(),
            version: 3,
            external_key: Some("e2e:export.spec.ts".into()),
            sort_order: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        CaseExportData {
            project_name: "Vulyk QA".into(),
            project_key: "VULYK_QA".into(),
            detail: TestCaseDetail {
                case,
                steps: vec![TestStep {
                    id: Uuid::nil(),
                    case_id: Uuid::nil(),
                    position: 0,
                    action: "Відкрити сторінку експорту".into(),
                    expected_result: "PDF завантажується".into(),
                    data: String::new(),
                }],
                tags: vec![Tag {
                    id: Uuid::nil(),
                    project_id: Uuid::nil(),
                    name: "smoke".into(),
                    color: "#0f6b4c".into(),
                    created_at: Utc::now(),
                }],
                custom_fields: vec![],
            },
            folder_path: "01. Setup / Export".into(),
            automated: true,
            requirements: vec![LinkedRequirement {
                external_key: Some("REQ-1".into()),
                title: "Export PDF".into(),
            }],
            results: vec![
                ExportResultRow {
                    executed_at: Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0).unwrap(),
                    run_name: "CI nightly".into(),
                    run_id: Uuid::nil(),
                    result_type: "automated",
                    status: "Failed".into(),
                    executed_by: "github-actions".into(),
                    environment: "build:42 · commit:abc12345".into(),
                    comment: "timeout".into(),
                    link: "/runs/00000000-0000-0000-0000-000000000000".into(),
                },
                ExportResultRow {
                    executed_at: Utc.with_ymd_and_hms(2026, 10, 25, 9, 30, 0).unwrap(),
                    run_name: "Manual smoke".into(),
                    run_id: Uuid::nil(),
                    result_type: "manual",
                    status: "Passed".into(),
                    executed_by: "Alice".into(),
                    environment: String::new(),
                    comment: String::new(),
                    link: "/runs/00000000-0000-0000-0000-000000000000".into(),
                },
            ],
            results_limit: 5,
            base_url: String::new(),
            exported_at: Utc::now(),
            vulyk_version: "0.1.0".into(),
            locale,
        }
    }

    #[test]
    fn locale_parse_variants() {
        assert_eq!(ExportLocale::parse("uk-UA"), ExportLocale::Uk);
        assert_eq!(ExportLocale::parse("fr"), ExportLocale::Fr);
        assert_eq!(ExportLocale::parse("es-ES,es;q=0.9"), ExportLocale::Es);
        assert_eq!(ExportLocale::parse("de"), ExportLocale::De);
        assert_eq!(ExportLocale::parse("en-US"), ExportLocale::En);
    }

    #[test]
    fn status_markers_are_distinct() {
        assert!(status_marker("Passed").starts_with('✓'));
        assert!(status_marker("Failed").starts_with('✗'));
        assert!(status_marker("Blocked").starts_with('⊘'));
        assert_ne!(status_marker("Passed"), status_marker("Failed"));
    }

    #[test]
    fn manifest_contains_cyrillic_and_mixed_results_order() {
        let data = sample_data("Експорт кейсу PDF", ExportLocale::Uk);
        let text = content_manifest(&data);
        assert!(text.contains("VQA-2194"));
        assert!(text.contains("Експорт кейсу PDF"));
        assert!(text.contains("Передумова"));
        assert!(text.contains("Автоматизований"));
        let auto = text
            .find("RESULT\t2026-10-26")
            .expect("newer automated first");
        let manual = text
            .find("RESULT\t2026-10-25")
            .expect("older manual second");
        assert!(auto < manual);
        assert!(text.contains("automated"));
        assert!(text.contains("manual"));
    }

    #[test]
    fn pdf_renders_with_cyrillic_and_embed_manifest() {
        let data = sample_data("Експорт кейсу PDF", ExportLocale::Uk);
        let pdf = render_case_pdf(&data).expect("pdf");
        assert!(pdf.starts_with(b"%PDF"));
        let as_str = String::from_utf8_lossy(&pdf);
        assert!(as_str.contains("%VULYK_EXPORT"));
        assert!(as_str.contains("VQA-2194"));
        assert!(as_str.contains("Експорт кейсу PDF"));
        assert!(as_str.contains("Передумова"));
    }

    #[test]
    fn results_limit_clamped() {
        let q = ExportQuery {
            results: Some(999),
            locale: None,
        };
        assert_eq!(q.results_limit(), 50);
        let q2 = ExportQuery {
            results: Some(0),
            locale: None,
        };
        assert_eq!(q2.results_limit(), 1);
        let q3 = ExportQuery {
            results: None,
            locale: None,
        };
        assert_eq!(q3.results_limit(), 5);
    }
}
