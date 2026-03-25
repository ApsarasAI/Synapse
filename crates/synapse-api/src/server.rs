use std::net::SocketAddr;

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use synapse_core::{ExecuteRequest, ExecuteResponse, SandboxPool, SynapseError};

use crate::app::default_state;
pub use crate::app::AppState;

pub fn router() -> Router {
    router_with_state(default_state())
}

pub fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/execute", post(execute_request))
        .with_state(state)
}

pub async fn serve(listen: SocketAddr) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, router()).await
}

async fn health() -> &'static str {
    "ok"
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    render_metrics(state.pool())
}

async fn execute_request(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> (StatusCode, Json<ExecuteResponse>) {
    match state.pool().execute(req).await {
        Ok(response) => (StatusCode::OK, Json(response)),
        Err(error) => map_error(error),
    }
}

fn map_error(error: SynapseError) -> (StatusCode, Json<ExecuteResponse>) {
    match error {
        SynapseError::InvalidInput(message) | SynapseError::UnsupportedLanguage(message) => (
            StatusCode::BAD_REQUEST,
            Json(ExecuteResponse::error(message, 0)),
        ),
        SynapseError::Execution(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ExecuteResponse::error(message, 0)),
        ),
        SynapseError::Io(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ExecuteResponse::error(error.to_string(), 0)),
        ),
    }
}

fn render_metrics(pool: &SandboxPool) -> String {
    let metrics = pool.metrics();
    format!(
        concat!(
            "synapse_pool_configured_size {}\n",
            "synapse_pool_available {}\n",
            "synapse_pool_pooled_total {}\n",
            "synapse_pool_active {}\n",
            "synapse_pool_overflow_active {}\n",
            "synapse_pool_overflow_total {}\n",
            "synapse_pool_poisoned_total {}\n",
            "synapse_execute_requests_total {}\n",
            "synapse_execute_completed_total {}\n",
            "synapse_execute_failed_total {}\n",
            "synapse_execute_timeouts_total {}\n",
        ),
        metrics.configured_size,
        metrics.available,
        metrics.pooled_total,
        metrics.active,
        metrics.overflow_active,
        metrics.overflow_total,
        metrics.poisoned_total,
        metrics.requests_total,
        metrics.completed_total,
        metrics.failed_total,
        metrics.timeouts_total,
    )
}
