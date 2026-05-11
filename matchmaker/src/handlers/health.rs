use axum::{Json, http::StatusCode};

#[derive(serde::Serialize)]
pub struct HealthResponseBody {
    status: &'static str,
}

pub async fn check_health() -> (StatusCode, Json<HealthResponseBody>) {
    (StatusCode::OK, Json(HealthResponseBody { status: "ok" }))
}
