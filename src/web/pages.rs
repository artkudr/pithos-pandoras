use std::path::Path as FsPath;
use std::sync::Arc;

use askama::Template;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Json};
use rust_embed::RustEmbed;
use serde::Deserialize;

use crate::domain::{AppError, ErrorCode, TaskListItem};
use crate::service::{
    FileAddRequest, Service, TaskBlockRequest, TaskNoteRequest, TaskReopenRequest,
    TaskResumeRequest, TaskSetStatusRequest, TaskUpdateRequest, HUMAN_ACTOR,
};
use crate::web::live::Live;

#[derive(Clone)]
struct AppState {
    service: Arc<Service>,
    project: Arc<tokio::sync::RwLock<String>>,
    live: Live,
}

impl AppState {
    async fn current_project(&self) -> String {
        self.project.read().await.clone()
    }
}

#[derive(RustEmbed)]
#[folder = "src/web/static/"]
struct Assets;

pub fn router(service: Arc<Service>, project: String, live: Live) -> axum::Router {
    let state = AppState {
        service,
        project: Arc::new(tokio::sync::RwLock::new(project)),
        live,
    };
    axum::Router::new()
        .route("/", get(board))
        .route("/projects", get(projects_page))
        .route("/projects/create", post(create_project))
        .route("/projects/select", post(select_project))
        .route("/tasks", post(create_task))
        .route("/tasks/{number}", get(task_page))
        .route("/tasks/{number}/note", post(add_note))
        .route("/tasks/{number}/status", post(set_status))
        .route("/tasks/{number}/block", post(block_task))
        .route("/tasks/{number}/resume", post(resume_task))
        .route("/tasks/{number}/reopen", post(reopen_task))
        .route("/tasks/{number}/update", post(update_task))
        .route("/tasks/{number}/folder", post(create_folder))
        .route(
            "/tasks/{number}/files",
            post(upload_file).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route("/search", get(search))
        .route("/stats", get(stats))
        .route("/settings", get(settings))
        .route("/settings/roles", post(add_role))
        .route("/settings/roles/update", post(update_role))
        .route("/settings/roles/delete", post(delete_role))
        .route("/settings/statuses", post(add_status))
        .route("/settings/config", post(update_config))
        .route("/settings/paths", post(update_paths))
        .route("/files/{file_id}", get(file))
        .route("/events", get(live_events))
        .route("/static/{*path}", get(static_file))
        .with_state(state)
}

async fn live_events(State(state): State<AppState>) -> impl IntoResponse {
    let body = serde_json::json!({ "rev": state.live.revision() });
    ([(header::CACHE_CONTROL, "no-store")], Json(body))
}

async fn board(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let project = state.current_project().await;
    let tasks = state.service.task_list(Some(project.clone())).await?;

    let template = BoardTemplate {
        project,
        current: "board",
        tasks: tasks.into_iter().map(board_task_view).collect(),
    };

    render(template)
}

async fn create_task(
    State(state): State<AppState>,
    Form(form): Form<CreateTaskForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    let route = normalize(form.route);
    let resource = normalize(form.resource);
    let created = state
        .service
        .human_task_create(
            &project,
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
    let project = state.current_project().await;
    let data = state
        .service
        .task_page(Some(project.clone()), Some(number.to_string()))
        .await?;
    let view = state.service.project_get(Some(project.clone())).await?;
    let role_colors: std::collections::HashMap<String, String> = view
        .roles
        .iter()
        .filter_map(|role| role.color.clone().map(|color| (role.name.clone(), color)))
        .collect();
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
    let status_color = status_color(
        &status_id,
        data.status
            .as_ref()
            .and_then(|status| status.color.as_deref()),
    );
    let status_title = data.status.map(|status| status.title).unwrap_or_default();
    let folder_missing = data
        .warnings
        .iter()
        .any(|warning| warning == "folder_missing");

    let template = TaskTemplate {
        project: project.clone(),
        current: "board",
        task: TaskView {
            number: data.task.number,
            title: data.task.title,
            statement: data.task.statement,
            status: status_title,
            status_id,
            status_kind,
            status_color,
            folder_name: data.task.folder_name,
            created_at: format_local_time(&data.task.created_at),
            updated_at: format_local_time(&data.task.updated_at),
        },
        statuses,
        folder_missing,
        current_stage: data.current_stage.map(|stage| StageView {
            role_color: role_colors.get(&stage.role).cloned().unwrap_or_default(),
            role: stage.role,
            round: stage.round,
            started_at: format_local_time(&stage.started_at),
        }),
        reports: data
            .reports
            .into_iter()
            .map(|item| ReportView {
                label: report_kind_label(&item.report.kind).to_string(),
                role_color: role_colors.get(&item.role).cloned().unwrap_or_default(),
                title: item.report.title,
                role: item.role,
                created_at: format_local_time(&item.report.created_at),
                html: render_markdown(&item.report.content),
            })
            .collect(),
        files: data
            .files
            .into_iter()
            .map(|file| {
                let name = std::path::Path::new(&file.path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| file.title.clone());
                FileView {
                    id: file.id,
                    name,
                    comment: file.title,
                    size: file.size.map(format_size).unwrap_or_default(),
                    created_at: format_local_time(&file.created_at),
                }
            })
            .collect(),
        comments: data
            .comments
            .into_iter()
            .map(|comment| CommentView {
                author: comment.author,
                body: comment.body,
                created_at: format_local_time(&comment.created_at),
            })
            .collect(),
        events: data
            .events
            .into_iter()
            .map(|event| EventView {
                label: event_label(&event.kind),
                actor: event.actor.unwrap_or_else(|| String::from("—")),
                at: format_local_time(&event.at),
            })
            .collect(),
        warnings: data.warnings.into_iter().map(warning_text).collect(),
    };

    render(template)
}

async fn add_note(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Form(form): Form<NoteForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state
        .service
        .task_note(TaskNoteRequest {
            project: Some(project.clone()),
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
    let project = state.current_project().await;
    state
        .service
        .task_set_status(TaskSetStatusRequest {
            project: Some(project.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
            status: Some(form.status),
        })
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn create_folder(
    State(state): State<AppState>,
    Path(number): Path<i64>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state
        .service
        .ensure_task_folder(&project, &number.to_string())
        .await?;
    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn upload_file(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    mut multipart: Multipart,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    let mut title = String::new();
    let mut kind = String::from("file");
    let mut file_name = String::from("file.bin");
    let mut bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| WebError::bad_request(format!("multipart: {err}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "title" => {
                title = field
                    .text()
                    .await
                    .map_err(|err| WebError::bad_request(format!("title: {err}")))?;
            }
            "kind" => {
                kind = field
                    .text()
                    .await
                    .map_err(|err| WebError::bad_request(format!("kind: {err}")))?;
            }
            "file" => {
                if let Some(name) = field.file_name() {
                    file_name = name.to_string();
                }
                bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|err| WebError::bad_request(format!("file: {err}")))?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let bytes = bytes.ok_or_else(|| WebError::bad_request("Файл не выбран"))?;
    let kind = match kind.as_str() {
        "image" | "code" | "file" => kind,
        _ => String::from("file"),
    };
    let folder = state
        .service
        .ensure_task_folder(&project, &number.to_string())
        .await?;
    let file_name = sanitize_file_name(&file_name);
    let mut path = FsPath::new(&folder).join(&file_name);
    if path.exists() {
        let id = uuid::Uuid::now_v7().simple().to_string();
        path = FsPath::new(&folder).join(format!("{id}-{file_name}"));
    }
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|err| WebError::internal(format!("записать файл: {err}")))?;
    let title = if title.trim().is_empty() {
        file_name
    } else {
        title.trim().to_string()
    };

    state
        .service
        .file_add(FileAddRequest {
            project: Some(project.clone()),
            task: Some(number.to_string()),
            role: Some(String::from(HUMAN_ACTOR)),
            path: Some(path.to_string_lossy().to_string()),
            title: Some(title),
            kind: Some(kind),
        })
        .await?;

    Ok(Redirect::to(&format!("/tasks/{number}")))
}

async fn block_task(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Form(form): Form<BlockForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state
        .service
        .task_block(TaskBlockRequest {
            project: Some(project.clone()),
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
    let project = state.current_project().await;
    state
        .service
        .task_resume(TaskResumeRequest {
            project: Some(project.clone()),
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
    let project = state.current_project().await;
    state
        .service
        .task_reopen(TaskReopenRequest {
            project: Some(project.clone()),
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
    let project = state.current_project().await;
    state
        .service
        .task_update(TaskUpdateRequest {
            project: Some(project.clone()),
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
    let project = state.current_project().await;
    let q = query.q.unwrap_or_default().trim().to_string();
    let tasks = if q.is_empty() {
        Vec::new()
    } else {
        state
            .service
            .task_search(Some(project.clone()), Some(q.clone()))
            .await?
    };

    let template = SearchTemplate {
        project: project.clone(),
        current: "search",
        query: q,
        tasks: tasks.into_iter().map(board_task_view).collect(),
    };

    render(template)
}

async fn stats(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let project = state.current_project().await;
    let stats = state.service.task_stats(Some(project.clone())).await?;

    let template = StatsTemplate {
        project: project.clone(),
        current: "stats",
        by_status: stats
            .by_status
            .into_iter()
            .map(|count| StatusCountView {
                color: status_color(&count.status_id, None),
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
    };

    render(template)
}

async fn settings(
    State(state): State<AppState>,
    Query(query): Query<SettingsQuery>,
) -> Result<Html<String>, WebError> {
    let project = state.current_project().await;
    let view = state.service.project_get(Some(project.clone())).await?;
    let saved = match query.saved.as_deref() {
        Some("paths") => Some("Пути сохранены."),
        Some("roles") => Some("Список ролей обновлён."),
        Some("statuses") => Some("Статус сохранён."),
        Some("config") => Some("Настройки сохранены."),
        _ => None,
    };

    let template = SettingsTemplate {
        project: project.clone(),
        current: "settings",
        saved,
        root_path: view.project.root_path,
        tasks_dir: view.project.tasks_dir,
        folder_pattern: view.project.folder_pattern,
        config_json: view.project.config_json,
        roles: view
            .roles
            .into_iter()
            .map(|role| SettingsRoleView {
                name: role.name,
                color: role.color.unwrap_or_default(),
            })
            .collect(),
        statuses: view
            .statuses
            .into_iter()
            .map(|status| {
                let color = status_color(&status.id, status.color.as_deref());
                SettingsStatusView {
                    id: status.id,
                    title: status.title,
                    kind: status.kind.clone(),
                    kind_label: status_kind_label(&status.kind).to_string(),
                    sort: status.sort,
                    color,
                }
            })
            .collect(),
    };

    render(template)
}

async fn projects_page(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let current = state.current_project().await;
    let projects = state.service.list_projects().await?;
    let counts = state.service.project_status_counts().await?;

    let template = ProjectsTemplate {
        project: current.clone(),
        current: "projects",
        selected: current,
        projects: projects
            .into_iter()
            .map(|project| {
                let statuses = counts
                    .iter()
                    .filter(|status| status.project_id == project.id && status.count > 0)
                    .map(|status| StatusChipView {
                        title: status.title.clone(),
                        count: status.count,
                        color: status_color(&status.id, status.color.as_deref()),
                    })
                    .collect();
                ProjectItemView {
                    name: project.name,
                    root_path: project.root_path,
                    statuses,
                }
            })
            .collect(),
    };
    render(template)
}

async fn create_project(
    State(state): State<AppState>,
    Form(form): Form<ProjectCreateForm>,
) -> Result<Redirect, WebError> {
    let roles = project_roles_from_form(form.roles_json.as_deref(), form.roles.as_deref());
    let created = state
        .service
        .create_project(
            &form.name,
            &form.root_path,
            &form.tasks_dir,
            &form.folder_pattern,
            &roles,
            &form.config_json,
        )
        .await?;
    *state.project.write().await = created.name.clone();
    Ok(Redirect::to("/"))
}

/// Роли из формы: `roles_json` (редактор таблицей), иначе старый список через запятую.
fn project_roles_from_form(
    roles_json: Option<&str>,
    roles: Option<&str>,
) -> Vec<(String, Option<String>)> {
    if let Some(json) = roles_json {
        #[derive(Deserialize)]
        struct RoleInput {
            name: String,
            color: Option<String>,
        }
        if let Ok(items) = serde_json::from_str::<Vec<RoleInput>>(json) {
            return items
                .into_iter()
                .map(|item| (item.name.trim().to_string(), item.color))
                .filter(|(name, _)| !name.is_empty())
                .collect();
        }
    }
    roles
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(|role| (role.to_string(), None))
        .collect()
}

async fn select_project(
    State(state): State<AppState>,
    Form(form): Form<ProjectSelectForm>,
) -> Result<Redirect, WebError> {
    let view = state.service.project_get(Some(form.name)).await?;
    *state.project.write().await = view.project.name;
    Ok(Redirect::to("/"))
}

async fn add_role(
    State(state): State<AppState>,
    Form(form): Form<RoleForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state
        .service
        .add_role(&project, &form.name, form.color.as_deref())
        .await?;
    Ok(Redirect::to("/settings?saved=roles"))
}

async fn update_role(
    State(state): State<AppState>,
    Form(form): Form<UpdateRoleForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state
        .service
        .update_role(&project, &form.old_name, &form.name, form.color.as_deref())
        .await?;
    Ok(Redirect::to("/settings?saved=roles"))
}

async fn delete_role(
    State(state): State<AppState>,
    Form(form): Form<DeleteRoleForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state.service.delete_role(&project, &form.old_name).await?;
    Ok(Redirect::to("/settings?saved=roles"))
}

async fn add_status(
    State(state): State<AppState>,
    Form(form): Form<StatusCreateForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state
        .service
        .upsert_status(
            &project,
            &form.id,
            &form.title,
            &form.kind,
            form.sort.unwrap_or(100),
            form.color.as_deref(),
        )
        .await?;
    Ok(Redirect::to("/settings?saved=statuses"))
}

async fn update_config(
    State(state): State<AppState>,
    Form(form): Form<ConfigForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state
        .service
        .update_config(&project, &form.config_json)
        .await?;
    Ok(Redirect::to("/settings?saved=config"))
}

async fn update_paths(
    State(state): State<AppState>,
    Form(form): Form<PathsForm>,
) -> Result<Redirect, WebError> {
    let project = state.current_project().await;
    state
        .service
        .update_paths(
            &project,
            &form.root_path,
            &form.tasks_dir,
            &form.folder_pattern,
        )
        .await?;
    Ok(Redirect::to("/settings?saved=paths"))
}

async fn file(
    State(state): State<AppState>,
    Path(file_id): Path<String>,
) -> Result<Response, WebError> {
    let project = state.current_project().await;
    let file = state.service.file_for_project(&project, &file_id).await?;
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
                [
                    (header::CONTENT_TYPE, mime.to_string()),
                    (header::CACHE_CONTROL, String::from("no-cache")),
                ],
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
        status_color: status_color(&task.status_id, None),
        current_role: task.current_role.unwrap_or_else(|| String::from("—")),
        current_role_color: task.current_role_color.unwrap_or_default(),
        waiting_for: if task.waiting_for.is_empty() {
            String::from("—")
        } else {
            task.waiting_for
        },
        updated_at: format_local_time(&task.updated_at),
    }
}

fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn warning_text(code: String) -> String {
    match code.as_str() {
        "folder_missing" => String::from(
            "Папка задачи не создана. Её можно создать кнопкой ниже или загрузкой файла.",
        ),
        "resource_busy" => String::from("Ресурс занят другой активной задачей."),
        _ => code,
    }
}

fn sanitize_file_name(name: &str) -> String {
    let base = FsPath::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file.bin");
    let sanitized: String = base
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        String::from("file.bin")
    } else {
        sanitized
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

fn format_local_time(value: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(value) {
        Ok(parsed) => parsed
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        Err(_) => value.to_string(),
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

/// Подписи для машинных кодов: человек не должен читать имена из базы.
fn event_label(kind: &str) -> String {
    let label = match kind {
        "task_created" => "Задача создана",
        "task_updated" => "Задача изменена",
        "task_status_changed" => "Статус изменён",
        "task_noted" => "Комментарий добавлен",
        "task_blocked" => "Задача остановлена",
        "task_resumed" => "Задача возобновлена",
        "task_closed" => "Задача закрыта",
        "task_reopened" => "Задача переоткрыта",
        "stage_started" => "Этап начат",
        "stage_finished" => "Этап завершён",
        "stage_returned" => "Этап возвращён",
        "report_added" => "Отчёт добавлен",
        "file_added" => "Файл добавлен",
        other => return other.to_string(),
    };
    label.to_string()
}

fn report_kind_label(kind: &str) -> &'static str {
    match kind {
        "report" => "отчёт",
        "plan" => "план",
        "acceptance" => "приёмка",
        "guide" => "инструкция",
        "research" => "исследование",
        "note" => "заметка",
        _ => "отчёт",
    }
}

fn status_kind_label(kind: &str) -> &'static str {
    match kind {
        "active" => "в работе",
        "waiting" => "ожидание",
        "blocked" => "остановлена",
        "done" => "закрыта",
        _ => "неизвестно",
    }
}

/// Цвет статуса: из базы, если он задан, иначе по известному идентификатору.
fn status_color(id: &str, color: Option<&str>) -> String {
    if let Some(value) = color {
        let trimmed = value.trim();
        if is_hex_color(trimmed) {
            return trimmed.to_string();
        }
    }
    match id {
        "new" => "#5c7cfa",
        "active" => "#1c7ed6",
        "waiting" => "#e8890c",
        "blocked" => "#e03131",
        "done" => "#2f9e44",
        _ => "#868e96",
    }
    .to_string()
}

fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    let digits = match bytes.len() {
        4 => &bytes[1..],
        7 => &bytes[1..],
        _ => return false,
    };
    value.starts_with('#') && digits.iter().all(u8::is_ascii_hexdigit)
}

#[derive(Deserialize)]
struct SettingsQuery {
    saved: Option<String>,
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

#[derive(Deserialize)]
struct ProjectCreateForm {
    name: String,
    root_path: String,
    tasks_dir: String,
    folder_pattern: String,
    roles: Option<String>,
    roles_json: Option<String>,
    config_json: String,
}

#[derive(Deserialize)]
struct ProjectSelectForm {
    name: String,
}

#[derive(Deserialize)]
struct RoleForm {
    name: String,
    color: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRoleForm {
    old_name: String,
    name: String,
    color: Option<String>,
}

#[derive(Deserialize)]
struct DeleteRoleForm {
    old_name: String,
}

#[derive(Deserialize)]
struct StatusCreateForm {
    id: String,
    title: String,
    kind: String,
    sort: Option<i64>,
    color: Option<String>,
}

#[derive(Deserialize)]
struct ConfigForm {
    config_json: String,
}

#[derive(Deserialize)]
struct PathsForm {
    root_path: String,
    tasks_dir: String,
    folder_pattern: String,
}

#[derive(Template)]
#[template(path = "board.html")]
struct BoardTemplate {
    project: String,
    current: &'static str,
    tasks: Vec<BoardTaskView>,
}

#[derive(Template)]
#[template(path = "task.html")]
struct TaskTemplate {
    project: String,
    current: &'static str,
    task: TaskView,
    statuses: Vec<StatusOptionView>,
    folder_missing: bool,
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
    project: String,
    current: &'static str,
    query: String,
    tasks: Vec<BoardTaskView>,
}

#[derive(Template)]
#[template(path = "stats.html")]
struct StatsTemplate {
    project: String,
    current: &'static str,
    by_status: Vec<StatusCountView>,
    by_role: Vec<RoleTimeView>,
    open_stages: i64,
}

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
    project: String,
    current: &'static str,
    saved: Option<&'static str>,
    root_path: String,
    tasks_dir: String,
    folder_pattern: String,
    config_json: String,
    roles: Vec<SettingsRoleView>,
    statuses: Vec<SettingsStatusView>,
}

struct SettingsRoleView {
    name: String,
    color: String,
}

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsTemplate {
    project: String,
    current: &'static str,
    selected: String,
    projects: Vec<ProjectItemView>,
}

struct ProjectItemView {
    name: String,
    root_path: String,
    statuses: Vec<StatusChipView>,
}

struct StatusChipView {
    title: String,
    count: i64,
    color: String,
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
    status_color: String,
    current_role: String,
    current_role_color: String,
    waiting_for: String,
    updated_at: String,
}

struct StatusCountView {
    title: String,
    count: i64,
    color: String,
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
    status_color: String,
    folder_name: String,
    created_at: String,
    updated_at: String,
}

struct StageView {
    role: String,
    role_color: String,
    round: i64,
    started_at: String,
}

struct ReportView {
    label: String,
    title: String,
    role: String,
    role_color: String,
    created_at: String,
    html: String,
}

struct FileView {
    id: String,
    name: String,
    comment: String,
    size: String,
    created_at: String,
}

struct CommentView {
    author: String,
    body: String,
    created_at: String,
}

struct EventView {
    label: String,
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
    kind_label: String,
    sort: i64,
    color: String,
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

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            hint: None,
            allowed: Vec::new(),
        }
    }
}

impl From<AppError> for WebError {
    fn from(err: AppError) -> Self {
        let status = match err.code {
            ErrorCode::ProjectMissing
            | ErrorCode::RoleMissing
            | ErrorCode::TaskMissing
            | ErrorCode::FieldMissing
            | ErrorCode::InputMissing => StatusCode::BAD_REQUEST,
            ErrorCode::ProjectUnknown
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
