use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Comment {
    pub id: String,
    pub task_id: String,
    pub author: String,
    pub body: String,
    pub created_at: String,
}
