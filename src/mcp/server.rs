//! MCP-сервер: инструменты для агентов.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header::AUTHORIZATION, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ErrorCode as McpErrorCode, ErrorData};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{tool, tool_handler, tool_router, Json, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::domain::{
    AppError, Comment, ErrorCode as AppErrorCode, Event, FileAdded, FileRecord, NextStep, Report,
    StageFinish, StageStart, Task, TaskCard, TaskCreated, TaskListItem, TaskStats, WorkspaceView,
};
use crate::service::{
    FileAddRequest, FileGetRequest, ReportAddRequest, ReportGetRequest, ReportPutRequest, Service,
    StageFinishRequest, StageStartRequest, TaskBlockRequest, TaskCloseRequest, TaskCreateRequest,
    TaskNoteRequest, TaskReopenRequest, TaskResumeRequest, TaskSetStatusRequest, TaskUpdateRequest,
};

#[derive(Clone)]
pub struct PithosServer {
    service: Arc<Service>,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorkspaceGetArgs {
    pub workspace: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskCreateArgs {
    pub workspace: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
    pub route: Option<String>,
    pub resource: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskListArgs {
    pub workspace: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskGetArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StageStartArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StageFinishArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub verdict: Option<String>,
    pub report: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportAddArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportGetArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub report: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportPutArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskCloseArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskBlockArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
    pub waiting_for: Option<String>,
    pub waiting_ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskResumeArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskReopenArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskNoteArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskTimelineArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskUpdateArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskSetStatusArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskNextArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskStatsArgs {
    pub workspace: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskSearchArgs {
    pub workspace: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FileAddArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
    pub path: Option<String>,
    pub title: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FileGetArgs {
    pub workspace: Option<String>,
    pub task: Option<String>,
    pub file: Option<String>,
}

#[tool_router(router = tool_router)]
impl PithosServer {
    pub fn new(service: Arc<Service>) -> Self {
        Self {
            service,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "workspace_get",
        description = "Настройки проекта: роли, статусы, config"
    )]
    pub async fn workspace_get(
        &self,
        Parameters(args): Parameters<WorkspaceGetArgs>,
    ) -> Result<Json<WorkspaceView>, ErrorData> {
        self.service
            .workspace_get(args.workspace)
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(
        name = "task_create",
        description = "Создаёт задачу, выдаёт номер и путь папки"
    )]
    pub async fn task_create(
        &self,
        Parameters(args): Parameters<TaskCreateArgs>,
    ) -> Result<Json<TaskCreated>, ErrorData> {
        self.service
            .task_create(TaskCreateRequest {
                workspace: args.workspace,
                role: args.role,
                title: args.title,
                statement: args.statement,
                route: args.route,
                resource: args.resource,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_list", description = "Список задач проекта")]
    pub async fn task_list(
        &self,
        Parameters(args): Parameters<TaskListArgs>,
    ) -> Result<Json<Vec<TaskListItem>>, ErrorData> {
        self.service
            .task_list(args.workspace)
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(
        name = "task_get",
        description = "Карточка задачи: постановка, этап, отчёты, файлы"
    )]
    pub async fn task_get(
        &self,
        Parameters(args): Parameters<TaskGetArgs>,
    ) -> Result<Json<TaskCard>, ErrorData> {
        self.service
            .task_get(args.workspace, args.task)
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(
        name = "stage_start",
        description = "Открывает этап роли и отдаёт входы"
    )]
    pub async fn stage_start(
        &self,
        Parameters(args): Parameters<StageStartArgs>,
    ) -> Result<Json<StageStart>, ErrorData> {
        self.service
            .stage_start(StageStartRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(
        name = "stage_finish",
        description = "Закрывает этап, пишет главный отчёт и вердикт"
    )]
    pub async fn stage_finish(
        &self,
        Parameters(args): Parameters<StageFinishArgs>,
    ) -> Result<Json<StageFinish>, ErrorData> {
        self.service
            .stage_finish(StageFinishRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                verdict: args.verdict,
                report: args.report,
                kind: args.kind,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(
        name = "report_add",
        description = "Добавляет отчёт к задаче без этапа"
    )]
    pub async fn report_add(
        &self,
        Parameters(args): Parameters<ReportAddArgs>,
    ) -> Result<Json<Report>, ErrorData> {
        self.service
            .report_add(ReportAddRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                kind: args.kind,
                title: args.title,
                content: args.content,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "report_get", description = "Полный текст отчёта")]
    pub async fn report_get(
        &self,
        Parameters(args): Parameters<ReportGetArgs>,
    ) -> Result<Json<Report>, ErrorData> {
        self.service
            .report_get(ReportGetRequest {
                workspace: args.workspace,
                task: args.task,
                report: args.report,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(
        name = "report_put",
        description = "Добавляет отчёт другого вида в открытый этап"
    )]
    pub async fn report_put(
        &self,
        Parameters(args): Parameters<ReportPutArgs>,
    ) -> Result<Json<Report>, ErrorData> {
        self.service
            .report_put(ReportPutRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                kind: args.kind,
                title: args.title,
                content: args.content,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_close", description = "Закрывает задачу")]
    pub async fn task_close(
        &self,
        Parameters(args): Parameters<TaskCloseArgs>,
    ) -> Result<Json<Task>, ErrorData> {
        self.service
            .task_close(TaskCloseRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(
        name = "task_block",
        description = "Останавливает задачу или ставит в ожидание"
    )]
    pub async fn task_block(
        &self,
        Parameters(args): Parameters<TaskBlockArgs>,
    ) -> Result<Json<Task>, ErrorData> {
        self.service
            .task_block(TaskBlockRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                reason: args.reason,
                waiting_for: args.waiting_for,
                waiting_ref: args.waiting_ref,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_resume", description = "Снимает остановку или ожидание")]
    pub async fn task_resume(
        &self,
        Parameters(args): Parameters<TaskResumeArgs>,
    ) -> Result<Json<Task>, ErrorData> {
        self.service
            .task_resume(TaskResumeRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                reason: args.reason,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_reopen", description = "Переоткрывает закрытую задачу")]
    pub async fn task_reopen(
        &self,
        Parameters(args): Parameters<TaskReopenArgs>,
    ) -> Result<Json<Task>, ErrorData> {
        self.service
            .task_reopen(TaskReopenRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                reason: args.reason,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_note", description = "Добавляет комментарий к задаче")]
    pub async fn task_note(
        &self,
        Parameters(args): Parameters<TaskNoteArgs>,
    ) -> Result<Json<Comment>, ErrorData> {
        self.service
            .task_note(TaskNoteRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                text: args.text,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_timeline", description = "События задачи")]
    pub async fn task_timeline(
        &self,
        Parameters(args): Parameters<TaskTimelineArgs>,
    ) -> Result<Json<Vec<Event>>, ErrorData> {
        self.service
            .task_timeline(args.workspace, args.task)
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(
        name = "task_update",
        description = "Правит название или постановку задачи"
    )]
    pub async fn task_update(
        &self,
        Parameters(args): Parameters<TaskUpdateArgs>,
    ) -> Result<Json<Task>, ErrorData> {
        self.service
            .task_update(TaskUpdateRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                title: args.title,
                statement: args.statement,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_set_status", description = "Меняет статус задачи")]
    pub async fn task_set_status(
        &self,
        Parameters(args): Parameters<TaskSetStatusArgs>,
    ) -> Result<Json<Task>, ErrorData> {
        self.service
            .task_set_status(TaskSetStatusRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                status: args.status,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_next", description = "Подсказка, что делать дальше")]
    pub async fn task_next(
        &self,
        Parameters(args): Parameters<TaskNextArgs>,
    ) -> Result<Json<NextStep>, ErrorData> {
        self.service
            .task_next(args.workspace, args.task)
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_stats", description = "Агрегаты проекта")]
    pub async fn task_stats(
        &self,
        Parameters(args): Parameters<TaskStatsArgs>,
    ) -> Result<Json<TaskStats>, ErrorData> {
        self.service
            .task_stats(args.workspace)
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "task_search", description = "Поиск по текстам задач и отчётов")]
    pub async fn task_search(
        &self,
        Parameters(args): Parameters<TaskSearchArgs>,
    ) -> Result<Json<Vec<TaskListItem>>, ErrorData> {
        self.service
            .task_search(args.workspace, args.query)
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "file_add", description = "Регистрирует файл и отдаёт ссылку")]
    pub async fn file_add(
        &self,
        Parameters(args): Parameters<FileAddArgs>,
    ) -> Result<Json<FileAdded>, ErrorData> {
        self.service
            .file_add(FileAddRequest {
                workspace: args.workspace,
                task: args.task,
                role: args.role,
                path: args.path,
                title: args.title,
                kind: args.kind,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }

    #[tool(name = "file_get", description = "Путь и ссылка на файл")]
    pub async fn file_get(
        &self,
        Parameters(args): Parameters<FileGetArgs>,
    ) -> Result<Json<FileRecord>, ErrorData> {
        self.service
            .file_get(FileGetRequest {
                workspace: args.workspace,
                task: args.task,
                file: args.file,
            })
            .await
            .map(Json)
            .map_err(Into::into)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PithosServer {}

impl From<AppError> for ErrorData {
    fn from(err: AppError) -> Self {
        let code = match err.code {
            AppErrorCode::WorkspaceMissing
            | AppErrorCode::RoleMissing
            | AppErrorCode::TaskMissing
            | AppErrorCode::FieldMissing
            | AppErrorCode::InputMissing
            | AppErrorCode::Conflict => McpErrorCode::INVALID_PARAMS,
            AppErrorCode::WorkspaceUnknown
            | AppErrorCode::RoleUnknown
            | AppErrorCode::TaskUnknown
            | AppErrorCode::NotFound => McpErrorCode::RESOURCE_NOT_FOUND,
            AppErrorCode::Io => McpErrorCode::INTERNAL_ERROR,
        };
        let data = serde_json::to_value(&err).ok();
        ErrorData::new(code, err.message, data)
    }
}

pub fn mcp_service(
    service: Arc<Service>,
) -> StreamableHttpService<PithosServer, LocalSessionManager> {
    StreamableHttpService::new(
        move || Ok(PithosServer::new(service.clone())),
        Default::default(),
        StreamableHttpServerConfig::default().with_json_response(true),
    )
}

#[derive(Clone)]
struct AppState {
    key: Arc<String>,
}

async fn require_key(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let authorized = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|value| value == state.key.as_str())
        .unwrap_or(false);

    if authorized {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

pub fn router(service: Arc<Service>, key: String) -> axum::Router {
    let state = AppState { key: Arc::new(key) };
    let protected = axum::Router::new()
        .nest_service("/mcp", mcp_service(service))
        .layer(middleware::from_fn_with_state(state, require_key));

    axum::Router::new()
        .route("/health", axum::routing::get(health))
        .merge(protected)
}

async fn health() -> &'static str {
    "ok"
}
