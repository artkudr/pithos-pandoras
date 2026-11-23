#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use pithos_pandoras::domain::ErrorCode;
use pithos_pandoras::service::{
    ReportOpenRequest, ReportPublishRequest, ReportPutRequest, ReportSectionRequest, Service,
    StageFinishRequest, StageNoteRequest, StageStartRequest, TaskCreateRequest,
};
use pithos_pandoras::store::{self, Store};
use tempfile::TempDir;

async fn setup() -> (Arc<Service>, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(Store::open(dir.path().join("test.db")).unwrap());
    let root = dir.path().to_string_lossy().to_string();
    let roles = vec![
        (String::from("Коммандер"), None),
        (String::from("Исполнитель"), None),
        (String::from("Проверяющий"), None),
    ];
    store
        .write(move |conn| {
            store::projects::init_project(
                conn,
                "PithosPandoras",
                &root,
                "Задачи",
                "{number} - {date}",
                &roles,
                "{}",
            )
        })
        .await
        .unwrap();
    (Arc::new(Service::new(store)), dir)
}

async fn create_task(service: &Service) -> String {
    let created = service
        .task_create(TaskCreateRequest {
            project: Some(String::from("PithosPandoras")),
            role: Some(String::from("Коммандер")),
            title: Some(String::from("Задача про этап")),
            statement: Some(String::from("постановка")),
            route: None,
            resource: None,
        })
        .await
        .unwrap();
    created.task.number.to_string()
}

async fn start_stage(service: &Service, task: &str) -> pithos_pandoras::domain::StageStart {
    service
        .stage_start(StageStartRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.to_string()),
            role: Some(String::from("Исполнитель")),
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn stage_get_shows_open_then_finished_stage() {
    let (service, _dir) = setup().await;
    let task = create_task(&service).await;
    let started = start_stage(&service, &task).await;

    let status = service
        .stage_get(Some(String::from("PithosPandoras")), Some(task.clone()))
        .await
        .unwrap();
    assert!(status.is_open);
    assert_eq!(status.stage.unwrap().id, started.stage.id);
    assert!(status.inputs.is_empty());
    assert!(status.reports.is_empty());
    assert!(status.drafts.is_empty());

    let draft = service
        .report_open(ReportOpenRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            role: Some(String::from("Исполнитель")),
            kind: Some(String::from("report")),
            title: None,
        })
        .await
        .unwrap();
    service
        .report_section(ReportSectionRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            key: Some(String::from("summary")),
            title: None,
            body: Some(String::from("Этап завершён, замечаний нет.")),
            order: None,
            expected_revision: None,
        })
        .await
        .unwrap();
    let published = service
        .report_publish(ReportPublishRequest {
            draft_id: Some(draft.draft_id),
            role: Some(String::from("Исполнитель")),
            expected_revision: None,
        })
        .await
        .unwrap();
    service
        .stage_finish(StageFinishRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            role: Some(String::from("Исполнитель")),
            verdict: Some(String::from("готово")),
            report_id: Some(published.report_id.clone()),
            kind: None,
        })
        .await
        .unwrap();

    let status = service
        .stage_get(Some(String::from("PithosPandoras")), Some(task))
        .await
        .unwrap();
    assert!(!status.is_open);
    assert_eq!(status.stage.unwrap().id, started.stage.id);
    assert_eq!(status.reports.len(), 1);
    assert_eq!(status.reports[0].report_id, published.report_id);
    assert!(status.drafts.is_empty());
}

#[tokio::test]
async fn stage_note_updates_card_and_rejects_other_role() {
    let (service, _dir) = setup().await;
    let task = create_task(&service).await;
    start_stage(&service, &task).await;

    let stage = service
        .stage_note(StageNoteRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            role: None,
            text: Some(String::from("Жду освобождения блокировки базы.")),
        })
        .await
        .unwrap();
    assert!(stage.note.contains("Жду освобождения"));
    assert!(stage.last_activity_at.is_some());

    let card = service
        .task_get(Some(String::from("PithosPandoras")), Some(task.clone()))
        .await
        .unwrap();
    let current = card.current_stage.unwrap();
    assert!(current.note.contains("Жду освобождения"));
    assert!(current.last_activity_at.is_some());

    let error = service
        .stage_note(StageNoteRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task),
            role: Some(String::from("Проверяющий")),
            text: Some(String::from("Чужая заметка.")),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn report_put_infers_role_from_open_stage() {
    let (service, _dir) = setup().await;
    let task = create_task(&service).await;
    start_stage(&service, &task).await;

    let report = service
        .report_put(ReportPutRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            role: None,
            kind: Some(String::from("guide")),
            title: Some(String::from("Короткий гайд")),
            content: Some(String::from("Гайд по работе с задачей.")),
        })
        .await
        .unwrap();
    assert_eq!(report.kind, "guide");

    let card = service
        .task_get(Some(String::from("PithosPandoras")), Some(task))
        .await
        .unwrap();
    assert_eq!(card.reports.len(), 1);
}
