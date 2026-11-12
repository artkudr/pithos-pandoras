use serde::Deserialize;
use std::collections::HashMap;

/// Настройки проекта из `projects.config_json`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub inputs: HashMap<String, Vec<InputRule>>,
}

/// Правило входа роли.
#[derive(Debug, Clone, Deserialize)]
pub struct InputRule {
    pub from: String,
    pub kind: String,
    pub round: String,
    #[serde(default)]
    pub required: bool,
}
