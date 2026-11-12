#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use pithos_pandoras::domain::ErrorCode;
use pithos_pandoras::service::{
    FileAddRequest, FileGetRequest, ReportAddRequest, ReportPutRequest, Service,
    StageFinishRequest, StageStartRequest, TaskBlockRequest, TaskCloseRequest, TaskCreateRequest,
    TaskNoteRequest, TaskResumeRequest, TaskSetStatusRequest,
};
use pithos_pandoras::store::{self, Store};
use tempfile::TempDir;

async fn setup(config: &str) -> (Arc<Service>, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(Store::open(dir.path().join("test.db")).unwrap());
    let root = dir.path().to_string_lossy().to_string();
    let roles = vec![
        (String::from("Коммандер"), None),
        (String::from("Исполнитель"), None),
        (String::from("Проверяющий"), None),
    ];
    let config = config.to_string();

    store
        .write(move |conn| {
            store::projects::init_project(
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
            project: Some(String::from("PithosPandoras")),
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
        project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
            task: Some(String::from("1")),
            role: Some(String::from("Коммандер")),
        })
        .await
        .unwrap();
    assert_eq!(closed.status_id, "done");
}

#[tokio::test]
async fn human_creates_task_with_human_origin() {
    let (service, _dir) = setup("{}").await;
    let created = service
        .human_task_create("PithosPandoras", "Человеческая", "постановка", None, None)
        .await
        .unwrap();
    assert_eq!(created.task.origin, "human");
    assert!(std::path::Path::new(&created.folder_path).exists());

    let timeline = service
        .task_timeline(
            Some(String::from("PithosPandoras")),
            Some(created.task.number.to_string()),
        )
        .await
        .unwrap();
    assert_eq!(timeline[0].actor.as_deref(), Some("человек"));
    assert_eq!(timeline[0].origin, "human");
}

#[tokio::test]
async fn human_note_and_block_use_human_actor() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "Человеческие действия").await;
    let task_ref = task.task.number.to_string();

    let comment = service
        .task_note(TaskNoteRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task_ref.clone()),
            role: Some(String::from("человек")),
            text: Some(String::from("привет")),
        })
        .await
        .unwrap();
    assert_eq!(comment.author, "человек");

    service
        .stage_start(start_req(&task_ref, "Исполнитель"))
        .await
        .unwrap();
    let blocked = service
        .task_block(TaskBlockRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task_ref),
            role: Some(String::from("человек")),
            reason: Some(String::from("стоп")),
            waiting_for: None,
            waiting_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(blocked.status_id, "blocked");
}

#[tokio::test]
async fn ensure_task_folder_creates_folder_for_agent_task() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "Папка").await;
    let task_ref = task.task.number.to_string();

    let folder = service
        .ensure_task_folder("PithosPandoras", &task_ref)
        .await
        .unwrap();
    assert!(std::path::Path::new(&folder).exists());

    let card = service
        .task_get(Some(String::from("PithosPandoras")), Some(task_ref))
        .await
        .unwrap();
    assert!(!card
        .warnings
        .iter()
        .any(|warning| warning.as_str() == "folder_missing"));
}

#[tokio::test]
async fn strict_errors_name_the_problem() {
    let (service, _dir) = setup("{}").await;

    let missing_project = service
        .task_create(TaskCreateRequest {
            project: None,
            role: Some(String::from("Коммандер")),
            title: Some(String::from("Без проекта")),
            statement: Some(String::from("постановка")),
            route: None,
            resource: None,
        })
        .await
        .unwrap_err();
    assert_eq!(missing_project.code, ErrorCode::ProjectMissing);

    let unknown_project = service
        .project_get(Some(String::from("nope")))
        .await
        .unwrap_err();
    assert_eq!(unknown_project.code, ErrorCode::ProjectUnknown);
    assert!(unknown_project.allowed.is_some());

    let unknown_role = service
        .task_create(TaskCreateRequest {
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
            project: Some(String::from("PithosPandoras")),
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
async fn set_status_done_without_open_stage_closes_task() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "Закрыть статусом").await;
    let task_ref = task.task.number.to_string();

    let closed = service
        .task_set_status(TaskSetStatusRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task_ref),
            role: Some(String::from("Коммандер")),
            status: Some(String::from("done")),
        })
        .await
        .unwrap();
    assert_eq!(closed.status_id, "done");
    assert!(closed.closed_at.is_some());
}

#[tokio::test]
async fn block_without_waiting_does_not_set_waiting_since() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "Блок без ожидания").await;
    let task_ref = task.task.number.to_string();

    let blocked = service
        .task_block(TaskBlockRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task_ref),
            role: Some(String::from("Коммандер")),
            reason: Some(String::from("стоп")),
            waiting_for: None,
            waiting_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(blocked.status_id, "blocked");
    assert!(blocked.waiting_since.is_none());
}

#[tokio::test]
async fn search_finds_title_and_report_text() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "Уникальное имя").await;
    let task_ref = task.task.number.to_string();

    service
        .report_add(ReportAddRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task_ref),
            role: Some(String::from("Исполнитель")),
            kind: Some(String::from("note")),
            title: Some(String::from("Заметка")),
            content: Some(String::from("в отчёте есть апельсин")),
        })
        .await
        .unwrap();

    let by_title = service
        .task_search(
            Some(String::from("PithosPandoras")),
            Some(String::from("Уникальное")),
        )
        .await
        .unwrap();
    assert_eq!(by_title.len(), 1);

    let by_report = service
        .task_search(
            Some(String::from("PithosPandoras")),
            Some(String::from("апельсин")),
        )
        .await
        .unwrap();
    assert_eq!(by_report.len(), 1);
}

#[tokio::test]
async fn set_status_done_with_open_stage_is_conflict() {
    let (service, _dir) = setup("{}").await;
    let task = create_task(&service, "Статус").await;
    let task_ref = task.task.number.to_string();

    service
        .stage_start(start_req(&task_ref, "Исполнитель"))
        .await
        .unwrap();

    let error = service
        .task_set_status(TaskSetStatusRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task_ref),
            role: Some(String::from("Коммандер")),
            status: Some(String::from("done")),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn backup_makes_readable_copy() {
    let (service, dir) = setup("{}").await;
    create_task(&service, "Копия").await;

    let store = Store::open(dir.path().join("test.db")).unwrap();
    let out = dir.path().join("backups");
    let path = pithos_pandoras::cli::backup::backup_to(&store, &out)
        .await
        .unwrap();

    assert!(path.exists());
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let tasks: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(tasks, 1);
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}

#[tokio::test]
async fn live_reports_change_after_write() {
    use std::time::Duration;

    let (service, dir) = setup("{}").await;
    let live = pithos_pandoras::web::live::Live::spawn_every(
        dir.path().join("test.db"),
        Duration::from_millis(20),
    );
    let before = live.revision();

    create_task(&service, "Живая").await;

    let changed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if live.revision() != before {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(changed.is_ok(), "поток изменений молчит");
}
