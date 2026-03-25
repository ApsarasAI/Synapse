use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::OnceLock;
use synapse_api::server::router;
use tokio::sync::Mutex;
use tower::util::ServiceExt;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[tokio::test]
async fn execute_does_not_expose_host_etc_passwd() {
    if !python3_available().await {
        return;
    }

    let response = router()
        .oneshot(json_request(
            "/execute",
            json!({
                "language": "python",
                "code": "from pathlib import Path\ntry:\n    Path('/etc/passwd').read_text()\n    print('visible')\nexcept Exception as exc:\n    print(type(exc).__name__)\n",
                "timeout_ms": 5_000,
                "memory_limit_mb": 128
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["stdout"], "FileNotFoundError\n");
    assert_eq!(body["exit_code"], 0);
}

#[tokio::test]
async fn execute_blocks_process_spawning_syscalls() {
    if !python3_available().await || !command_available("bwrap").await {
        return;
    }

    let response = router()
        .oneshot(json_request(
            "/execute",
            json!({
                "language": "python",
                "code": "import os\ntry:\n    os.fork()\n    print('forked')\nexcept Exception as exc:\n    print(type(exc).__name__)\n    print(getattr(exc, 'errno', None))\n",
                "timeout_ms": 5_000,
                "memory_limit_mb": 128
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let stdout = body["stdout"].as_str().unwrap();
    assert!(!stdout.contains("forked"));
    assert!(stdout.contains("PermissionError") || stdout.contains("OSError"));
}

#[tokio::test]
async fn execute_does_not_leak_server_environment() {
    if !python3_available().await {
        return;
    }

    let _guard = env_lock().lock().await;
    unsafe {
        std::env::set_var("SYNAPSE_HTTP_SECRET", "top-secret");
    }
    let response = router()
        .oneshot(json_request(
            "/execute",
            json!({
                "language": "python",
                "code": "import os\nprint(os.getenv('SYNAPSE_HTTP_SECRET', 'missing'))\n",
                "timeout_ms": 5_000,
                "memory_limit_mb": 128
            }),
        ))
        .await
        .unwrap();
    unsafe {
        std::env::remove_var("SYNAPSE_HTTP_SECRET");
    }

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["stdout"], "missing\n");
    assert_eq!(body["exit_code"], 0);
}

fn json_request(uri: &str, payload: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn python3_available() -> bool {
    command_available("python3").await
}

async fn command_available(command: &str) -> bool {
    tokio::process::Command::new(command)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok()
}
