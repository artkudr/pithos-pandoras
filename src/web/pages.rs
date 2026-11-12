use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Form;
use rust_embed::RustEmbed;
use serde::Deserialize;

use crate::domain::{AppError, ErrorCode, TaskListItem};
use crate::service::{
    Service, TaskBlockRequest, TaskCloseRequest, TaskNoteRequest, TaskReopenRequest,
    TaskResumeRequest, TaskSetStatusRequest, TaskUpdateRequest, HUMAN_ACTOR,
};

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
        .route("/tasks", post(create_task))
        .route("/tasks/{number}", get(task_page))
        .route("/tasks/{number}/note", post(add_note))
        .route("/tasks/{number}/status", post(set_status))
        .route("/tasks/{number}/close", post(close_task))
        .route("/tasks/{number}/block", post(block_task))
        .route("/tasks/{number}/resume", post(resume_task))
        .route("/tasks/{number}/reopen", post(reopen_task))
        .route("/tasks/{number}/update", post(update_task))
        .route("/search", get(search))
        .route("/stats", get(stats))
        .route("/settings", get(settings))
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
        tasks: tasks.into_iter().map(board_task_view).collect(),
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

async fn create_task(
    State(state): State<AppState>,
    Form(form): Form<CreateTaskForm>,
) -> Result<Redirect, WebError> {
    let route = normalize(form.route);
    let resource = normalize(form.resource);
    let created = state
        .service
        .human_task_create(
            &state.workspace,
            &form.title,
            &form.statement,
            route.as_deref(),
            resource.as_deref(),
        )
        .await?;
    Ok(Redirect::to(&format!("/tasks/{}", created.task.number)))
}

async fn task_page(
    State(state): State<AppState>,
    Path(number): Path<i64>,
) -> Result<Html<String>, WebError> {
    let data = state
        .service
        .task_page(Some(state.workspace.clone()), Some(number.to_string()))
        .await?;
    let view = state
        .service
        .workspace_get(Some(state.workspace.clone()))
        .await?;
    let statuses = view
        .statuses
        .into_iter()
        .map(|status| StatusOptionView {
            id: status.id,
            title: status.title,
        })
        .collect();

    let status_id = data.task.status_id.clone();
    let status_kind = data
        .status
        .as_ref()
        .map(|status| status.kind.clone())
        .unwrap_or_default();
    let status_title = data.status.map(|status| status.title).unwrap_or_default();

    let template = TaskTemplate {
        workspace: state.workspace.clone(),
        task: TaskView {
            number: data.task.number,
            title: data.task.title,
            statement: data.task.statement,
            status: status_title,
            status_id,
            status_kind,
            folder_name: data.task.folder_name,
            created_at: data.task.created_at,
            updated_at: data.task.updated_at,
        },
        statuses,
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

async fn add_note(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Form(form): Form<NoteForm>,
) -> Result<Redirect, WebError> {
    state
        .service
        .task_note(TaskNoteRequest {
            workspace: Some(state.workspace.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
            text: Some(form.text),
        })
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn set_status(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Form(form): Form<StatusForm>,
) -> Result<Redirect, WebError> {
    state
        .service
        .task_set_status(TaskSetStatusRequest {
            workspace: Some(state.workspace.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
            status: Some(form.status),
        })
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn close_task(
    State(state): State<AppState>,
    Path(number): Path<i64>,
) -> Result<Redirect, WebError> {
    state
        .service
        .task_close(TaskCloseRequest {
            workspace: Some(state.workspace.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
        })
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn block_task(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Form(form): Form<BlockForm>,
) -> Result<Redirect, WebError> {
    state
        .service
        .task_block(TaskBlockRequest {
            workspace: Some(state.workspace.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
            reason: Some(form.reason),
            waiting_for: normalize(form.waiting_for),
            waiting_ref: normalize(form.waiting_ref),
        })
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn resume_task(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Form(form): Form<ReasonForm>,
) -> Result<Redirect, WebError> {
    state
        .service
        .task_resume(TaskResumeRequest {
            workspace: Some(state.workspace.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
            reason: Some(form.reason),
        })
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn reopen_task(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Form(form): Form<ReasonForm>,
) -> Result<Redirect, WebError> {
    state
        .service
        .task_reopen(TaskReopenRequest {
            workspace: Some(state.workspace.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
            reason: Some(form.reason),
        })
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn update_task(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Form(form): Form<UpdateForm>,
) -> Result<Redirect, WebError> {
    state
        .service
        .task_update(TaskUpdateRequest {
            workspace: Some(state.workspace.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
            title: normalize(form.title),
            statement: normalize(form.statement),
        })
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Html<String>, WebError> {
    let q = query.q.unwrap_or_default().trim().to_string();
    let tasks = if q.is_empty() {
        Vec::new()
    } else {
        state
            .service
            .task_search(Some(state.workspace.clone()), Some(q.clone()))
            .await?
    };

    let template = SearchTemplate {
        workspace: state.workspace.clone(),
        query: q,
        tasks: tasks.into_iter().map(board_task_view).collect(),
    };

    render(template)
}

async fn stats(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let stats = state
        .service
        .task_stats(Some(state.workspace.clone()))
        .await?;

    let template = StatsTemplate {
        workspace: state.workspace.clone(),
        by_status: stats
            .by_status
            .into_iter()
            .map(|count| StatusCountView {
                title: count.title,
                count: count.count,
            })
            .collect(),
        by_role: stats
            .by_role
            .into_iter()
            .map(|role| RoleTimeView {
                role: role.role,
                stages: role.stages,
                duration: format_duration(role.seconds),
            })
            .collect(),
        open_stages: stats.open_stages,
        waiting: stats.waiting,
        blocked: stats.blocked,
    };

    render(template)
}

async fn settings(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let view = state
        .service
        .workspace_get(Some(state.workspace.clone()))
        .await?;

    let template = SettingsTemplate {
        workspace: state.workspace.clone(),
        name: view.workspace.name,
        root_path: view.workspace.root_path,
        tasks_dir: view.workspace.tasks_dir,
        folder_pattern: view.workspace.folder_pattern,
        config_json: view.workspace.config_json,
        roles: view.roles.into_iter().map(|role| role.name).collect(),
        statuses: view
            .statuses
            .into_iter()
            .map(|status| SettingsStatusView {
                id: status.id,
                title: status.title,
                kind: status.kind,
            })
            .collect(),
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

fn board_task_view(task: TaskListItem) -> BoardTaskView {
    BoardTaskView {
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
    }
}

fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

fn format_duration(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours} ч {minutes} мин")
    } else {
        format!("{minutes} мин")
    }
}

#[derive(Deserialize)]
struct CreateTaskForm {
    title: String,
    statement: String,
    route: Option<String>,
    resource: Option<String>,
}

#[derive(Deserialize)]
struct NoteForm {
    text: String,
}

#[derive(Deserialize)]
struct StatusForm {
    status: String,
}

#[derive(Deserialize)]
struct UpdateForm {
    title: Option<String>,
    statement: Option<String>,
}

#[derive(Deserialize)]
struct BlockForm {
    reason: String,
    waiting_for: Option<String>,
    waiting_ref: Option<String>,
}

#[derive(Deserialize)]
struct ReasonForm {
    reason: String,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
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
    statuses: Vec<StatusOptionView>,
    current_stage: Option<StageView>,
    reports: Vec<ReportView>,
    files: Vec<FileView>,
    comments: Vec<CommentView>,
    events: Vec<EventView>,
    warnings: Vec<String>,
}

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate {
    workspace: String,
    query: String,
    tasks: Vec<BoardTaskView>,
}

#[derive(Template)]
#[template(path = "stats.html")]
struct StatsTemplate {
    workspace: String,
    by_status: Vec<StatusCountView>,
    by_role: Vec<RoleTimeView>,
    open_stages: i64,
    waiting: i64,
    blocked: i64,
}

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
    workspace: String,
    name: String,
    root_path: String,
    tasks_dir: String,
    folder_pattern: String,
    config_json: String,
    roles: Vec<String>,
    statuses: Vec<SettingsStatusView>,
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

struct StatusOptionView {
    id: String,
    title: String,
}

struct TaskView {
    number: i64,
    title: String,
    statement: String,
    status: String,
    status_id: String,
    status_kind: String,
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

struct RoleTimeView {
    role: String,
    stages: i64,
    duration: String,
}

struct SettingsStatusView {
    id: String,
    title: String,
    kind: String,
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
