//! Сценарии приложения: проверки, writer, входы роли.

pub mod inputs;

use std::path::Path;
use std::sync::Arc;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::domain::{
    AppError, Comment, Event, FileAdded, FileRecord, NextStep, Project, ProjectView, RoleInput,
    RoleTime, StageFinish, StageStart, Status, StatusCount, Task, TaskCard, TaskCreated,
    TaskListItem, TaskStats,
};
use crate::store::{self, Store, StoreError};
use store::{comments, db, events, files, projects, reports, stages, tasks};

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
pub struct ProjectStatusCount {
    pub project_id: String,
    pub id: String,
    pub title: String,
    pub color: Option<String>,
    pub count: i64,
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
    pub project: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
    pub route: Option<String>,
    pub resource: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StageStartRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StageFinishRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub verdict: Option<String>,
    pub report: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReportAddRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReportGetRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub report: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReportPutRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskCloseRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskBlockRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
    pub waiting_for: Option<String>,
    pub waiting_ref: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskResumeRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskReopenRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskNoteRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskUpdateRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskSetStatusRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileAddRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub path: Option<String>,
    pub title: Option<String>,
    pub comment: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileUpdateRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub file: Option<String>,
    pub comment: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileDeleteRequest {
    pub project: Option<String>,
    pub task: Option<String>,
    pub file: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileGetRequest {
    pub project: Option<String>,
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

    pub async fn project_get(&self, project: Option<String>) -> Result<ProjectView, AppError> {
        let name = require_project(project.as_deref())?;
        self.project_view(name).await
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>, AppError> {
        let projects = self.store.read(projects::list_projects).await?;
        Ok(projects)
    }

    /// Статусы всех проектов с числом задач: для списка проектов.
    pub async fn project_status_counts(&self) -> Result<Vec<ProjectStatusCount>, AppError> {
        let rows = self.store.read(projects::list_status_counts).await?;
        Ok(rows
            .into_iter()
            .map(|row| ProjectStatusCount {
                project_id: row.project_id,
                id: row.id,
                title: row.title,
                color: row.color,
                count: row.count,
            })
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_project(
        &self,
        name: &str,
        root_path: &str,
        tasks_dir: &str,
        folder_pattern: &str,
        roles: &[(String, Option<String>)],
        config_json: &str,
    ) -> Result<Project, AppError> {
        let name = require_field(Some(name), "name")?.to_string();
        let root_path = require_field(Some(root_path), "root_path")?.to_string();
        let tasks_dir = require_field(Some(tasks_dir), "tasks_dir")?.to_string();
        let folder_pattern = require_field(Some(folder_pattern), "folder_pattern")?.to_string();
        serde_json::from_str::<serde_json::Value>(config_json)
            .map_err(|err| AppError::field_missing("config_json").with_hint(err.to_string()))?;
        let roles: Vec<(String, Option<String>)> = roles
            .iter()
            .map(|(role, color)| (role.trim().to_string(), normalize_color(color.as_deref())))
            .filter(|(role, _)| !role.is_empty())
            .collect();
        let config_json = config_json.to_string();

        let project = self
            .store
            .write(move |conn| {
                projects::init_project(
                    conn,
                    &name,
                    &root_path,
                    &tasks_dir,
                    &folder_pattern,
                    &roles,
                    &config_json,
                )
            })
            .await?;
        Ok(project)
    }

    pub async fn add_role(
        &self,
        project: &str,
        role: &str,
        color: Option<&str>,
    ) -> Result<(), AppError> {
        let role = require_field(Some(role), "role")?.to_string();
        let color = normalize_color(color);
        let view = self.project_view(project).await?;
        let project_id = view.project.id;
        self.store
            .write(move |conn| projects::add_role(conn, &project_id, &role, color.as_deref()))
            .await?;
        Ok(())
    }

    pub async fn update_role(
        &self,
        project: &str,
        old_name: &str,
        new_name: &str,
        color: Option<&str>,
    ) -> Result<(), AppError> {
        let old_name = require_field(Some(old_name), "role")?.to_string();
        let new_name = require_field(Some(new_name), "role")?.to_string();
        let color = normalize_color(color);
        let view = self.project_view(project).await?;
        let project_id = view.project.id;
        self.store
            .write(move |conn| {
                projects::update_role(conn, &project_id, &old_name, &new_name, color.as_deref())
            })
            .await?;
        Ok(())
    }

    pub async fn delete_role(&self, project: &str, name: &str) -> Result<(), AppError> {
        let name = require_field(Some(name), "role")?.to_string();
        let view = self.project_view(project).await?;
        let project_id = view.project.id;
        self.store
            .write(move |conn| projects::delete_role(conn, &project_id, &name))
            .await?;
        Ok(())
    }

    pub async fn upsert_status(
        &self,
        project: &str,
        id: &str,
        title: &str,
        kind: &str,
        sort: i64,
        color: Option<&str>,
    ) -> Result<(), AppError> {
        let id = require_field(Some(id), "status.id")?.to_string();
        let title = require_field(Some(title), "status.title")?.to_string();
        let kind = require_field(Some(kind), "status.kind")?.to_string();
        if !matches!(kind.as_str(), "active" | "waiting" | "blocked" | "done") {
            return Err(AppError::field_missing("status.kind").with_allowed(vec![
                String::from("active"),
                String::from("waiting"),
                String::from("blocked"),
                String::from("done"),
            ]));
        }
        let color = normalize_color(color);
        let view = self.project_view(project).await?;
        let project_id = view.project.id;
        self.store
            .write(move |conn| {
                projects::upsert_status(
                    conn,
                    &project_id,
                    &id,
                    &title,
                    &kind,
                    color.as_deref(),
                    sort,
                )
            })
            .await?;
        Ok(())
    }

    pub async fn update_config(&self, project: &str, config_json: &str) -> Result<(), AppError> {
        serde_json::from_str::<serde_json::Value>(config_json)
            .map_err(|err| AppError::field_missing("config_json").with_hint(err.to_string()))?;
        let view = self.project_view(project).await?;
        let project_id = view.project.id;
        let config_json = config_json.to_string();
        self.store
            .write(move |conn| projects::update_config(conn, &project_id, &config_json))
            .await?;
        Ok(())
    }

    pub async fn update_paths(
        &self,
        project: &str,
        root_path: &str,
        tasks_dir: &str,
        folder_pattern: &str,
    ) -> Result<(), AppError> {
        let root_path = require_field(Some(root_path), "root_path")?.to_string();
        let tasks_dir = require_field(Some(tasks_dir), "tasks_dir")?.to_string();
        let folder_pattern = require_field(Some(folder_pattern), "folder_pattern")?.to_string();
        let view = self.project_view(project).await?;
        let project_id = view.project.id;
        self.store
            .write(move |conn| {
                projects::update_paths(conn, &project_id, &root_path, &tasks_dir, &folder_pattern)
            })
            .await?;
        Ok(())
    }

    pub async fn task_create(&self, req: TaskCreateRequest) -> Result<TaskCreated, AppError> {
        let project_name = require_project(req.project.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let title = require_field(req.title.as_deref(), "title")?.to_string();
        let statement = require_field(req.statement.as_deref(), "statement")?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;

        self.create_task_internal(
            &project_name,
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
        project: &str,
        title: &str,
        statement: &str,
        route: Option<&str>,
        resource: Option<&str>,
    ) -> Result<TaskCreated, AppError> {
        let project_name = project.trim();
        if project_name.is_empty() {
            return Err(AppError::project_missing());
        }
        let title = require_field(Some(title), "title")?.to_string();
        let statement = require_field(Some(statement), "statement")?.to_string();
        self.create_task_internal(
            project_name,
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
        project_name: &str,
        actor: &str,
        origin: &str,
        title: &str,
        statement: &str,
        route: Option<&str>,
        resource: Option<&str>,
    ) -> Result<TaskCreated, AppError> {
        let view = self.project_view(project_name).await?;
        let new_status = view
            .statuses
            .iter()
            .find(|status| status.id == "new")
            .or_else(|| view.statuses.iter().find(|status| status.kind == "active"))
            .map(|status| status.id.clone())
            .ok_or_else(|| AppError::not_found("В проекте нет активного статуса"))?;

        let project = view.project.clone();
        let project_for_write = project.clone();
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
                let number = tasks::next_number(&tx, &project_for_write.id)?;
                let folder_name = db::folder_name(&project_for_write.folder_pattern, number, &date);
                let task = tasks::insert_task(
                    &tx,
                    &project_for_write.id,
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

        let folder_path = Path::new(&project.root_path)
            .join(&project.tasks_dir)
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

    pub async fn task_list(&self, project: Option<String>) -> Result<Vec<TaskListItem>, AppError> {
        let name = require_project(project.as_deref())?.to_string();
        let view = self.project_view(&name).await?;
        let project_id = view.project.id;
        let items = self
            .store
            .read(move |conn| tasks::list_tasks(conn, &project_id))
            .await?;
        Ok(items)
    }

    pub async fn task_get(
        &self,
        project: Option<String>,
        task: Option<String>,
    ) -> Result<TaskCard, AppError> {
        let project_name = require_project(project.as_deref())?.to_string();
        let task_ref = require_task(task.as_deref())?.to_string();
        let view = self.project_view(&project_name).await?;
        let project = view.project.clone();
        let statuses = view.statuses;
        let task_ref_for_error = task_ref.clone();

        let card = self
            .store
            .read(move |conn| {
                let task = tasks::get_task(conn, &project.id, &task_ref)?;
                let status = statuses
                    .iter()
                    .find(|status| status.id == task.status_id)
                    .cloned();
                let current_stage = stages::open_stage(conn, &task.id)?;
                let reports = reports::list_report_previews(conn, &task.id)?;
                let files = files::list_files(conn, &task.id)?;
                let mut warnings = Vec::new();
                let folder_path = Path::new(&project.root_path)
                    .join(&project.tasks_dir)
                    .join(&task.folder_name);
                if !folder_path.exists() {
                    warnings.push(String::from("folder_missing"));
                }
                if let Some(resource) = &task.resource {
                    if tasks::resource_busy(conn, &project.id, resource, &task.id)? {
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

    pub async fn ensure_task_folder(&self, project: &str, task: &str) -> Result<String, AppError> {
        let view = self.project_view(project).await?;
        let project = view.project.clone();
        let task = self.task(&project.id, task).await?;
        let folder_path = Path::new(&project.root_path)
            .join(&project.tasks_dir)
            .join(&task.folder_name);
        std::fs::create_dir_all(&folder_path)
            .map_err(|err| AppError::io(format!("создать папку задачи: {err}")))?;
        Ok(folder_path.to_string_lossy().to_string())
    }

    pub async fn task_page(
        &self,
        project: Option<String>,
        task: Option<String>,
    ) -> Result<TaskPageData, AppError> {
        let project_name = require_project(project.as_deref())?.to_string();
        let task_ref = require_task(task.as_deref())?.to_string();
        let view = self.project_view(&project_name).await?;
        let project = view.project.clone();
        let statuses = view.statuses;
        let task_ref_for_error = task_ref.clone();

        let page = self
            .store
            .read(move |conn| {
                let task = tasks::get_task(conn, &project.id, &task_ref)?;
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
                let folder_path = Path::new(&project.root_path)
                    .join(&project.tasks_dir)
                    .join(&task.folder_name);
                if !folder_path.exists() {
                    warnings.push(String::from("folder_missing"));
                }
                if let Some(resource) = &task.resource {
                    if tasks::resource_busy(conn, &project.id, resource, &task.id)? {
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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
            .resolve_inputs(&project, &task_id, &role_owned, round)
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let verdict = require_field(req.verdict.as_deref(), "verdict")?.to_string();
        let report = require_field(req.report.as_deref(), "report")?.to_string();
        let kind = req.kind.unwrap_or_else(|| String::from("report"));
        validate_kind(&kind, STAGE_KINDS)?;

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let kind = require_field(req.kind.as_deref(), "kind")?.to_string();
        validate_kind(&kind, TASK_KINDS)?;
        let title = require_field(req.title.as_deref(), "title")?.to_string();
        let content = require_field(req.content.as_deref(), "content")?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let kind = require_field(req.kind.as_deref(), "kind")?.to_string();
        validate_kind(&kind, STAGE_KINDS)?;
        let title = require_field(req.title.as_deref(), "title")?.to_string();
        let content = require_field(req.content.as_deref(), "content")?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let report_id = require_field(req.report.as_deref(), "report")?.to_string();

        let view = self.project_view(&project_name).await?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
        let task_id = task.id.clone();
        let project_id = project.id.clone();

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
                let task = tasks::close_task(&tx, &project_id, &task_id)?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let reason = require_field(req.reason.as_deref(), "reason")?.to_string();
        let waiting_for = req.waiting_for.clone().unwrap_or_default();
        let waiting_ref = req.waiting_ref.clone().unwrap_or_default();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
        let task_id = task.id.clone();
        let project_id = project.id.clone();

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
                    let status = tasks::status_by_kind(&tx, &project_id, "blocked")?;
                    tasks::set_blocked(&tx, &task_id, &status.id)?
                } else {
                    let status = tasks::status_by_kind(&tx, &project_id, "waiting")?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let reason = require_field(req.reason.as_deref(), "reason")?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
        let task_id = task.id.clone();
        let project_id = project.id.clone();

        let origin = event_origin(&role);
        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let status = tasks::status_by_kind(&tx, &project_id, "active")?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let reason = require_field(req.reason.as_deref(), "reason")?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
        let task_id = task.id.clone();
        let project_id = project.id.clone();

        let origin = event_origin(&role);
        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let task = tasks::reopen_task(&tx, &project_id, &task_id)?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let text = require_field(req.text.as_deref(), "text")?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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
        project: Option<String>,
        task: Option<String>,
    ) -> Result<Vec<Event>, AppError> {
        let project_name = require_project(project.as_deref())?.to_string();
        let task_ref = require_task(task.as_deref())?.to_string();
        let view = self.project_view(&project_name).await?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
        let task_id = task.id.clone();
        let events = self
            .store
            .read(move |conn| events::list_events(conn, &task_id))
            .await?;
        Ok(events)
    }

    pub async fn task_update(&self, req: TaskUpdateRequest) -> Result<Task, AppError> {
        let project_name = require_project(req.project.as_deref())?.to_string();
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

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let status = require_field(req.status.as_deref(), "status")?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
        let task_id = task.id.clone();
        let project_id = project.id.clone();

        let origin = event_origin(&role);
        let task = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let status_row = tasks::status_by_id(&tx, &project_id, &status)?;
                if status_row.kind == "done" {
                    if stages::open_stage(&tx, &task_id)?.is_some() {
                        return Err(StoreError::Conflict(
                            "нельзя закрыть задачу с открытым этапом".to_string(),
                        ));
                    }
                    let task = tasks::close_task(&tx, &project_id, &task_id)?;
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
        project: Option<String>,
        task: Option<String>,
    ) -> Result<NextStep, AppError> {
        let project_name = require_project(project.as_deref())?.to_string();
        let task_ref = require_task(task.as_deref())?.to_string();
        let view = self.project_view(&project_name).await?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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

    pub async fn task_stats(&self, project: Option<String>) -> Result<TaskStats, AppError> {
        let name = require_project(project.as_deref())?.to_string();
        let view = self.project_view(&name).await?;
        let project_id = view.project.id;

        let stats = self
            .store
            .read(move |conn| {
                let mut by_status = Vec::new();
                {
                    let mut stmt = conn.prepare(
                        "SELECT s.id, s.title, COUNT(t.id)
                         FROM project_statuses s
                         LEFT JOIN tasks t
                           ON t.project_id = s.project_id AND t.status_id = s.id
                         WHERE s.project_id = ?1
                         GROUP BY s.id, s.title
                         ORDER BY s.sort, s.id",
                    )?;
                    let rows = stmt.query_map([project_id.as_str()], |row| {
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
                         WHERE t.project_id = ?1
                         GROUP BY st.role
                         ORDER BY st.role",
                    )?;
                    let rows = stmt.query_map([project_id.as_str()], |row| {
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
                     WHERE t.project_id = ?1 AND st.finished_at IS NULL",
                    [project_id.as_str()],
                    |row| row.get(0),
                )?;
                let waiting: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM tasks t
                     JOIN project_statuses s
                       ON s.project_id = t.project_id AND s.id = t.status_id
                     WHERE t.project_id = ?1 AND s.kind = 'waiting'",
                    [project_id.as_str()],
                    |row| row.get(0),
                )?;
                let blocked: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM tasks t
                     JOIN project_statuses s
                       ON s.project_id = t.project_id AND s.id = t.status_id
                     WHERE t.project_id = ?1 AND s.kind = 'blocked'",
                    [project_id.as_str()],
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
        project: Option<String>,
        query: Option<String>,
    ) -> Result<Vec<TaskListItem>, AppError> {
        let name = require_project(project.as_deref())?.to_string();
        let query = require_field(query.as_deref(), "query")?.to_string();
        let view = self.project_view(&name).await?;
        let project_id = view.project.id;
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
                            (SELECT r.color FROM stages st
                              JOIN project_roles r
                                ON r.project_id = t.project_id AND r.name = st.role
                              WHERE st.task_id = t.id AND st.finished_at IS NULL
                              LIMIT 1),
                            t.waiting_for, t.updated_at
                     FROM tasks t
                     LEFT JOIN project_statuses s
                       ON s.project_id = t.project_id AND s.id = t.status_id
                     WHERE t.project_id = ?1
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
                let rows = stmt.query_map(rusqlite::params![project_id, fts_query], |row| {
                    Ok(TaskListItem {
                        id: row.get(0)?,
                        number: row.get(1)?,
                        folder_name: row.get(2)?,
                        title: row.get(3)?,
                        status_id: row.get(4)?,
                        status_title: row.get(5)?,
                        current_role: row.get(6)?,
                        current_role_color: row.get(7)?,
                        waiting_for: row.get(8)?,
                        updated_at: row.get(9)?,
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let path = require_field(req.path.as_deref(), "path")?.to_string();
        let title = require_field(req.title.as_deref(), "title")?.to_string();
        let kind = req.kind.unwrap_or_else(|| String::from("file"));
        let comment = req.comment.unwrap_or_default().trim().to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
        let task_id = task.id.clone();

        let root = std::fs::canonicalize(&project.root_path)
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
        let comment_for_write = comment.clone();

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
                    &comment_for_write,
                    &path_string,
                    None,
                    size,
                    &now,
                )?;
                let payload = serde_json::json!({
                    "path": &path_string,
                    "title": &title_for_write,
                    "comment": &comment_for_write
                })
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
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let file_id = require_field(req.file.as_deref(), "file")?.to_string();

        let view = self.project_view(&project_name).await?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
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

    pub async fn file_update(&self, req: FileUpdateRequest) -> Result<FileRecord, AppError> {
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let file_id = require_field(req.file.as_deref(), "file")?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();
        let comment = req.comment.unwrap_or_default().trim().to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let task = self.task(&view.project.id, &task_ref).await?;
        let task_id = task.id.clone();

        let found = self
            .store
            .read({
                let file_id = file_id.clone();
                move |conn| files::get_file(conn, &file_id)
            })
            .await;
        match found {
            Ok(file) if file.task_id == task_id => {}
            Ok(_) => return Err(AppError::not_found("Файл не принадлежит задаче")),
            Err(StoreError::NotFound) => return Err(AppError::not_found("Файл не найден")),
            Err(err) => return Err(err.into()),
        }

        let origin = event_origin(&role);
        let file = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let file = files::update_comment(&tx, &file_id, &comment)?;
                let payload = serde_json::json!({"comment": &comment}).to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "file_updated",
                    None,
                    Some(&role),
                    &payload,
                    origin,
                )?;
                tx.commit()?;
                Ok(file)
            })
            .await?;
        Ok(file)
    }

    pub async fn file_delete(&self, req: FileDeleteRequest) -> Result<(), AppError> {
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let file_id = require_field(req.file.as_deref(), "file")?.to_string();
        let role = require_role(req.role.as_deref())?.to_string();

        let view = self.project_view(&project_name).await?;
        ensure_role(&view.roles, &role)?;
        let project = view.project.clone();
        let task = self.task(&project.id, &task_ref).await?;
        let task_id = task.id.clone();

        let found = self
            .store
            .read({
                let file_id = file_id.clone();
                move |conn| files::get_file(conn, &file_id)
            })
            .await;
        match found {
            Ok(file) if file.task_id == task_id => {}
            Ok(_) => return Err(AppError::not_found("Файл не принадлежит задаче")),
            Err(StoreError::NotFound) => return Err(AppError::not_found("Файл не найден")),
            Err(err) => return Err(err.into()),
        }

        let origin = event_origin(&role);
        let removed = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let file = files::delete_file(&tx, &file_id)?;
                let payload = serde_json::json!({"path": &file.path}).to_string();
                events::insert_event(
                    &tx,
                    &task_id,
                    "file_removed",
                    None,
                    Some(&role),
                    &payload,
                    origin,
                )?;
                tx.commit()?;
                Ok(file)
            })
            .await?;

        let root = std::fs::canonicalize(&project.root_path).ok();
        let path = std::fs::canonicalize(&removed.path).ok();
        if let (Some(root), Some(path)) = (root, path) {
            if path.starts_with(&root) {
                if let Err(err) = std::fs::remove_file(&path) {
                    tracing::warn!(%err, path = %path.display(), "не удалось удалить файл");
                }
            }
        }
        Ok(())
    }

    pub async fn file_for_project(
        &self,
        project: &str,
        file_id: &str,
    ) -> Result<FileRecord, AppError> {
        let view = self.project_view(project).await?;
        let project_id = view.project.id;
        let file_id = file_id.to_string();

        let file = self
            .store
            .read(move |conn| {
                let file = files::get_file(conn, &file_id)?;
                let task = tasks::get_task_by_id(conn, &file.task_id)?;
                if task.project_id != project_id {
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

    async fn project_view(&self, name: &str) -> Result<ProjectView, AppError> {
        let name_owned = name.to_string();
        let (view, names) = self
            .store
            .read(move |conn| {
                let names = projects::list_project_names(conn)?;
                let view = match projects::project_view(conn, &name_owned) {
                    Ok(view) => Some(view),
                    Err(StoreError::NotFound) => None,
                    Err(err) => return Err(err),
                };
                Ok((view, names))
            })
            .await?;

        match view {
            Some(view) => Ok(view),
            None => Err(AppError::project_unknown(name, names)),
        }
    }

    async fn task(&self, project_id: &str, task_ref: &str) -> Result<Task, AppError> {
        let project_id = project_id.to_string();
        let task_ref_owned = task_ref.to_string();
        let result = self
            .store
            .read(move |conn| tasks::get_task(conn, &project_id, &task_ref_owned))
            .await;

        match result {
            Ok(task) => Ok(task),
            Err(StoreError::NotFound) => Err(AppError::task_unknown(task_ref)),
            Err(err) => Err(err.into()),
        }
    }

    async fn resolve_inputs(
        &self,
        project: &Project,
        task_id: &str,
        role: &str,
        round: i64,
    ) -> Result<Vec<RoleInput>, AppError> {
        let project = project.clone();
        let task_id = task_id.to_string();
        let role = role.to_string();
        let inputs = self
            .store
            .read(move |conn| inputs::resolve(conn, &project, &task_id, &role, round))
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

/// Цвет роли: принимаем только `#rgb` или `#rrggbb`, иначе считаем, что цвета нет.
fn normalize_color(color: Option<&str>) -> Option<String> {
    let value = color?.trim();
    let digits = match value.len() {
        4 | 7 => &value[1..],
        _ => return None,
    };
    if !value.starts_with('#') || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some(value.to_string())
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

fn require_project(value: Option<&str>) -> Result<&str, AppError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Ok(value),
        None => Err(AppError::project_missing()),
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
