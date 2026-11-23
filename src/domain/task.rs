use schemars::JsonSchema;
use serde::Serialize;

use crate::domain::{FileRecord, ReportDraftPreview, ReportPreview, Stage, Status};

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub number: i64,
    pub folder_name: String,
    pub title: String,
    pub statement: String,
    pub status_id: String,
    pub route: Option<String>,
    pub route_snapshot: Option<String>,
    pub resource: Option<String>,
    pub waiting_for: String,
    pub waiting_ref: String,
    pub waiting_since: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub origin: String,
    /// Только в ответе `task_block`: был ли закрыт открытый этап.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_closed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TaskCreated {
    pub task: Task,
    pub folder_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TaskListItem {
    pub id: String,
    pub number: i64,
    pub folder_name: String,
    pub title: String,
    pub status_id: String,
    pub status_title: String,
    pub current_role: Option<String>,
    pub current_role_color: Option<String>,
    pub waiting_for: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TaskCard {
    pub task: Task,
    /// Полный путь к папке задачи: `root_path + tasks_dir + folder_name`.
    pub folder_path: String,
    pub status: Option<Status>,
    pub current_stage: Option<Stage>,
    pub reports: Vec<ReportPreview>,
    pub drafts: Vec<ReportDraftPreview>,
    pub files: Vec<FileRecord>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NextStep {
    pub action: String,
    pub role: Option<String>,
    pub round: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StatusCount {
    pub status_id: String,
    pub title: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RoleTime {
    pub role: String,
    pub seconds: i64,
    pub stages: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TaskStats {
    pub by_status: Vec<StatusCount>,
    pub by_role: Vec<RoleTime>,
    pub open_stages: i64,
    pub waiting: i64,
    pub blocked: i64,
}
