use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::db::Database;
use crate::models::MediaFile;
use crate::scanner::{MediaScanner, ScanStats};

pub struct AppState {
    pub db: Arc<Database>,
    pub scanner: Arc<MediaScanner>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub media_type: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub path: String,
}

pub async fn get_media_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<MediaFile>>, StatusCode> {
    match state.db.get_media_by_id(&id).await {
        Ok(Some(media)) => Ok(Json(ApiResponse::success(media))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn list_media(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<MediaFile>>>, StatusCode> {
    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);

    match state.db.list_media(params.media_type, limit, offset).await {
        Ok(media) => Ok(Json(ApiResponse::success(media))),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn search_media(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<MediaFile>>>, StatusCode> {
    match state.db.search_media(&params.q).await {
        Ok(media) => Ok(Json(ApiResponse::success(media))),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    let media = match state.db.get_media_by_id(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if !media.media_type.eq("image") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let path = std::path::Path::new(&media.file_path);
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    match ServeFile::new(path).oneshot(axum::http::Request::new(())).await {
        Ok(response) => Ok(response.into_response()),
        Err(e) => {
            tracing::error!("Failed to serve file: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn start_scan(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ScanRequest>,
) -> Result<Json<ApiResponse<ScanStats>>, StatusCode> {
    let path = std::path::Path::new(&req.path);
    
    if !path.exists() || !path.is_dir() {
        return Ok(Json(ApiResponse::error(
            "Invalid path: directory does not exist".to_string(),
        )));
    }

    let scanner = state.scanner.clone();
    let scan_result = tokio::spawn(async move {
        scanner.scan_directory(path).await
    });

    match scan_result.await {
        Ok(Ok(stats)) => Ok(Json(ApiResponse::success(stats))),
        Ok(Err(e)) => {
            tracing::error!("Scan error: {}", e);
            Ok(Json(ApiResponse::error(format!("Scan failed: {}", e))))
        }
        Err(e) => {
            tracing::error!("Task join error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match state.db.get_stats().await {
        Ok(stats) => Ok(Json(ApiResponse::success(stats))),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_scan_history(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<crate::models::ScanHistory>>>, StatusCode> {
    match state.db.get_scan_history(10).await {
        Ok(history) => Ok(Json(ApiResponse::success(history))),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "medianator",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}