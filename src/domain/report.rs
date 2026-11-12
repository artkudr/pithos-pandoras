use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Report {
    pub id: String,
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
    pub id: String,
    pub task_id: String,
    pub stage_id: Option<String>,
    pub actor: Option<String>,
    pub kind: String,
    pub title: String,
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
