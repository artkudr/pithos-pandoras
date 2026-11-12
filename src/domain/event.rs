use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Event {
    pub id: i64,
    pub uuid: String,
    pub task_id: String,
    pub seq: i64,
    pub at: String,
    pub kind: String,
    pub stage_id: Option<String>,
    pub actor: Option<String>,
    pub payload: String,
    pub origin: String,
}
