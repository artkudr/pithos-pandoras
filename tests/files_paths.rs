#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::sync::Arc;

use pithos_pandoras::domain::ErrorCode;
use pithos_pandoras::service::{
    FileAddRequest, FileGetRequest, Service, StageStartRequest, TaskCreateRequest,
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

async fn create_task(service: &Service) -> (String, String) {
    let created = service
        .task_create(TaskCreateRequest {
            project: Some(String::from("PithosPandoras")),
            role: Some(String::from("Коммандер")),
            title: Some(String::from("Файловая задача")),
            statement: Some(String::from("постановка")),
            route: None,
            resource: None,
        })
        .await
        .unwrap();
    (created.task.number.to_string(), created.folder_path)
}

fn add_req(task: &str, path: Option<String>, file_name: Option<String>) -> FileAddRequest {
    FileAddRequest {
        project: Some(String::from("PithosPandoras")),
        task: Some(task.to_string()),
        role: Some(String::from("Исполнитель")),
        path,
        file_name,
        title: Some(String::from("Файл проверки")),
        comment: None,
        kind: None,
    }
}

#[tokio::test]
async fn relative_path_is_resolved_from_project_root() {
    let (service, _dir) = setup().await;
    let (task, folder_path) = create_task(&service).await;
    let file_path = Path::new(&folder_path).join("note.txt");
    std::fs::write(&file_path, b"hello").unwrap();

    let relative = format!(
        "Задачи/{}/note.txt",
        Path::new(&folder_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
    );
    let added = service
        .file_add(add_req(&task, Some(relative), None))
        .await
        .unwrap();

    let file = service
        .file_get(FileGetRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            file: Some(added.file_id),
        })
        .await
        .unwrap();
    assert_eq!(Path::new(&file.path), file_path.canonicalize().unwrap());

    let card = service
        .task_get(Some(String::from("PithosPandoras")), Some(task))
        .await
        .unwrap();
    assert_eq!(Path::new(&card.folder_path), Path::new(&folder_path));
}

#[tokio::test]
async fn file_name_is_looked_up_inside_task_folder() {
    let (service, _dir) = setup().await;
    let (task, folder_path) = create_task(&service).await;
    std::fs::write(Path::new(&folder_path).join("shot.png"), b"png").unwrap();

    let added = service
        .file_add(add_req(&task, None, Some(String::from("shot.png"))))
        .await
        .unwrap();

    let file = service
        .file_get(FileGetRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task),
            file: Some(added.file_id),
        })
        .await
        .unwrap();
    assert!(file.path.ends_with("shot.png"));
}

#[tokio::test]
async fn missing_file_reports_absolute_path_it_tried() {
    let (service, _dir) = setup().await;
    let (task, folder_path) = create_task(&service).await;
    let relative = format!(
        "Задачи/{}/absent.txt",
        Path::new(&folder_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
    );

    let error = service
        .file_add(add_req(&task, Some(relative.clone()), None))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::FileNotFound);
    assert_eq!(error.path.as_deref(), Some(relative.as_str()));
    let tried = error.tried.unwrap();
    assert_eq!(tried.len(), 1);
    assert!(tried[0].ends_with("absent.txt"));
    assert!(Path::new(&tried[0]).is_absolute());
}

#[tokio::test]
async fn file_outside_root_is_rejected() {
    let (service, dir) = setup().await;
    let (task, _folder_path) = create_task(&service).await;
    let outside = dir.path().parent().unwrap().join("outside-pipa-test.txt");
    std::fs::write(&outside, b"x").unwrap();

    let error = service
        .file_add(add_req(
            &task,
            Some(String::from("../outside-pipa-test.txt")),
            None,
        ))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    let _ = std::fs::remove_file(outside);
}

#[tokio::test]
async fn file_name_with_separator_is_rejected() {
    let (service, _dir) = setup().await;
    let (task, _folder_path) = create_task(&service).await;

    let error = service
        .file_add(add_req(
            &task,
            None,
            Some(String::from("Проверки/shot.png")),
        ))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::FieldInvalid);
}

#[tokio::test]
async fn role_is_inferred_from_open_stage() {
    let (service, _dir) = setup().await;
    let (task, folder_path) = create_task(&service).await;
    service
        .stage_start(StageStartRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            role: Some(String::from("Исполнитель")),
        })
        .await
        .unwrap();
    std::fs::write(Path::new(&folder_path).join("log.txt"), b"log").unwrap();

    let added = service
        .file_add(FileAddRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task.clone()),
            role: None,
            path: None,
            file_name: Some(String::from("log.txt")),
            title: Some(String::from("Лог")),
            comment: None,
            kind: None,
        })
        .await
        .unwrap();

    let file = service
        .file_get(FileGetRequest {
            project: Some(String::from("PithosPandoras")),
            task: Some(task),
            file: Some(added.file_id),
        })
        .await
        .unwrap();
    assert_eq!(file.actor, "Исполнитель");
}
