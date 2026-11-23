#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use pithos_pandoras::domain::ErrorCode;
use pithos_pandoras::service::{
    ReportDiscardRequest, ReportGetRequest, ReportOpenRequest, ReportPublishRequest,
    ReportSectionRequest, Service, StageFinishRequest, StageStartRequest, TaskCreateRequest,
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

async fn start_task(service: &Service) -> String {
    let created = service
        .task_create(TaskCreateRequest {
            project: Some(String::from("PithosPandoras")),
            role: Some(String::from("Коммандер")),
            title: Some(String::from("Отчётная задача")),
            statement: Some(String::from("постановка")),
            route: None,
            resource: None,
        })
        .await
        .unwrap();
    let number = created.task.number.to_string();
    service
        .stage_start(StageStartRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(number.clone()),
            role: Some(String::from("Исполнитель")),
        })
        .await
        .unwrap();
    number
}

fn open_req(task: &str, kind: &str) -> ReportOpenRequest {
    ReportOpenRequest {
        project: Some(String::from("PithosPandoras")),
        task: Some(task.to_string()),
        role: Some(String::from("Исполнитель")),
        kind: Some(kind.to_string()),
        title: Some(String::from("Отчёт исполнителя")),
    }
}

#[tokio::test]
async fn draft_sections_replace_and_publish_once() {
    let (service, _dir) = setup().await;
    let task = start_task(&service).await;

    let draft = service
        .report_open(open_req(&task, "report"))
        .await
        .unwrap();
    assert_eq!(draft.revision, 0);

    let first = service
        .report_section(ReportSectionRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            key: Some(String::from("summary")),
            title: None,
            body: Some(String::from("первый вариант итога")),
            order: None,
            expected_revision: None,
        })
        .await
        .unwrap();
    assert_eq!(first.revision, 1);

    let second = service
        .report_section(ReportSectionRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            key: Some(String::from("summary")),
            title: None,
            body: Some(String::from("второй вариант итога, уже проверенный")),
            order: None,
            expected_revision: Some(1),
        })
        .await
        .unwrap();
    assert_eq!(second.revision, 2);

    let document = service
        .report_draft(Some(draft.draft_id.clone()))
        .await
        .unwrap();
    assert_eq!(document.blocks.len(), 1);
    assert!(document.rendered.contains("второй вариант"));
    assert!(!document.rendered.contains("первый вариант"));

    let published = service
        .report_publish(ReportPublishRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            expected_revision: Some(2),
        })
        .await
        .unwrap();
    let repeated = service
        .report_publish(ReportPublishRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            expected_revision: None,
        })
        .await
        .unwrap();
    assert_eq!(published.report_id, repeated.report_id);

    let fetched = service
        .report_get(ReportGetRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            report_id: Some(published.report_id.clone()),
        })
        .await
        .unwrap();
    assert_eq!(fetched.report_id, published.report_id);

    let missing = service
        .report_get(ReportGetRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            report_id: None,
        })
        .await
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::FieldMissing);
    let allowed = missing.allowed.unwrap();
    assert!(allowed
        .iter()
        .any(|value| value.contains(&published.report_id)));

    let wrong = service
        .report_get(ReportGetRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            report_id: Some(String::from("no-such-report")),
        })
        .await
        .unwrap_err();
    assert_eq!(wrong.code, ErrorCode::NotFound);
    assert!(wrong
        .allowed
        .unwrap()
        .iter()
        .any(|value| value.contains(&published.report_id)));

    let finished = service
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
    assert_eq!(finished.report.report_id, published.report_id);

    let card = service
        .task_get(Some(String::from("PithosPandoras")), Some(task.clone()))
        .await
        .unwrap();
    assert_eq!(card.reports.len(), 1);
    assert!(card.drafts.is_empty());
}

#[tokio::test]
async fn acceptance_requires_summary_and_checks() {
    let (service, _dir) = setup().await;
    let task = start_task(&service).await;
    let draft = service
        .report_open(open_req(&task, "acceptance"))
        .await
        .unwrap();

    service
        .report_section(ReportSectionRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            key: Some(String::from("changes")),
            title: Some(String::from("Что изменено")),
            body: Some(String::from("Подробное описание изменений в модуле.")),
            order: None,
            expected_revision: None,
        })
        .await
        .unwrap();

    let error = service
        .report_publish(ReportPublishRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            expected_revision: None,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::FieldInvalid);

    service
        .report_section(ReportSectionRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            key: Some(String::from("summary")),
            title: None,
            body: Some(String::from("Задача принята, замечаний нет.")),
            order: None,
            expected_revision: None,
        })
        .await
        .unwrap();
    service
        .report_section(ReportSectionRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            key: Some(String::from("checks")),
            title: None,
            body: Some(String::from("Проверены списки, карточка и сохранение.")),
            order: None,
            expected_revision: None,
        })
        .await
        .unwrap();

    let published = service
        .report_publish(ReportPublishRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            expected_revision: None,
        })
        .await
        .unwrap();
    assert!(published.report.content.contains("## Итог"));
    assert!(published.report.content.contains("## Что проверено"));
}

#[tokio::test]
async fn empty_stage_finish_writes_nothing() {
    let (service, _dir) = setup().await;
    let task = start_task(&service).await;

    let error = service
        .stage_finish(StageFinishRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            role: Some(String::from("Исполнитель")),
            verdict: Some(String::from("готово")),
            report_id: None,
            kind: None,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::FieldInvalid);

    let card = service
        .task_get(Some(String::from("PithosPandoras")), Some(task))
        .await
        .unwrap();
    assert!(card.reports.is_empty());
    assert!(card.current_stage.is_some());
}

#[tokio::test]
async fn draft_owner_only_writes() {
    let (service, _dir) = setup().await;
    let task = start_task(&service).await;
    let draft = service
        .report_open(open_req(&task, "report"))
        .await
        .unwrap();

    let error = service
        .report_section(ReportSectionRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Коммандер")),
            key: Some(String::from("summary")),
            body: Some(String::from(
                "чужой текст, который не должен попасть в отчёт",
            )),
            title: None,
            order: None,
            expected_revision: None,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);

    let document = service
        .report_draft(Some(draft.draft_id.clone()))
        .await
        .unwrap();
    assert!(document.rendered.is_empty());

    service
        .report_discard(ReportDiscardRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
        })
        .await
        .unwrap();
    let missing = service
        .report_draft(Some(draft.draft_id))
        .await
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::NotFound);
}

#[tokio::test]
async fn draft_is_visible_in_card_but_not_in_reports() {
    let (service, _dir) = setup().await;
    let task = start_task(&service).await;
    let draft = service
        .report_open(open_req(&task, "report"))
        .await
        .unwrap();
    service
        .report_section(ReportSectionRequest {
            draft_id: Some(draft.draft_id.clone()),
            role: Some(String::from("Исполнитель")),
            key: Some(String::from("summary")),
            title: None,
            body: Some(String::from("Черновик виден в карточке, но ещё не отчёт.")),
            order: None,
            expected_revision: None,
        })
        .await
        .unwrap();

    let card = service
        .task_get(Some(String::from("PithosPandoras")), Some(task))
        .await
        .unwrap();
    assert!(card.reports.is_empty());
    assert_eq!(card.drafts.len(), 1);
    assert_eq!(card.drafts[0].draft_id, draft.draft_id);
    assert!(card.drafts[0].preview.contains("Черновик виден"));
}
