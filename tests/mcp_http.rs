#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pithos_pandoras::mcp::server::router;
use pithos_pandoras::service::Service;
use pithos_pandoras::store::{self, Store};
use tempfile::TempDir;
use tower::ServiceExt;

const KEY: &str = "test-key";

async fn setup() -> (axum::Router, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(Store::open(dir.path().join("test.db")).unwrap());
    let root = dir.path().to_string_lossy().to_string();
    let roles = vec![(String::from("Коммандер"), None)];
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
    let service = Arc::new(Service::new(store));
    (router(service, KEY.to_string()), dir)
}

fn request(body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", "localhost")
        .header("authorization", format!("Bearer {KEY}"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn call(router: axum::Router, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let response = router.oneshot(request(body)).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or_else(
        |_| serde_json::json!({ "raw": String::from_utf8_lossy(&bytes).to_string() }),
    );
    (status, value)
}

fn initialize() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "pipatest", "version": "1.0"}
        }
    })
}

fn tool_call(id: i64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "task_list",
            "arguments": {"project": "PithosPandoras"}
        }
    })
}

#[tokio::test]
async fn initialize_works_without_session_header() {
    let (router, _dir) = setup().await;
    let (status, value) = call(router, initialize()).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert!(value.get("result").is_some(), "{value}");
    assert!(value.get("error").is_none(), "{value}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn five_parallel_tool_calls_all_succeed() {
    let (router, _dir) = setup().await;
    let (status, value) = call(router.clone(), initialize()).await;
    assert_eq!(status, StatusCode::OK, "{value}");

    let mut tasks = Vec::new();
    for id in 2..7 {
        let router = router.clone();
        tasks.push(tokio::spawn(
            async move { call(router, tool_call(id)).await },
        ));
    }

    for task in tasks {
        let (status, value) = task.await.unwrap();
        assert_eq!(status, StatusCode::OK, "{value}");
        assert!(value.get("result").is_some(), "{value}");
        assert!(value.get("error").is_none(), "{value}");
    }
}
