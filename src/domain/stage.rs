use schemars::JsonSchema;
use serde::Serialize;

use crate::domain::{Report, RoleInput};

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Stage {
    pub id: String,
    pub task_id: String,
    pub role: String,
    pub round: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub end_reason: Option<String>,
    pub verdict: String,
    pub seconds: Option<i64>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StageStart {
    pub stage: Stage,
    pub inputs: Vec<RoleInput>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StageFinish {
    pub stage: Stage,
    pub report: Report,
}
