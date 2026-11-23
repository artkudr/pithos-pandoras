//! Домен: типы, инварианты и ошибки. Без I/O и без зависимостей от axum/rmcp.

pub mod comment;
pub mod error;
pub mod event;
pub mod file;
pub mod input;
pub mod project;
pub mod report;
pub mod stage;
pub mod task;

pub use comment::Comment;
pub use error::{AppError, ErrorCode};
pub use event::Event;
pub use file::{FileAdded, FileRecord};
pub use input::{InputRule, ProjectConfig};
pub use project::{Project, ProjectView, Role, Status};
pub use report::{
    DraftBlock, DraftRevision, Report, ReportDraft, ReportDraftDocument, ReportDraftPreview,
    ReportPreview, ReportPublished, RoleInput,
};
pub use stage::{Stage, StageFinish, StageStart, StageStatus};
pub use task::{
    NextStep, RoleTime, StatusCount, Task, TaskCard, TaskCreated, TaskListItem, TaskStats,
};
