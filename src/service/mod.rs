//! Сценарии приложения: проверки, writer, входы роли.

pub mod inputs;

use std::path::Path;
use std::sync::Arc;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::domain::{
    AppError, Comment, Event, FileAdded, FileRecord, NextStep, RoleInput, RoleTime, StageFinish,
    StageStart, Status, StatusCount, Task, TaskCard, TaskCreated, TaskListItem, TaskStats,
    Workspace, WorkspaceView,
};
use crate::store::{self, Store, StoreError};
use store::{comments, db, events, files, reports, stages, tasks, workspaces};

pub const HUMAN_ACTOR: &str = "человек";

#[derive(Clone)]
pub struct Service {
    store: Arc<Store>,
}

#[derive(Debug, Clone)]
pub struct TaskReport {
    pub report: crate::domain::Report,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct TaskPageData {
    pub task: Task,
    pub status: Option<Status>,
    pub current_stage: Option<crate::domain::Stage>,
    pub reports: Vec<TaskReport>,
    pub files: Vec<FileRecord>,
    pub comments: Vec<Comment>,
    pub events: Vec<Event>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TaskCreateRequest {
    pub workspace: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
    pub route: Option<String>,
    pub resource: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StageStartRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StageFinishRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub verdict: Option<String>,
    pub report: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReportAddRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReportGetRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub report: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReportPutRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskCloseRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskBlockRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
    pub waiting_for: Option<String>,
    pub waiting_ref: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskResumeRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskReopenRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskNoteRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskUpdateRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskSetStatusRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileAddRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub path: Option<String>,
    pub title: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileGetRequest {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub file: Option<String>,
}

impl Service {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    pub async fn workspace_get(
        &self,
        workspace: Option<String>,
    ) -> Result<WorkspaceView, AppError> {
        let name = require_workspace(workspace.as_deref())?;
        self.workspace_view(name).await
    }

    pub async fn task_create(&self, req: TaskCreateRequest) -> Result<TaskCreated, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let title = require_field(req.title.as_deref(), "title")?.to_string();
        let statement = require_field(req.statement.as_deref(), "statement")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;

        self.create_task_internal(
            &workspace_name,
            &role,
            "agent",
            &title,
            &statement,
            req.route.as_deref(),
            req.resource.as_deref(),
        )
        .await
    }

    pub async fn human_task_create(
        &self,
        workspace: &str,
        title: &str,
        statement: &str,
        route: Option<&str>,
        resource: Option<&str>,
    ) -> Result<TaskCreated, AppError> {
        let workspace_name = workspace.trim();
        if workspace_name.is_empty() {
            return Err(AppError::workspace_missing());
        }
        let title = require_field(Some(title), "title")?.to_string();
        let statement = require_field(Some(statement), "statement")?.to_string();
        self.create_task_internal(
            workspace_name,
            HUMAN_ACTOR,
            "human",
            &title,
            &statement,
            route,
            resource,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_task_internal(
        &self,
        workspace_name: &str,
        actor: &str,
        origin: &str,
        title: &str,
        statement: &str,
        route: Option<&str>,
        resource: Option<&str>,
    ) -> Result<TaskCreated, AppError> {
        let view = self.workspace_view(workspace_name).await?;
        let new_status = view
            .statuses
            .iter()
            .find(|status| status.id == "new")
            .or_else(|| view.statuses.iter().find(|status| status.kind == "active"))
            .map(|status| status.id.clone())
            .ok_or_else(|| AppError::not_found("В пространстве нет активного статуса"))?;

        let workspace = view.workspace.clone();
        let workspace_for_write = workspace.clone();
        let date = db::local_date();
        let route = route.map(str::to_string);
        let resource = resource.map(str::to_string);
        let create_folder = origin == "human";
        let origin = origin.to_string();
        let actor = actor.to_string();
        let title = title.to_string();
        let statement = statement.to_string();
        let title_for_event = title.clone();

        let (task, folder_name) = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let number = tasks::next_number(&tx, &workspace_for_write.id)?;
                let folder_name =
                    db::folder_name(&workspace_for_write.folder_pattern, number, &date);
                let task = tasks::insert_task(
                    &tx,
                    &workspace_for_write.id,
                    number,
                    &folder_name,
                    &title,
                    &statement,
                    &new_status,
                    route.as_deref(),
                    resource.as_deref(),
                    &origin,
                )?;
                let payload =
                    serde_json::json!({"number": number, "title": title_for_event}).to_string();
                events::insert_event(
                    &tx,
                    &task.id,
                    "task_created",
                    None,
                    Some(&actor),
                    &payload,
                    &origin,
                )?;
                tx.commit()?;
                Ok((task, folder_name))
            })
            .await?;

        let folder_path = Path::new(&workspace.root_path)
            .join(&workspace.tasks_dir)
            .join(&folder_name);
        let folder_existed = folder_path.exists();
        if !folder_existed && create_folder {
            std::fs::create_dir_all(&folder_path)
                .map_err(|err| AppError::io(format!("создать папку задачи: {err}")))?;
        }
        let warning = if folder_existed {
            Some(String::from("folder_exists"))
        } else {
            None
        };

        Ok(TaskCreated {
            task,
            folder_path: folder_path.to_string_lossy().to_string(),
            warning,
        })
    }

    pub async fn task_list(
        &self,
        workspace: Option<String>,
    ) -> Result<Vec<TaskListItem>, AppError> {
        let name = require_workspace(workspace.as_deref())?.to_string();
        let view = self.workspace_view(&name).await?;
        let workspace_id = view.workspace.id;
        let items = self
            .store
            .read(move |conn| tasks::list_tasks(conn, &workspace_id))
            .await?;
        Ok(items)
    }

    pub async fn task_get(
        &self,
        workspace: Option<String>,
        task: Option<String>,
    ) -> Result<TaskCard, AppError> {
        let workspace_name = require_workspace(workspace.as_deref())?.to_string();
        let task_ref = require_task(task.as_deref())?.to_string();
        let view = self.workspace_view(&workspace_name).await?;
        let workspace = view.workspace.clone();
        let statuses = view.statuses;
        let task_ref_for_error = task_ref.clone();

        let card = self
            .store
            .read(move |conn| {
                let task = tasks::get_task(conn, &workspace.id, &task_ref)?;
                let status = statuses
                    .iter()
                    .find(|status| status.id == task.status_id)
                    .cloned();
                let current_stage = stages::open_stage(conn, &task.id)?;
                let reports = reports::list_report_previews(conn, &task.id)?;
                let files = files::list_files(conn, &task.id)?;
                let mut warnings = Vec::new();
                let folder_path = Path::new(&workspace.root_path)
                    .join(&workspace.tasks_dir)
                    .join(&task.folder_name);
                if !folder_path.exists() {
                    warnings.push(String::from("folder_missing"));
                }
                if let Some(resource) = &task.resource {
                    if tasks::resource_busy(conn, &workspace.id, resource, &task.id)? {
                        warnings.push(String::from("resource_busy"));
                    }
                }
                Ok(TaskCard {
                    task,
                    status,
                    current_stage,
                    reports,
                    files,
                    warnings,
                })
            })
            .await;

        match card {
            Ok(card) => Ok(card),
            Err(StoreError::NotFound) => Err(AppError::task_unknown(&task_ref_for_error)),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn ensure_task_folder(
        &self,
        workspace: &str,
        task: &str,
    ) -> Result<String, AppError> {
        let view = self.workspace_view(workspace).await?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, task).await?;
        let folder_path = Path::new(&workspace.root_path)
            .join(&workspace.tasks_dir)
            .join(&task.folder_name);
        std::fs::create_dir_all(&folder_path)
            .map_err(|err| AppError::io(format!("создать папку задачи: {err}")))?;
        Ok(folder_path.to_string_lossy().to_string())
    }

    pub async fn task_page(
        &self,
        workspace: Option<String>,
        task: Option<String>,
    ) -> Result<TaskPageData, AppError> {
        let workspace_name = require_workspace(workspace.as_deref())?.to_string();
        let task_ref = require_task(task.as_deref())?.to_string();
        let view = self.workspace_view(&workspace_name).await?;
        let workspace = view.workspace.clone();
        let statuses = view.statuses;
        let task_ref_for_error = task_ref.clone();

        let page = self
            .store
            .read(move |conn| {
                let task = tasks::get_task(conn, &workspace.id, &task_ref)?;
                let status = statuses
                    .iter()
                    .find(|status| status.id == task.status_id)
                    .cloned();
                let current_stage = stages::open_stage(conn, &task.id)?;
                let reports = reports::list_reports_with_role(conn, &task.id)?
                    .into_iter()
                    .map(|(report, role)| TaskReport { report, role })
                    .collect();
                let files = files::list_files(conn, &task.id)?;
                let comments = comments::list_comments(conn, &task.id)?;
                let events = events::list_events(conn, &task.id)?;
                let mut warnings = Vec::new();
                let folder_path = Path::new(&workspace.root_path)
                    .join(&workspace.tasks_dir)
                    .join(&task.folder_name);
                if !folder_path.exists() {
                    warnings.push(String::from("folder_missing"));
                }
                if let Some(resource) = &task.resource {
                    if tasks::resource_busy(conn, &workspace.id, resource, &task.id)? {
                        warnings.push(String::from("resource_busy"));
                    }
                }
                Ok(TaskPageData {
                    task,
                    status,
                    current_stage,
                    reports,
                    files,
                    comments,
                    events,
                    warnings,
                })
            })
            .await;

        match page {
            Ok(page) => Ok(page),
            Err(StoreError::NotFound) => Err(AppError::task_unknown(&task_ref_for_error)),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn stage_start(&self, req: StageStartRequest) -> Result<StageStart, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let role_owned = role.clone();

        let (open, round) = self
            .store
            .read({
                let task_id = task_id.clone();
                let role = role_owned.clone();
                move |conn| {
                    let open = stages::open_stage(conn, &task_id)?;
                    let round = stages::next_round(conn, &task_id, &role)?;
                    Ok((open, round))
                }
            })
            .await?;

        if open.is_some() {
            return Err(AppError::conflict("этап уже открыт, сначала закрой его"));
        }

        let inputs = self
            .resolve_inputs(&workspace, &task_id, &role_owned, round)
            .await?;
        let started_at = db::now_rfc3339();
        let task_id_for_write = task_id.clone();
        let role_for_write = role_owned.clone();

        let stage = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let stage = stages::insert_stage(
                    &tx,
                    &task_id_for_write,
                    &role_for_write,
                    round,
                    &started_at,
                )?;
                let payload =
                    serde_json::json!({"role": &role_for_write, "round": round}).to_string();
                events::insert_event(
                    &tx,
                    &task_id_for_write,
                    "stage_started",
                    Some(&stage.id),
                    None,
                    &payload,
                    "agent",
                )?;
                tx.commit()?;
                Ok(stage)
            })
            .await;

        let stage = match stage {
            Ok(stage) => stage,
            Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(ffi_err, _)))
                if ffi_err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                return Err(AppError::conflict("этап уже открыт, сначала закрой его"));
            }
            Err(err) => return Err(err.into()),
        };

        Ok(StageStart { stage, inputs })
    }

    pub async fn stage_finish(&self, req: StageFinishRequest) -> Result<StageFinish, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let verdict = require_field(req.verdict.as_deref(), "verdict")?.to_string();
        let report = require_field(req.report.as_deref(), "report")?.to_string();
        let kind = req.kind.unwrap_or_else(|| String::from("report"));
        validate_kind(&kind, STAGE_KINDS)?;

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let role_owned = role.clone();

        let open = self
            .store
            .read({
                let task_id = task_id.clone();
                move |conn| stages::open_stage(conn, &task_id)
            })
            .await?;
        let Some(stage) = open else {
            return Err(AppError::conflict("нет открытого этапа"));
        };
        if stage.role != role_owned {
            return Err(AppError::conflict("открыт этап другой роли"));
        }

        let now = db::now_rfc3339();
        let seconds = compute_seconds(&stage.started_at, &now);
        let stage_id = stage.id.clone();
        let title = format!("Отчёт: {role_owned}");
        let task_id_for_write = task_id.clone();
        let kind_for_write = kind.clone();
        let title_for_write = title.clone();
        let report_for_write = report.clone();
        let verdict_for_write = verdict.clone();

        let result = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let report = reports::insert_report(
                    &tx,
                    &task_id_for_write,
                    Some(&stage_id),
                    None,
                    &kind_for_write,
                    &title_for_write,
                    &report_for_write,
                    &now,
                )?;
                let stage = stages::finish_stage(
                    &tx,
                    &stage_id,
                    &now,
                    "finished",
                    &verdict_for_write,
                    seconds,
                    "",
                )?;
                let payload =
                    serde_json::json!({"verdict": &verdict_for_write, "kind": &kind_for_write})
                        .to_string();
                events::insert_event(
                    &tx,
                    &task_id_for_write,
                    "stage_finished",
                    Some(&stage_id),
                    None,
                    &payload,
                    "agent",
                )?;
                if is_return_verdict(&verdict_for_write) {
                    let payload = serde_json::json!({
                        "report_id": &report.id,
                        "kind": &report.kind,
                        "verdict": &verdict_for_write,
                    })
                    .to_string();
                    events::insert_event(
                        &tx,
                        &task_id_for_write,
                        "stage_returned",
                        Some(&stage_id),
                        None,
                        &payload,
                        "agent",
                    )?;
                }
                tx.commit()?;
                Ok((stage, report))
            })
            .await;

        match result {
            Ok((stage, report)) => Ok(StageFinish { stage, report }),
            Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(ffi_err, _)))
                if ffi_err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(AppError::conflict(
                    "в этом этапе уже есть отчёт такого вида",
                ))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub async fn report_add(
        &self,
        req: ReportAddRequest,
    ) -> Result<crate::domain::Report, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let kind = require_field(req.kind.as_deref(), "kind")?.to_string();
        validate_kind(&kind, TASK_KINDS)?;
        let title = require_field(req.title.as_deref(), "title")?.to_string();
        let content = require_field(req.content.as_deref(), "content")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let now = db::now_rfc3339();
        let role_for_write = role.clone();
        let kind_for_write = kind.clone();
        let title_for_write = title.clone();

        let report = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let report = reports::insert_report(
                    &tx,
                    &task_id,
                    None,
                    Some(&role_for_write),
                    &kind_for_write,
                    &title_for_write,
                    &content,
                    &now,
                )?;
                let payload =
                    serde_json::json!({"kind": &kind_for_write, "title": &title_for_write})
                        .to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "report_added",
                    None,
                    Some(&role_for_write),
                    &payload,
                    "agent",
                )?;
                tx.commit()?;
                Ok(report)
            })
            .await?;

        Ok(report)
    }

    pub async fn report_put(
        &self,
        req: ReportPutRequest,
    ) -> Result<crate::domain::Report, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let kind = require_field(req.kind.as_deref(), "kind")?.to_string();
        validate_kind(&kind, STAGE_KINDS)?;
        let title = require_field(req.title.as_deref(), "title")?.to_string();
        let content = require_field(req.content.as_deref(), "content")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();

        let open = self
            .store
            .read({
                let task_id = task_id.clone();
                move |conn| stages::open_stage(conn, &task_id)
            })
            .await?;
        let Some(stage) = open else {
            return Err(AppError::conflict("нет открытого этапа"));
        };
        if stage.role != role {
            return Err(AppError::conflict("открыт этап другой роли"));
        }

        let stage_id = stage.id.clone();
        let now = db::now_rfc3339();
        let kind_for_write = kind.clone();
        let title_for_write = title.clone();
        let result = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let report = reports::insert_report(
                    &tx,
                    &task_id,
                    Some(&stage_id),
                    None,
                    &kind_for_write,
                    &title_for_write,
                    &content,
                    &now,
                )?;
                let payload =
                    serde_json::json!({"kind": &kind_for_write, "title": &title_for_write})
                        .to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "report_added",
                    Some(&stage_id),
                    None,
                    &payload,
                    "agent",
                )?;
                tx.commit()?;
                Ok(report)
            })
            .await;

        match result {
            Ok(report) => Ok(report),
            Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(ffi_err, _)))
                if ffi_err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(AppError::conflict(
                    "в этом этапе уже есть отчёт такого вида",
                ))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub async fn report_get(
        &self,
        req: ReportGetRequest,
    ) -> Result<crate::domain::Report, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let report_id = require_field(req.report.as_deref(), "report")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();

        let report = self
            .store
            .read(move |conn| reports::get_report(conn, &report_id))
            .await;

        match report {
            Ok(report) if report.task_id == task_id => Ok(report),
            Ok(_) => Err(AppError::not_found("Отчёт не принадлежит задаче")),
            Err(StoreError::NotFound) => Err(AppError::not_found("Отчёт не найден")),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn task_close(&self, req: TaskCloseRequest) -> Result<Task, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let workspace_id = workspace.id.clone();

        let open = self
            .store
            .read({
                let task_id = task_id.clone();
                move |conn| stages::open_stage(conn, &task_id)
            })
            .await?;
        if open.is_some() {
            return Err(AppError::conflict(
                "нельзя закрыть задачу с открытым этапом",
            ));
        }

        let origin = event_origin(&role);
        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let task = tasks::close_task(&tx, &workspace_id, &task_id)?;
                events::insert_event(
                    &tx,
                    &task_id,
                    "task_closed",
                    None,
                    Some(&role),
                    "{}",
                    origin,
                )?;
                tx.commit()?;
                Ok(task)
            })
            .await?;

        Ok(task)
    }

    pub async fn task_block(&self, req: TaskBlockRequest) -> Result<Task, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let reason = require_field(req.reason.as_deref(), "reason")?.to_string();
        let waiting_for = req.waiting_for.clone().unwrap_or_default();
        let waiting_ref = req.waiting_ref.clone().unwrap_or_default();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let workspace_id = workspace.id.clone();

        let open = self
            .store
            .read({
                let task_id = task_id.clone();
                move |conn| stages::open_stage(conn, &task_id)
            })
            .await?;

        if let Some(stage) = &open {
            if role != HUMAN_ACTOR && stage.role != role {
                return Err(AppError::conflict("открыт этап другой роли"));
            }
        }

        let now = db::now_rfc3339();
        let end_reason = if waiting_for.is_empty() {
            "blocked"
        } else {
            "waiting"
        };
        let seconds = open
            .as_ref()
            .and_then(|stage| compute_seconds(&stage.started_at, &now));
        let open_id = open.as_ref().map(|stage| stage.id.clone());
        let origin = event_origin(&role);

        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                if let Some(stage_id) = open_id {
                    stages::finish_stage(
                        &tx, &stage_id, &now, end_reason, end_reason, seconds, &reason,
                    )?;
                }
                let task = if waiting_for.is_empty() {
                    let status = tasks::status_by_kind(&tx, &workspace_id, "blocked")?;
                    tasks::set_blocked(&tx, &task_id, &status.id)?
                } else {
                    let status = tasks::status_by_kind(&tx, &workspace_id, "waiting")?;
                    tasks::set_waiting(&tx, &task_id, &status.id, &waiting_for, &waiting_ref)?
                };
                let kind = if waiting_for.is_empty() {
                    "task_blocked"
                } else {
                    "task_waiting"
                };
                let payload =
                    serde_json::json!({"reason": &reason, "waiting_for": &waiting_for}).to_string();
                events::insert_event(&tx, &task_id, kind, None, Some(&role), &payload, origin)?;
                tx.commit()?;
                Ok(task)
            })
            .await?;

        Ok(task)
    }

    pub async fn task_resume(&self, req: TaskResumeRequest) -> Result<Task, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let reason = require_field(req.reason.as_deref(), "reason")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let workspace_id = workspace.id.clone();

        let origin = event_origin(&role);
        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let status = tasks::status_by_kind(&tx, &workspace_id, "active")?;
                let task = tasks::clear_waiting(&tx, &task_id, &status.id)?;
                let payload = serde_json::json!({"reason": &reason}).to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "task_resumed",
                    None,
                    Some(&role),
                    &payload,
                    origin,
                )?;
                tx.commit()?;
                Ok(task)
            })
            .await?;

        Ok(task)
    }

    pub async fn task_reopen(&self, req: TaskReopenRequest) -> Result<Task, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let reason = require_field(req.reason.as_deref(), "reason")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let workspace_id = workspace.id.clone();

        let origin = event_origin(&role);
        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let task = tasks::reopen_task(&tx, &workspace_id, &task_id)?;
                let payload = serde_json::json!({"reason": &reason}).to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "task_reopened",
                    None,
                    Some(&role),
                    &payload,
                    origin,
                )?;
                tx.commit()?;
                Ok(task)
            })
            .await?;

        Ok(task)
    }

    pub async fn task_note(&self, req: TaskNoteRequest) -> Result<Comment, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let text = require_field(req.text.as_deref(), "text")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let now = db::now_rfc3339();

        let origin = event_origin(&role);
        let comment = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let comment = comments::insert_comment(&tx, &task_id, &role, &text, &now)?;
                let payload = serde_json::json!({"text": &text}).to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "task_noted",
                    None,
                    Some(&role),
                    &payload,
                    origin,
                )?;
                tx.commit()?;
                Ok(comment)
            })
            .await?;

        Ok(comment)
    }

    pub async fn task_timeline(
        &self,
        workspace: Option<String>,
        task: Option<String>,
    ) -> Result<Vec<Event>, AppError> {
        let workspace_name = require_workspace(workspace.as_deref())?.to_string();
        let task_ref = require_task(task.as_deref())?.to_string();
        let view = self.workspace_view(&workspace_name).await?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let events = self
            .store
            .read(move |conn| events::list_events(conn, &task_id))
            .await?;
        Ok(events)
    }

    pub async fn task_update(&self, req: TaskUpdateRequest) -> Result<Task, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();

        let title = req
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let statement = req
            .statement
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if title.is_none() && statement.is_none() {
            return Err(AppError::field_missing("title или statement"));
        }

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();

        let origin = event_origin(&role);
        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let task =
                    tasks::update_task(&tx, &task_id, title.as_deref(), statement.as_deref())?;
                let payload =
                    serde_json::json!({"title": title, "statement": statement}).to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "task_updated",
                    None,
                    Some(&role),
                    &payload,
                    origin,
                )?;
                tx.commit()?;
                Ok(task)
            })
            .await?;

        Ok(task)
    }

    pub async fn task_set_status(&self, req: TaskSetStatusRequest) -> Result<Task, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let status = require_field(req.status.as_deref(), "status")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();
        let workspace_id = workspace.id.clone();

        let origin = event_origin(&role);
        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let status_row = tasks::status_by_id(&tx, &workspace_id, &status)?;
                if status_row.kind == "done" {
                    if stages::open_stage(&tx, &task_id)?.is_some() {
                        return Err(StoreError::Conflict(
                            "нельзя закрыть задачу с открытым этапом".to_string(),
                        ));
                    }
                    let task = tasks::close_task(&tx, &workspace_id, &task_id)?;
                    events::insert_event(
                        &tx,
                        &task_id,
                        "task_closed",
                        None,
                        Some(&role),
                        "{}",
                        origin,
                    )?;
                    tx.commit()?;
                    return Ok(task);
                }
                let task = tasks::set_status(&tx, &task_id, &status)?;
                let payload = serde_json::json!({"status": &status}).to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "task_status_changed",
                    None,
                    Some(&role),
                    &payload,
                    origin,
                )?;
                tx.commit()?;
                Ok(task)
            })
            .await?;

        Ok(task)
    }

    pub async fn task_next(
        &self,
        workspace: Option<String>,
        task: Option<String>,
    ) -> Result<NextStep, AppError> {
        let workspace_name = require_workspace(workspace.as_deref())?.to_string();
        let task_ref = require_task(task.as_deref())?.to_string();
        let view = self.workspace_view(&workspace_name).await?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();

        let open = self
            .store
            .read(move |conn| stages::open_stage(conn, &task_id))
            .await?;

        Ok(match open {
            Some(stage) => NextStep {
                action: String::from("stage_finish"),
                role: Some(stage.role),
                round: Some(stage.round),
                reason: String::from("открыт этап"),
            },
            None => NextStep {
                action: String::from("stage_start"),
                role: None,
                round: None,
                reason: String::from("открытого этапа нет"),
            },
        })
    }

    pub async fn task_stats(&self, workspace: Option<String>) -> Result<TaskStats, AppError> {
        let name = require_workspace(workspace.as_deref())?.to_string();
        let view = self.workspace_view(&name).await?;
        let workspace_id = view.workspace.id;

        let stats = self
            .store
            .read(move |conn| {
                let mut by_status = Vec::new();
                {
                    let mut stmt = conn.prepare(
                        "SELECT s.id, s.title, COUNT(t.id)
                         FROM workspace_statuses s
                         LEFT JOIN tasks t
                           ON t.workspace_id = s.workspace_id AND t.status_id = s.id
                         WHERE s.workspace_id = ?1
                         GROUP BY s.id, s.title
                         ORDER BY s.sort, s.id",
                    )?;
                    let rows = stmt.query_map([workspace_id.as_str()], |row| {
                        Ok(StatusCount {
                            status_id: row.get(0)?,
                            title: row.get(1)?,
                            count: row.get(2)?,
                        })
                    })?;
                    for row in rows {
                        by_status.push(row?);
                    }
                }

                let mut by_role = Vec::new();
                {
                    let mut stmt = conn.prepare(
                        "SELECT st.role, COALESCE(SUM(st.seconds), 0), COUNT(*)
                         FROM stages st
                         JOIN tasks t ON t.id = st.task_id
                         WHERE t.workspace_id = ?1
                         GROUP BY st.role
                         ORDER BY st.role",
                    )?;
                    let rows = stmt.query_map([workspace_id.as_str()], |row| {
                        Ok(RoleTime {
                            role: row.get(0)?,
                            seconds: row.get(1)?,
                            stages: row.get(2)?,
                        })
                    })?;
                    for row in rows {
                        by_role.push(row?);
                    }
                }

                let open_stages: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM stages st
                     JOIN tasks t ON t.id = st.task_id
                     WHERE t.workspace_id = ?1 AND st.finished_at IS NULL",
                    [workspace_id.as_str()],
                    |row| row.get(0),
                )?;
                let waiting: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM tasks t
                     JOIN workspace_statuses s
                       ON s.workspace_id = t.workspace_id AND s.id = t.status_id
                     WHERE t.workspace_id = ?1 AND s.kind = 'waiting'",
                    [workspace_id.as_str()],
                    |row| row.get(0),
                )?;
                let blocked: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM tasks t
                     JOIN workspace_statuses s
                       ON s.workspace_id = t.workspace_id AND s.id = t.status_id
                     WHERE t.workspace_id = ?1 AND s.kind = 'blocked'",
                    [workspace_id.as_str()],
                    |row| row.get(0),
                )?;

                Ok(TaskStats {
                    by_status,
                    by_role,
                    open_stages,
                    waiting,
                    blocked,
                })
            })
            .await?;

        Ok(stats)
    }

    pub async fn task_search(
        &self,
        workspace: Option<String>,
        query: Option<String>,
    ) -> Result<Vec<TaskListItem>, AppError> {
        let name = require_workspace(workspace.as_deref())?.to_string();
        let query = require_field(query.as_deref(), "query")?.to_string();
        let view = self.workspace_view(&name).await?;
        let workspace_id = view.workspace.id;
        let fts_query = query
            .split_whitespace()
            .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" AND ");

        let items = self
            .store
            .read(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT t.id, t.number, t.folder_name, t.title, t.status_id,
                            COALESCE(s.title, t.status_id),
                            (SELECT st.role FROM stages st
                              WHERE st.task_id = t.id AND st.finished_at IS NULL
                              LIMIT 1),
                            t.waiting_for, t.updated_at
                     FROM tasks t
                     LEFT JOIN workspace_statuses s
                       ON s.workspace_id = t.workspace_id AND s.id = t.status_id
                     WHERE t.workspace_id = ?1
                       AND t.id IN (
                           SELECT task_id FROM tasks_fts
                            WHERE tasks_fts MATCH ?2
                           UNION
                           SELECT task_id FROM reports_fts
                            WHERE reports_fts MATCH ?2
                           UNION
                           SELECT task_id FROM comments_fts
                            WHERE comments_fts MATCH ?2
                       )
                     ORDER BY t.number DESC",
                )?;
                let rows = stmt.query_map(rusqlite::params![workspace_id, fts_query], |row| {
                    Ok(TaskListItem {
                        id: row.get(0)?,
                        number: row.get(1)?,
                        folder_name: row.get(2)?,
                        title: row.get(3)?,
                        status_id: row.get(4)?,
                        status_title: row.get(5)?,
                        current_role: row.get(6)?,
                        waiting_for: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(row?);
                }
                Ok(out)
            })
            .await?;

        Ok(items)
    }

    pub async fn file_add(&self, req: FileAddRequest) -> Result<FileAdded, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let path = require_field(req.path.as_deref(), "path")?.to_string();
        let title = require_field(req.title.as_deref(), "title")?.to_string();
        let kind = req.kind.unwrap_or_else(|| String::from("file"));

        let view = self.workspace_view(&workspace_name).await?;
        ensure_role(&view.roles, &role)?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();

        let root = std::fs::canonicalize(&workspace.root_path)
            .map_err(|err| AppError::io(format!("корень проекта: {err}")))?;
        let canonical = std::fs::canonicalize(&path)
            .map_err(|err| AppError::io(format!("файл `{path}`: {err}")))?;
        if !canonical.starts_with(&root) {
            return Err(AppError::conflict("путь файла выходит за корень проекта"));
        }
        let size = std::fs::metadata(&canonical)
            .ok()
            .map(|meta| meta.len() as i64);
        let path_string = canonical.to_string_lossy().to_string();
        let now = db::now_rfc3339();
        let role_for_write = role.clone();
        let kind_for_write = kind.clone();
        let title_for_write = title.clone();

        let file = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let file = files::insert_file(
                    &tx,
                    &task_id,
                    None,
                    None,
                    &role_for_write,
                    &kind_for_write,
                    &title_for_write,
                    &path_string,
                    None,
                    size,
                    &now,
                )?;
                let payload = serde_json::json!({"path": &path_string, "title": &title_for_write})
                    .to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "file_added",
                    None,
                    Some(&role_for_write),
                    &payload,
                    "agent",
                )?;
                tx.commit()?;
                Ok(file)
            })
            .await?;

        Ok(FileAdded {
            file_id: file.id.clone(),
            url: format!("/files/{}", file.id),
        })
    }

    pub async fn file_get(&self, req: FileGetRequest) -> Result<FileRecord, AppError> {
        let workspace_name = require_workspace(req.workspace.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let file_id = require_field(req.file.as_deref(), "file")?.to_string();

        let view = self.workspace_view(&workspace_name).await?;
        let workspace = view.workspace.clone();
        let task = self.task(&workspace.id, &task_ref).await?;
        let task_id = task.id.clone();

        let file = self
            .store
            .read(move |conn| files::get_file(conn, &file_id))
            .await;

        match file {
            Ok(file) if file.task_id == task_id => Ok(file),
            Ok(_) => Err(AppError::not_found("Файл не принадлежит задаче")),
            Err(StoreError::NotFound) => Err(AppError::not_found("Файл не найден")),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn file_for_workspace(
        &self,
        workspace: &str,
        file_id: &str,
    ) -> Result<FileRecord, AppError> {
        let view = self.workspace_view(workspace).await?;
        let workspace_id = view.workspace.id;
        let file_id = file_id.to_string();

        let file = self
            .store
            .read(move |conn| {
                let file = files::get_file(conn, &file_id)?;
                let task = tasks::get_task_by_id(conn, &file.task_id)?;
                if task.workspace_id != workspace_id {
                    return Err(StoreError::NotFound);
                }
                Ok(file)
            })
            .await;

        match file {
            Ok(file) => Ok(file),
            Err(StoreError::NotFound) => Err(AppError::not_found("Файл не найден")),
            Err(err) => Err(err.into()),
        }
    }

    async fn workspace_view(&self, name: &str) -> Result<WorkspaceView, AppError> {
        let name_owned = name.to_string();
        let (view, names) = self
            .store
            .read(move |conn| {
                let names = workspaces::list_workspace_names(conn)?;
                let view = match workspaces::workspace_view(conn, &name_owned) {
                    Ok(view) => Some(view),
                    Err(StoreError::NotFound) => None,
                    Err(err) => return Err(err),
                };
                Ok((view, names))
            })
            .await?;

        match view {
            Some(view) => Ok(view),
            None => Err(AppError::workspace_unknown(name, names)),
        }
    }

    async fn task(&self, workspace_id: &str, task_ref: &str) -> Result<Task, AppError> {
        let workspace_id = workspace_id.to_string();
        let task_ref_owned = task_ref.to_string();
        let result = self
            .store
            .read(move |conn| tasks::get_task(conn, &workspace_id, &task_ref_owned))
            .await;

        match result {
            Ok(task) => Ok(task),
            Err(StoreError::NotFound) => Err(AppError::task_unknown(task_ref)),
            Err(err) => Err(err.into()),
        }
    }

    async fn resolve_inputs(
        &self,
        workspace: &Workspace,
        task_id: &str,
        role: &str,
        round: i64,
    ) -> Result<Vec<RoleInput>, AppError> {
        let workspace = workspace.clone();
        let task_id = task_id.to_string();
        let role = role.to_string();
        let inputs = self
            .store
            .read(move |conn| inputs::resolve(conn, &workspace, &task_id, &role, round))
            .await?;
        Ok(inputs)
    }
}

const STAGE_KINDS: &[&str] = &["report", "plan", "acceptance", "guide"];
const TASK_KINDS: &[&str] = &["report", "plan", "acceptance", "guide", "research", "note"];

fn validate_kind(kind: &str, allowed: &[&str]) -> Result<(), AppError> {
    if allowed.contains(&kind) {
        Ok(())
    } else {
        Err(AppError::field_missing("kind")
            .with_allowed(allowed.iter().map(|value| (*value).to_string()).collect()))
    }
}

fn is_return_verdict(verdict: &str) -> bool {
    matches!(verdict, "return" | "returned" | "возврат" | "на доработку")
}

fn event_origin(role: &str) -> &'static str {
    if role == HUMAN_ACTOR {
        "human"
    } else {
        "agent"
    }
}

fn require_workspace(value: Option<&str>) -> Result<&str, AppError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Ok(value),
        None => Err(AppError::workspace_missing()),
    }
}

fn require_role(value: Option<&str>) -> Result<&str, AppError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Ok(value),
        None => Err(AppError::role_missing()),
    }
}

fn require_task(value: Option<&str>) -> Result<&str, AppError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Ok(value),
        None => Err(AppError::task_missing()),
    }
}

fn require_field<'a>(value: Option<&'a str>, field: &str) -> Result<&'a str, AppError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Ok(value),
        None => Err(AppError::field_missing(field)),
    }
}

fn ensure_role(roles: &[crate::domain::Role], role: &str) -> Result<(), AppError> {
    if role == HUMAN_ACTOR || roles.iter().any(|item| item.name == role) {
        Ok(())
    } else {
        Err(AppError::role_unknown(
            role,
            roles.iter().map(|item| item.name.clone()).collect(),
        ))
    }
}

fn compute_seconds(started_at: &str, finished_at: &str) -> Option<i64> {
    let start = OffsetDateTime::parse(started_at, &Rfc3339).ok()?;
    let end = OffsetDateTime::parse(finished_at, &Rfc3339).ok()?;
    let seconds = (end - start).whole_seconds();
    if seconds < 0 {
        None
    } else {
        Some(seconds)
    }
}
