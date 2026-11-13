#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use pithos_pandoras::service::Service;
use pithos_pandoras::store::{self, Store, StoreError};
use tempfile::TempDir;

async fn init(
    store: &Store,
    name: &str,
    root: &str,
    tasks_dir: &str,
    folder_pattern: &str,
) -> Result<(), StoreError> {
    let name = name.to_string();
    let root = root.to_string();
    let tasks_dir = tasks_dir.to_string();
    let folder_pattern = folder_pattern.to_string();
    store
        .write(move |conn| {
            store::projects::init_project(
                conn,
                &name,
                &root,
                &tasks_dir,
                &folder_pattern,
                &[(String::from("Коммандер"), None)],
                "{}",
            )
        })
        .await
        .map(|_| ())
}

fn service(store: &Store) -> Service {
    Service::new(Arc::new(store.clone()))
}

#[tokio::test]
async fn projects_number_tasks_independently() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("test.db")).unwrap();
    let root = dir.path().to_string_lossy().to_string();
    init(&store, "A", &root, "Задачи-A", "{number} - {date}")
        .await
        .unwrap();
    init(&store, "B", &root, "Задачи-B", "{number} - {date}")
        .await
        .unwrap();
    let service = service(&store);

    let a1 = service
        .human_task_create("A", "A1", "s", None, None)
        .await
        .unwrap();
    let b1 = service
        .human_task_create("B", "B1", "s", None, None)
        .await
        .unwrap();
    let a2 = service
        .human_task_create("A", "A2", "s", None, None)
        .await
        .unwrap();
    let b2 = service
        .human_task_create("B", "B2", "s", None, None)
        .await
        .unwrap();

    assert_eq!((a1.task.number, a2.task.number), (1, 2));
    assert_eq!((b1.task.number, b2.task.number), (1, 2));
    assert_ne!(a1.folder_path, b1.folder_path);
}

#[tokio::test]
async fn same_number_resolves_inside_project() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("test.db")).unwrap();
    let root = dir.path().to_string_lossy().to_string();
    init(&store, "A", &root, "Задачи-A", "{number} - {date}")
        .await
        .unwrap();
    init(&store, "B", &root, "Задачи-B", "{number} - {date}")
        .await
        .unwrap();
    let service = service(&store);

    let a = service
        .human_task_create("A", "Задача A", "s", None, None)
        .await
        .unwrap();
    let b = service
        .human_task_create("B", "Задача B", "s", None, None)
        .await
        .unwrap();

    let from_a = service
        .task_get(Some(String::from("A")), Some(a.task.number.to_string()))
        .await
        .unwrap();
    let from_b = service
        .task_get(Some(String::from("B")), Some(b.task.number.to_string()))
        .await
        .unwrap();

    assert_eq!(from_a.task.title, "Задача A");
    assert_eq!(from_b.task.title, "Задача B");
}

#[tokio::test]
async fn task_error_names_project() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("test.db")).unwrap();
    let root = dir.path().to_string_lossy().to_string();
    init(&store, "devtools", &root, "Задачи", "{number} - {date}")
        .await
        .unwrap();
    let service = service(&store);

    let err = service
        .task_get(Some(String::from("devtools")), Some(String::from("99")))
        .await
        .unwrap_err();
    assert!(err.message.contains("devtools"), "message: {}", err.message);
    assert!(err.message.contains("99"), "message: {}", err.message);
}

#[tokio::test]
async fn duplicate_task_folder_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("test.db")).unwrap();
    let root = dir.path().to_string_lossy().to_string();
    init(&store, "A", &root, "Задачи", "{number} - {date}")
        .await
        .unwrap();

    let err = init(&store, "B", &root, "Задачи", "{number} - {date}")
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)), "err: {err:?}");
}

#[tokio::test]
async fn same_tasks_folder_through_different_paths_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("test.db")).unwrap();
    let root = dir.path().to_string_lossy().to_string();
    init(&store, "A", &root, "A/B", "{number} - {date}")
        .await
        .unwrap();

    let nested = std::path::Path::new(&root).join("A");
    let nested = nested.to_string_lossy().to_string();
    let err = init(&store, "B", &nested, "B", "{number} - {date}")
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)), "err: {err:?}");
}

#[tokio::test]
async fn folder_pattern_without_number_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("test.db")).unwrap();
    let root = dir.path().to_string_lossy().to_string();

    let err = init(&store, "A", &root, "Задачи", "{date}")
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)), "err: {err:?}");
}

#[tokio::test]
async fn folder_pattern_cannot_escape_tasks_dir() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("test.db")).unwrap();
    let root = dir.path().to_string_lossy().to_string();

    let err = init(&store, "A", &root, "Задачи", "../{number}")
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)), "err: {err:?}");
}

#[tokio::test]
async fn project_name_with_path_separator_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("test.db")).unwrap();
    let root = dir.path().to_string_lossy().to_string();

    let err = init(&store, "A/B", &root, "Задачи", "{number} - {date}")
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Conflict(_)), "err: {err:?}");
}
