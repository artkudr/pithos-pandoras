use schemars::JsonSchema;
use serde::Serialize;

use crate::domain::{Report, ReportDraftPreview, ReportPreview, RoleInput};

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
    pub last_activity_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StageStart {
    pub stage: Stage,
    pub inputs: Vec<RoleInput>,
    /// `true`, если этап уже был открыт этой же ролью и возвращён как есть.
    pub resumed: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StageFinish {
    pub stage: Stage,
    pub report: Report,
}

/// Статус этапа: открытый, а если его нет — последний завершённый.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StageStatus {
    pub stage: Option<Stage>,
    pub is_open: bool,
    pub inputs: Vec<RoleInput>,
    pub reports: Vec<ReportPreview>,
    pub drafts: Vec<ReportDraftPreview>,
}
