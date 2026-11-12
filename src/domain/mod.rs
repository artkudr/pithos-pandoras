//! Домен: типы, инварианты и ошибки. Без I/O и без зависимостей от axum/rmcp.

pub mod comment;
pub mod error;
pub mod event;
pub mod file;
pub mod input;
pub mod report;
pub mod stage;
pub mod task;
pub mod workspace;

pub use comment::Comment;
pub use error::{AppError, ErrorCode};
pub use event::Event;
pub use file::{FileAdded, FileRecord};
pub use input::{InputRule, WorkspaceConfig};
pub use report::{Report, ReportPreview, RoleInput};
pub use stage::{Stage, StageFinish, StageStart};
pub use task::{
    NextStep, RoleTime, StatusCount, Task, TaskCard, TaskCreated, TaskListItem, TaskStats,
};
pub use workspace::{Role, Status, Workspace, WorkspaceView};
