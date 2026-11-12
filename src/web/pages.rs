use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use rust_embed::RustEmbed;

use crate::domain::{AppError, ErrorCode};
use crate::service::Service;

#[derive(Clone)]
struct AppState {
    service: Arc<Service>,
    workspace: String,
}

#[derive(RustEmbed)]
#[folder = "src/web/static/"]
struct Assets;

pub fn router(service: Arc<Service>, workspace: String) -> axum::Router {
    let state = AppState { service, workspace };
    axum::Router::new()
        .route("/", get(board))
        .route("/tasks/{number}", get(task_page))
        .route("/files/{file_id}", get(file))
        .route("/static/{*path}", get(static_file))
        .with_state(state)
}

async fn board(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let workspace = state.workspace.clone();
    let tasks = state.service.task_list(Some(workspace.clone())).await?;
    let stats = state.service.task_stats(Some(workspace.clone())).await?;

    let template = BoardTemplate {
        workspace,
        tasks: tasks
            .into_iter()
            .map(|task| BoardTaskView {
                number: task.number,
                title: task.title,
                status: task.status_title,
                current_role: task.current_role.unwrap_or_else(|| String::from("—")),
                waiting_for: if task.waiting_for.is_empty() {
                    String::from("—")
                } else {
                    task.waiting_for
                },
                updated_at: task.updated_at,
            })
            .collect(),
        counts: stats
            .by_status
            .into_iter()
            .map(|count| StatusCountView {
                title: count.title,
                count: count.count,
            })
            .collect(),
    };

    render(template)
}

async fn task_page(
    State(state): State<AppState>,
    Path(number): Path<i64>,
) -> Result<Html<String>, WebError> {
    let data = state
        .service
        .task_page(Some(state.workspace.clone()), Some(number.to_string()))
        .await?;

    let template = TaskTemplate {
        workspace: state.workspace.clone(),
        task: TaskView {
            number: data.task.number,
            title: data.task.title,
            statement: data.task.statement,
            status: data.status.map(|status| status.title).unwrap_or_default(),
            folder_name: data.task.folder_name,
            created_at: data.task.created_at,
            updated_at: data.task.updated_at,
        },
        current_stage: data.current_stage.map(|stage| StageView {
            role: stage.role,
            round: stage.round,
            started_at: stage.started_at,
        }),
        reports: data
            .reports
            .into_iter()
            .map(|item| ReportView {
                kind: item.report.kind,
                title: item.report.title,
                role: item.role,
                created_at: item.report.created_at,
                html: render_markdown(&item.report.content),
            })
            .collect(),
        files: data
            .files
            .into_iter()
            .map(|file| FileView {
                id: file.id,
                title: file.title,
                kind: file.kind,
                size: file.size.map(format_size).unwrap_or_default(),
                created_at: file.created_at,
            })
            .collect(),
        comments: data
            .comments
            .into_iter()
            .map(|comment| CommentView {
                author: comment.author,
                body: comment.body,
                created_at: comment.created_at,
            })
            .collect(),
        events: data
            .events
            .into_iter()
            .map(|event| EventView {
                kind: event.kind,
                actor: event.actor.unwrap_or_else(|| String::from("—")),
                at: event.at,
            })
            .collect(),
        warnings: data.warnings,
    };

    render(template)
}

async fn file(
    State(state): State<AppState>,
    Path(file_id): Path<String>,
) -> Result<Response, WebError> {
    let file = state
        .service
        .file_for_workspace(&state.workspace, &file_id)
        .await?;
    let bytes = tokio::fs::read(&file.path)
        .await
        .map_err(|err| WebError::internal(format!("файл {}: {err}", file.path)))?;
    let mime = file.mime.unwrap_or_else(|| {
        mime_guess::from_path(&file.path)
            .first_or_octet_stream()
            .to_string()
    });
    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}

async fn static_file(Path(path): Path<String>) -> Response {
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.to_string())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn render<T: Template>(template: T) -> Result<Html<String>, WebError> {
    template
        .render()
        .map(Html)
        .map_err(|err| WebError::internal(format!("шаблон: {err}")))
}

fn render_markdown(text: &str) -> String {
    let parser = pulldown_cmark::Parser::new(text);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    ammonia::Builder::default()
        .url_relative(ammonia::UrlRelative::PassThrough)
        .clean(&html)
        .to_string()
}

fn format_size(size: i64) -> String {
    if size < 1024 {
        format!("{size} Б")
    } else if size < 1024 * 1024 {
        format!("{:.1} КБ", size as f64 / 1024.0)
    } else {
        format!("{:.1} МБ", size as f64 / (1024.0 * 1024.0))
    }
}

#[derive(Template)]
#[template(path = "board.html")]
struct BoardTemplate {
    workspace: String,
    tasks: Vec<BoardTaskView>,
    counts: Vec<StatusCountView>,
}

#[derive(Template)]
#[template(path = "task.html")]
struct TaskTemplate {
    workspace: String,
    task: TaskView,
    current_stage: Option<StageView>,
    reports: Vec<ReportView>,
    files: Vec<FileView>,
    comments: Vec<CommentView>,
    events: Vec<EventView>,
    warnings: Vec<String>,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    status: u16,
    message: String,
    hint: Option<String>,
    allowed: Vec<String>,
}

struct BoardTaskView {
    number: i64,
    title: String,
    status: String,
    current_role: String,
    waiting_for: String,
    updated_at: String,
}

struct StatusCountView {
    title: String,
    count: i64,
}

struct TaskView {
    number: i64,
    title: String,
    statement: String,
    status: String,
    folder_name: String,
    created_at: String,
    updated_at: String,
}

struct StageView {
    role: String,
    round: i64,
    started_at: String,
}

struct ReportView {
    kind: String,
    title: String,
    role: String,
    created_at: String,
    html: String,
}

struct FileView {
    id: String,
    title: String,
    kind: String,
    size: String,
    created_at: String,
}

struct CommentView {
    author: String,
    body: String,
    created_at: String,
}

struct EventView {
    kind: String,
    actor: String,
    at: String,
}

struct WebError {
    status: StatusCode,
    message: String,
    hint: Option<String>,
    allowed: Vec<String>,
}

impl WebError {
    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            hint: None,
            allowed: Vec::new(),
        }
    }
}

impl From<AppError> for WebError {
    fn from(err: AppError) -> Self {
        let status = match err.code {
            ErrorCode::WorkspaceMissing
            | ErrorCode::RoleMissing
            | ErrorCode::TaskMissing
            | ErrorCode::FieldMissing
            | ErrorCode::InputMissing => StatusCode::BAD_REQUEST,
            ErrorCode::WorkspaceUnknown
            | ErrorCode::RoleUnknown
            | ErrorCode::TaskUnknown
            | ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::Io => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: err.message,
            hint: err.hint,
            allowed: err.allowed.unwrap_or_default(),
        }
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let template = ErrorTemplate {
            status: self.status.as_u16(),
            message: self.message.clone(),
            hint: self.hint,
            allowed: self.allowed,
        };
        match template.render() {
            Ok(html) => (self.status, Html(html)).into_response(),
            Err(err) => {
                tracing::error!(%err, "не удалось отрисовать страницу ошибки");
                (self.status, self.message).into_response()
            }
        }
    }
}
