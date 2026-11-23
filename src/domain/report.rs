use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Report {
    pub report_id: String,
    pub task_id: String,
    pub stage_id: Option<String>,
    pub actor: Option<String>,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReportPreview {
    pub report_id: String,
    pub task_id: String,
    pub stage_id: Option<String>,
    pub actor: Option<String>,
    pub kind: String,
    pub title: String,
    pub round: Option<i64>,
    pub preview: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RoleInput {
    pub role: String,
    pub round: i64,
    pub kind: String,
    pub report_id: String,
    pub title: String,
    pub preview: String,
}

/// Черновик отчёта: ещё не отчёт, но уже виден в карточке.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReportDraft {
    pub draft_id: String,
    pub task_id: String,
    pub stage_id: String,
    pub actor: String,
    pub kind: String,
    pub title: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_report_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DraftBlock {
    pub key: String,
    pub title: String,
    pub body: String,
    pub sort: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReportDraftDocument {
    pub draft: ReportDraft,
    pub blocks: Vec<DraftBlock>,
    /// Тот Markdown, который уйдёт в отчёт при публикации.
    pub rendered: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReportDraftPreview {
    pub draft_id: String,
    pub task_id: String,
    pub stage_id: String,
    pub actor: String,
    pub kind: String,
    pub title: String,
    pub revision: i64,
    pub preview: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DraftRevision {
    pub draft_id: String,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReportPublished {
    pub draft_id: String,
    pub report_id: String,
    pub report: Report,
}

pub fn preview(content: &str) -> String {
    let one_line = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    for ch in one_line.chars().take(240) {
        out.push(ch);
    }
    if one_line.chars().count() > 240 {
        out.push('…');
    }
    out
}
