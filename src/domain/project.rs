use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub tasks_dir: String,
    pub folder_pattern: String,
    pub config_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Role {
    pub name: String,
    pub color: Option<String>,
    pub sort: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Status {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub color: Option<String>,
    pub sort: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProjectView {
    pub project: Project,
    pub roles: Vec<Role>,
    pub statuses: Vec<Status>,
}
