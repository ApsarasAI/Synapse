use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use synapse_api::server::{router, router_with_state, AppState};
use synapse_core::SandboxPool;
use tower::util::ServiceExt;

#[tokio::test]
async fn health_returns_ok() {
    let response = router()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn execute_returns_python_output() {
    if !python3_available().await {
        return;
    }

    let response = router()
        .oneshot(json_request(
            "/execute",
            json!({
                "language": "python",
                "code": "print('hello from api')\n",
                "timeout_ms": 5_000,
                "memory_limit_mb": 128
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["stdout"], "hello from api\n");
    assert_eq!(body["stderr"], "");
    assert_eq!(body["exit_code"], 0);
}

#[tokio::test]
async fn execute_rejects_invalid_input() {
    let response = router()
        .oneshot(json_request(
            "/execute",
            json!({
                "language": "python",
                "code": "   ",
                "timeout_ms": 5_000,
                "memory_limit_mb": 128
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(response).await;
    assert_eq!(body["exit_code"], -1);
    assert_eq!(body["stderr"], "code cannot be empty");
}

#[tokio::test]
async fn execute_times_out_through_http() {
    if !python3_available().await {
        return;
    }

    let response = router()
        .oneshot(json_request(
            "/execute",
            json!({
                "language": "python",
                "code": "while True:\n    pass\n",
                "timeout_ms": 50,
                "memory_limit_mb": 128
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["exit_code"], -1);
    assert!(body["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("execution timed out"));
}

#[tokio::test]
async fn metrics_reflect_execute_requests() {
    if !python3_available().await {
        return;
    }

    let state = AppState::new(SandboxPool::new(2));
    let app = router_with_state(state);

    let response = app
        .clone()
        .oneshot(json_request(
            "/execute",
            json!({
                "language": "python",
                "code": "print('metrics')\n",
                "timeout_ms": 5_000,
                "memory_limit_mb": 128
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("synapse_pool_configured_size 2"));
    assert!(text.contains("synapse_execute_requests_total 1"));
    assert!(text.contains("synapse_execute_completed_total 1"));
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
    tokio::process::Command::new("python3")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok()
}
