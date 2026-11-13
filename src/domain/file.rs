use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileRecord {
    pub id: String,
    pub task_id: String,
    pub stage_id: Option<String>,
    pub report_id: Option<String>,
    pub actor: String,
    pub kind: String,
    pub title: String,
    pub comment: String,
    pub path: String,
    pub mime: Option<String>,
    pub size: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileAdded {
    pub file_id: String,
    pub url: String,
}
