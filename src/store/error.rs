use crate::domain::AppError;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("input missing: {role}/{kind}/{round}")]
    InputMissing {
        role: String,
        kind: String,
        round: String,
    },
    #[error("{0}")]
    App(Box<AppError>),
}

impl From<AppError> for StoreError {
    fn from(err: AppError) -> Self {
        Self::App(Box::new(err))
    }
}

impl From<StoreError> for AppError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::Sqlite(rusqlite::Error::SqliteFailure(ffi_err, message))
                if ffi_err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                AppError::conflict(message.unwrap_or_else(|| "нарушение ограничения".to_string()))
            }
            StoreError::Sqlite(err) => AppError::io(err.to_string()),
            StoreError::Io(err) => AppError::io(err.to_string()),
            StoreError::Json(err) => AppError::io(err.to_string()),
            StoreError::NotFound => AppError::not_found("Запись не найдена"),
            StoreError::Conflict(message) => AppError::conflict(message),
            StoreError::Invalid(message) => AppError::io(message),
            StoreError::InputMissing { role, kind, round } => {
                AppError::input_missing(&role, &kind, &round)
            }
            StoreError::App(err) => *err,
        }
    }
}
