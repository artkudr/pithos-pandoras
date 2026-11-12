use serde::Serialize;

/// Код ошибки приложения. Это часть контракта с агентом.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    WorkspaceMissing,
    WorkspaceUnknown,
    RoleMissing,
    RoleUnknown,
    TaskMissing,
    TaskUnknown,
    FieldMissing,
    InputMissing,
    Conflict,
    NotFound,
    Io,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceMissing => "workspace_missing",
            Self::WorkspaceUnknown => "workspace_unknown",
            Self::RoleMissing => "role_missing",
            Self::RoleUnknown => "role_unknown",
            Self::TaskMissing => "task_missing",
            Self::TaskUnknown => "task_unknown",
            Self::FieldMissing => "field_missing",
            Self::InputMissing => "input_missing",
            Self::Conflict => "conflict",
            Self::NotFound => "not_found",
            Self::Io => "io",
        }
    }
}

/// Ошибка в формате `{code, message, hint, allowed}`.
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Vec<String>>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
            allowed: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_allowed(mut self, allowed: Vec<String>) -> Self {
        self.allowed = Some(allowed);
        self
    }

    pub fn workspace_missing() -> Self {
        Self::new(
            ErrorCode::WorkspaceMissing,
            "Не указано рабочее пространство",
        )
        .with_hint("Возьмите имя из блока `## Сервис задач` в `AGENTS.md` проекта.")
    }

    pub fn workspace_unknown(name: &str, allowed: Vec<String>) -> Self {
        Self::new(
            ErrorCode::WorkspaceUnknown,
            format!("Рабочее пространство `{name}` не найдено"),
        )
        .with_allowed(allowed)
    }

    pub fn role_missing() -> Self {
        Self::new(ErrorCode::RoleMissing, "Не указана роль").with_hint(
            "Роль — имя зарегистрированного агента, например `Коммандер` или `Исполнитель`.",
        )
    }

    pub fn role_unknown(name: &str, allowed: Vec<String>) -> Self {
        Self::new(
            ErrorCode::RoleUnknown,
            format!("Роль `{name}` не зарегистрирована"),
        )
        .with_allowed(allowed)
    }

    pub fn task_missing() -> Self {
        Self::new(ErrorCode::TaskMissing, "Не указана задача")
            .with_hint("Укажите номер или идентификатор задачи. Значения по умолчанию нет.")
    }

    pub fn task_unknown(task: &str) -> Self {
        Self::new(
            ErrorCode::TaskUnknown,
            format!("Задача `{task}` не найдена в пространстве"),
        )
    }

    pub fn field_missing(field: &str) -> Self {
        Self::new(
            ErrorCode::FieldMissing,
            format!("Не заполнено обязательное поле `{field}`"),
        )
    }

    pub fn input_missing(role: &str, kind: &str, round: &str) -> Self {
        Self::new(
            ErrorCode::InputMissing,
            format!("У роли `{role}` нет отчёта `{kind}` за круг `{round}`"),
        )
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Io, message)
    }
}
