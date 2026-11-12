#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use pithos_pandoras::domain::ErrorCode;
use pithos_pandoras::service::{
    FileAddRequest, FileGetRequest, ReportAddRequest, ReportPutRequest, Service,
    StageFinishRequest, StageStartRequest, TaskBlockRequest, TaskCloseRequest, TaskCreateRequest,
    TaskResumeRequest,
};
use pithos_pandoras::store::{self, Store};
use tempfile::TempDir;

async fn setup(config: &str) -> (Arc<Service>, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(Store::open(dir.path().join("test.db")).unwrap());
    let root = dir.path().to_string_lossy().to_string();
    let roles = vec![
        String::from("Коммандер"),
        String::from("Исполнитель"),
        String::from("Проверяющий"),
    ];
    let config = config.to_string();

    store
        .write(move |conn| {
            store::workspaces::init_workspace(
                conn,
                "PithosPandoras",
                &root,
                "Задачи",
                "{number} - {date}",
                &roles,
                &config,
            )
        })
        .await
        .unwrap();

    (Arc::new(Service::new(store)), dir)
}

async fn create_task(service: &Service, title: &str) -> pithos_pandoras::domain::TaskCreated {
    service
        .task_create(TaskCreateRequest {
            workspace: Some(String::from("PithosPandoras")),
            role: Some(String::from("Коммандер")),
            title: Some(title.to_string()),
            statement: Some(String::from("постановка")),
            route: None,
            resource: None,
        })
        .await
        .unwrap()
}

fn start_req(task: &str, role: &str) -> StageStartRequest {
    StageStartRequest {
        workspace: Some(String::from("PithosPandoras")),
        task: Some(task.to_string()),
        role: Some(role.to_string()),
    }
}

#[tokio::test]
async fn core_cycle_creates_numbers_and_closes_task() {
    let (service, _dir) = setup("{}").await;

    let first = create_task(&service, "Первая").await;
    let second = create_task(&service, "Вторая").await;
    assert_eq!(first.task.number, 1);
    assert_eq!(second.task.number, 2);
    assert!(first.folder_path.contains("001"));

    let started = service
        .stage_start(start_req("1", "Исполнитель"))
        .await
        .unwrap();
    assert_eq!(started.stage.round, 1);
    assert!(started.inputs.is_empty());

    let conflict = service
        .stage_start(start_req("1", "Исполнитель"))
        .await
        .unwrap_err();
    assert_eq!(conflict.code, ErrorCode::Conflict);

    let finished = service
        .stage_finish(StageFinishRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(String::from("1")),
            role: Some(String::from("Исполнитель")),
            verdict: Some(String::from("готово")),
            report: Some(String::from("сделано")),
            kind: None,
        })
        .await
        .unwrap();
    assert!(finished.stage.finished_at.is_some());
    assert_eq!(finished.report.kind, "report");

    let note = service
        .report_add(ReportAddRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(String::from("1")),
            role: Some(String::from("Исполнитель")),
            kind: Some(String::from("note")),
            title: Some(String::from("Заметка")),
            content: Some(String::from("текст заметки")),
        })
        .await
        .unwrap();
    assert_eq!(note.kind, "note");

    let card = service
        .task_get(
            Some(String::from("PithosPandoras")),
            Some(String::from("1")),
        )
        .await
        .unwrap();
    assert!(card.current_stage.is_none());
    assert_eq!(card.reports.len(), 2);
    assert!(card.warnings.contains(&String::from("folder_missing")));

    let closed = service
        .task_close(TaskCloseRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(String::from("1")),
            role: Some(String::from("Коммандер")),
        })
        .await
        .unwrap();
    assert_eq!(closed.status_id, "done");
}

#[tokio::test]
async fn strict_errors_name_the_problem() {
    let (service, _dir) = setup("{}").await;

    let missing_workspace = service
        .task_create(TaskCreateRequest {
            workspace: None,
            role: Some(String::from("Коммандер")),
            title: Some(String::from("Без пространства")),
            statement: Some(String::from("постановка")),
            route: None,
            resource: None,
        })
        .await
        .unwrap_err();
    assert_eq!(missing_workspace.code, ErrorCode::WorkspaceMissing);

    let unknown_workspace = service
        .workspace_get(Some(String::from("nope")))
        .await
        .unwrap_err();
    assert_eq!(unknown_workspace.code, ErrorCode::WorkspaceUnknown);
    assert!(unknown_workspace.allowed.is_some());

    let unknown_role = service
        .task_create(TaskCreateRequest {
            workspace: Some(String::from("PithosPandoras")),
            role: Some(String::from("Плотник")),
            title: Some(String::from("Без роли")),
            statement: Some(String::from("постановка")),
            route: None,
            resource: None,
        })
        .await
        .unwrap_err();
    assert_eq!(unknown_role.code, ErrorCode::RoleUnknown);

    let missing_title = service
        .task_create(TaskCreateRequest {
            workspace: Some(String::from("PithosPandoras")),
            role: Some(String::from("Коммандер")),
            title: None,
            statement: Some(String::from("постановка")),
            route: None,
            resource: None,
        })
        .await
        .unwrap_err();
    assert_eq!(missing_title.code, ErrorCode::FieldMissing);
}

#[tokio::test]
async fn role_inputs_take_latest_finished_report() {
    let config = r#"{"inputs":{"Проверяющий":[{"from":"Исполнитель","kind":"report","round":"latest","required":true}]}}"#;
    let (service, _dir) = setup(config).await;
    let task = create_task(&service, "С входом").await;
    let task_ref = task.task.number.to_string();

    let missing = service
        .stage_start(start_req(&task_ref, "Проверяющий"))
        .await
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::InputMissing);

    service
        .stage_start(start_req(&task_ref, "Исполнитель"))
        .await
        .unwrap();
    service
        .stage_finish(StageFinishRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref.clone()),
            role: Some(String::from("Исполнитель")),
            verdict: Some(String::from("ok")),
            report: Some(String::from("отчёт исполнителя")),
            kind: None,
        })
        .await
        .unwrap();

    let started = service
        .stage_start(start_req(&task_ref, "Проверяющий"))
        .await
        .unwrap();
    assert_eq!(started.inputs.len(), 1);
    assert_eq!(started.inputs[0].role, "Исполнитель");
    assert_eq!(started.inputs[0].round, 1);
}

#[tokio::test]
async fn report_put_adds_second_kind_to_open_stage() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "С планом").await;
    let task_ref = task.task.number.to_string();

    service
        .stage_start(start_req(&task_ref, "Исполнитель"))
        .await
        .unwrap();

    let plan = service
        .report_put(ReportPutRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref.clone()),
            role: Some(String::from("Исполнитель")),
            kind: Some(String::from("plan")),
            title: Some(String::from("План")),
            content: Some(String::from("шаги")),
        })
        .await
        .unwrap();
    assert_eq!(plan.kind, "plan");

    let duplicate = service
        .report_put(ReportPutRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref),
            role: Some(String::from("Исполнитель")),
            kind: Some(String::from("plan")),
            title: Some(String::from("План 2")),
            content: Some(String::from("ещё шаги")),
        })
        .await
        .unwrap_err();
    assert_eq!(duplicate.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn return_input_takes_report_that_sent_task_back() {
    let config = r#"{"inputs":{"Исполнитель":[{"from":"Проверяющий","kind":"report","round":"return","required":true}]}}"#;
    let (service, _dir) = setup(config).await;
    let task = create_task(&service, "С возвратом").await;
    let task_ref = task.task.number.to_string();

    service
        .stage_start(start_req(&task_ref, "Проверяющий"))
        .await
        .unwrap();
    service
        .stage_finish(StageFinishRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref.clone()),
            role: Some(String::from("Проверяющий")),
            verdict: Some(String::from("return")),
            report: Some(String::from("верни на доработку")),
            kind: None,
        })
        .await
        .unwrap();

    let started = service
        .stage_start(start_req(&task_ref, "Исполнитель"))
        .await
        .unwrap();
    assert_eq!(started.inputs.len(), 1);
    assert_eq!(started.inputs[0].role, "Проверяющий");
    assert_eq!(started.inputs[0].round, 1);
    assert_eq!(started.inputs[0].kind, "report");
}

#[tokio::test]
async fn parallel_task_create_gets_unique_numbers() {
    let (service, _dir) = setup("{}").await;

    let (first, second) = tokio::join!(
        create_task(&service, "Параллельная A"),
        create_task(&service, "Параллельная B")
    );

    let mut numbers = vec![first.task.number, second.task.number];
    numbers.sort();
    assert_eq!(numbers, vec![1, 2]);
}

#[tokio::test]
async fn parallel_stage_start_returns_conflict() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "Гонка").await;
    let task_ref = task.task.number.to_string();

    let (first, second) = tokio::join!(
        service.stage_start(start_req(&task_ref, "Исполнитель")),
        service.stage_start(start_req(&task_ref, "Исполнитель"))
    );

    let successes = u8::from(first.is_ok()) + u8::from(second.is_ok());
    assert_eq!(successes, 1);
    let error = first.err().or_else(|| second.err()).unwrap();
    assert_eq!(error.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn block_finishes_stage_and_resume_clears_waiting() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "Блок").await;
    let task_ref = task.task.number.to_string();

    service
        .stage_start(start_req(&task_ref, "Исполнитель"))
        .await
        .unwrap();

    let blocked = service
        .task_block(TaskBlockRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref.clone()),
            role: Some(String::from("Исполнитель")),
            reason: Some(String::from("нужны данные")),
            waiting_for: Some(String::from("человек")),
            waiting_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(blocked.status_id, "waiting");
    assert_eq!(blocked.waiting_for, "человек");

    let card = service
        .task_get(Some(String::from("PithosPandoras")), Some(task_ref.clone()))
        .await
        .unwrap();
    assert!(card.current_stage.is_none());

    let resumed = service
        .task_resume(TaskResumeRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref),
            role: Some(String::from("Исполнитель")),
            reason: Some(String::from("данные есть")),
        })
        .await
        .unwrap();
    assert_eq!(resumed.status_id, "active");
    assert_eq!(resumed.waiting_for, "");
}

#[tokio::test]
async fn file_add_stays_inside_project_root() {
    let (service, dir) = setup("{}").await;
    let task = create_task(&service, "Файл").await;
    let task_ref = task.task.number.to_string();

    let inside = dir.path().join("note.txt");
    std::fs::write(&inside, b"hello").unwrap();
    let added = service
        .file_add(FileAddRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref.clone()),
            role: Some(String::from("Исполнитель")),
            path: Some(inside.to_string_lossy().to_string()),
            title: Some(String::from("Заметка")),
            kind: None,
        })
        .await
        .unwrap();
    assert!(added.url.starts_with("/files/"));

    let record = service
        .file_get(FileGetRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref.clone()),
            file: Some(added.file_id),
        })
        .await
        .unwrap();
    assert_eq!(record.size, Some(5));

    let outside = std::env::temp_dir().join("pithos-outside.txt");
    std::fs::write(&outside, b"x").unwrap();
    let error = service
        .file_add(FileAddRequest {
            workspace: Some(String::from("PithosPandoras")),
            task: Some(task_ref),
            role: Some(String::from("Исполнитель")),
            path: Some(outside.to_string_lossy().to_string()),
            title: Some(String::from("Снаружи")),
            kind: None,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn search_finds_title_and_report_text() {
    let (service, _dir) = setup("{}").await;
    create_task(&service, "Уникальное имя").await;

    let found = service
        .task_search(
            Some(String::from("PithosPandoras")),
            Some(String::from("Уникальное")),
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
}
