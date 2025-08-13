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
use crate::models::{MediaFile, Face, FaceGroup, DuplicateGroup};
use crate::scanner::{MediaScanner, ScanStats, duplicate::DuplicateDetector};

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

pub async fn get_thumbnail(
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

    if let Some(thumbnail_path) = media.thumbnail_path {
        let path = std::path::Path::new(&thumbnail_path);
        if path.exists() {
            match ServeFile::new(path).oneshot(axum::http::Request::new(())).await {
                Ok(response) => return Ok(response.into_response()),
                Err(e) => {
                    tracing::error!("Failed to serve thumbnail: {}", e);
                }
            }
        }
    }

    Err(StatusCode::NOT_FOUND)
}

pub async fn get_duplicates(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<DuplicateGroup>>>, StatusCode> {
    let detector = DuplicateDetector::new(state.db.get_pool());
    
    match detector.find_all_duplicates().await {
        Ok(duplicates) => Ok(Json(ApiResponse::success(duplicates))),
        Err(e) => {
            tracing::error!("Failed to find duplicates: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DuplicateCleanupQuery {
    pub keep_newest: Option<bool>,
}

pub async fn suggest_duplicate_cleanup(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DuplicateCleanupQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    let detector = DuplicateDetector::new(state.db.get_pool());
    let keep_newest = params.keep_newest.unwrap_or(true);
    
    match detector.suggest_files_to_remove(keep_newest).await {
        Ok(files) => Ok(Json(ApiResponse::success(files))),
        Err(e) => {
            tracing::error!("Failed to suggest duplicate cleanup: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_duplicate_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let detector = DuplicateDetector::new(state.db.get_pool());
    
    match detector.get_duplicate_stats().await {
        Ok(stats) => {
            Ok(Json(ApiResponse::success(serde_json::json!({
                "duplicate_groups": stats.duplicate_groups,
                "redundant_files": stats.redundant_files,
                "wasted_space_bytes": stats.wasted_space,
                "wasted_space_human": stats.wasted_space_human_readable(),
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to get duplicate stats: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_faces_for_media(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Face>>>, StatusCode> {
    match state.db.get_faces_for_media(&id).await {
        Ok(faces) => Ok(Json(ApiResponse::success(faces))),
        Err(e) => {
            tracing::error!("Failed to get faces: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_face_groups(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<FaceGroup>>>, StatusCode> {
    match state.db.get_face_groups().await {
        Ok(groups) => Ok(Json(ApiResponse::success(groups))),
        Err(e) => {
            tracing::error!("Failed to get face groups: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateFaceGroupRequest {
    pub name: Option<String>,
}

pub async fn create_face_group(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateFaceGroupRequest>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    match state.db.create_face_group(req.name).await {
        Ok(group_id) => Ok(Json(ApiResponse::success(group_id))),
        Err(e) => {
            tracing::error!("Failed to create face group: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GroupFacesRequest {
    pub face_id: String,
    pub group_id: String,
    pub similarity_score: f32,
}

pub async fn add_face_to_group(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GroupFacesRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.db.add_face_to_group(&req.face_id, &req.group_id, req.similarity_score).await {
        Ok(()) => Ok(Json(ApiResponse::success(()))),
        Err(e) => {
            tracing::error!("Failed to add face to group: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}