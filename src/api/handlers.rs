use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::db::Database;
use crate::models::{MediaFile, Face, FaceGroup, DuplicateGroup, MediaGroup, MediaGroupWithItems, SmartAlbum, SmartAlbumFilter};
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

pub async fn get_sub_images(
    State(state): State<Arc<AppState>>,
    Path(parent_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<MediaFile>>>, StatusCode> {
    match state.db.get_sub_images(&parent_id).await {
        Ok(sub_images) => Ok(Json(ApiResponse::success(sub_images))),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_parent_image(
    State(state): State<Arc<AppState>>,
    Path(sub_image_id): Path<String>,
) -> Result<Json<ApiResponse<MediaFile>>, StatusCode> {
    match state.db.get_parent_image(&sub_image_id).await {
        Ok(Some(parent)) => Ok(Json(ApiResponse::success(parent))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
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

pub async fn get_video(
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

    if !media.media_type.eq("video") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let path = std::path::Path::new(&media.file_path);
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Serve video with proper content type
    let mime_type = mime_guess::from_path(path).first_or_octet_stream();
    
    match ServeFile::new(path).oneshot(axum::http::Request::new(())).await {
        Ok(mut response) => {
            // Ensure proper content type for video
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_str(mime_type.as_ref()).unwrap_or_else(|_| {
                    axum::http::HeaderValue::from_static("video/mp4")
                }),
            );
            Ok(response.into_response())
        }
        Err(e) => {
            tracing::error!("Failed to serve video file: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_audio(
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

    if !media.media_type.eq("audio") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let path = std::path::Path::new(&media.file_path);
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Serve audio with proper content type
    let mime_type = mime_guess::from_path(path).first_or_octet_stream();
    
    match ServeFile::new(path).oneshot(axum::http::Request::new(())).await {
        Ok(mut response) => {
            // Ensure proper content type for audio
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_str(mime_type.as_ref()).unwrap_or_else(|_| {
                    axum::http::HeaderValue::from_static("audio/mpeg")
                }),
            );
            Ok(response.into_response())
        }
        Err(e) => {
            tracing::error!("Failed to serve audio file: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn start_scan(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ScanRequest>,
) -> Result<Json<ApiResponse<ScanStats>>, StatusCode> {
    let path_str = req.path.clone();
    let path = std::path::Path::new(&path_str);
    
    if !path.exists() || !path.is_dir() {
        return Ok(Json(ApiResponse::error(
            "Invalid path: directory does not exist".to_string(),
        )));
    }

    let scanner = state.scanner.clone();
    let path_buf = path.to_path_buf();
    let scan_result = tokio::spawn(async move {
        scanner.scan_directory(&path_buf).await
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

pub async fn get_face_thumbnail(
    State(state): State<Arc<AppState>>,
    Path(face_id): Path<String>,
) -> Result<Response, StatusCode> {
    // Get the face from database
    let face = match state.db.get_face_by_id(&face_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Get the media file
    let media = match state.db.get_media_by_id(&face.media_file_id).await {
        Ok(Some(m)) => m,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Parse face bounding box
    let bbox_parts: Vec<i32> = face.face_bbox
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    if bbox_parts.len() != 4 {
        tracing::error!("Invalid face bbox: {}", face.face_bbox);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    let (x, y, width, height) = (bbox_parts[0], bbox_parts[1], bbox_parts[2], bbox_parts[3]);
    
    // Load the image and extract face region
    let image_path = std::path::Path::new(&media.file_path);
    if !image_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    match extract_face_thumbnail(image_path, x, y, width, height).await {
        Ok(thumbnail_bytes) => {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "image/jpeg")
                .header("Cache-Control", "public, max-age=3600")
                .body(Body::from(thumbnail_bytes))
                .unwrap())
        }
        Err(e) => {
            tracing::error!("Failed to extract face thumbnail: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn extract_face_thumbnail(
    image_path: &std::path::Path,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::imageops;
    
    let img = image::open(image_path)?;
    
    // Ensure coordinates are within bounds
    let x = x.max(0) as u32;
    let y = y.max(0) as u32;
    let width = width.max(1) as u32;
    let height = height.max(1) as u32;
    
    let img_width = img.width();
    let img_height = img.height();
    
    let x = x.min(img_width.saturating_sub(1));
    let y = y.min(img_height.saturating_sub(1));
    let width = width.min(img_width - x);
    let height = height.min(img_height - y);
    
    // Crop the face region
    let face_img = img.crop_imm(x, y, width, height);
    
    // Resize to thumbnail (150x150)
    let thumbnail = face_img.resize(150, 150, imageops::FilterType::Lanczos3);
    
    // Convert to JPEG bytes
    let mut bytes = Vec::new();
    thumbnail.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg)?;
    
    Ok(bytes)
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

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().unwrap_or("").to_string();
        
        if name == "file" && !file_name.is_empty() {
            // Create upload directory if it doesn't exist
            let upload_dir = std::path::Path::new("uploads");
            if !upload_dir.exists() {
                if let Err(e) = fs::create_dir_all(upload_dir).await {
                    tracing::error!("Failed to create upload directory: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
            
            // Generate unique filename
            let unique_name = format!("{}_{}", uuid::Uuid::new_v4(), file_name);
            let file_path = upload_dir.join(&unique_name);
            
            // Get file data
            let data = match field.bytes().await {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::error!("Failed to read file data: {}", e);
                    return Err(StatusCode::BAD_REQUEST);
                }
            };
            
            // Save file
            if let Err(e) = fs::write(&file_path, &data).await {
                tracing::error!("Failed to save file: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            
            // Scan the uploaded file
            let scanner = state.scanner.clone();
            let path = file_path.clone();
            tokio::spawn(async move {
                if let Err(e) = scanner.scan_directory(&path.parent().unwrap()).await {
                    tracing::error!("Failed to scan uploaded file: {}", e);
                }
            });
            
            return Ok(Json(ApiResponse::success(format!("File uploaded: {}", unique_name))));
        }
    }
    
    Ok(Json(ApiResponse::error("No file provided".to_string())))
}

pub async fn reprocess_media(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    // Get the media file from database
    let media = match state.db.get_media_by_id(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let scanner = state.scanner.clone();
    let db = state.db.clone();
    let file_path = media.file_path.clone();
    let media_id = media.id.clone();
    
    // Spawn reprocessing task
    tokio::spawn(async move {
        tracing::info!("Reprocessing media file: {}", file_path);
        
        // Re-extract metadata
        if let Ok(metadata) = crate::scanner::metadata::MetadataExtractor::extract(
            std::path::Path::new(&file_path)
        ).await {
            if let Err(e) = db.insert_media_file(&metadata).await {
                tracing::error!("Failed to update metadata: {}", e);
            }
        }
        
        // Extract sub-images if configured and it's an image
        if media.media_type == "image" {
            if let (Some(ref extractor), Some(ref output_dir)) = (&scanner.sub_image_extractor, &scanner.sub_image_output_dir) {
                tracing::info!("Checking for sub-images in {}", file_path);
                
                // Delete existing sub-images for this parent
                if let Err(e) = db.delete_sub_images(&media_id).await {
                    tracing::warn!("Failed to delete old sub-images: {}", e);
                }
                
                match extractor.extract_sub_images(std::path::Path::new(&file_path), output_dir).await {
                    Ok(sub_images) => {
                        tracing::info!("Extracted {} sub-images from {}", sub_images.len(), file_path);
                        for (sub_image_path, extraction_metadata) in sub_images {
                            // Process each sub-image as a new media file
                            if let Ok(mut sub_metadata) = crate::scanner::metadata::MetadataExtractor::extract(&sub_image_path).await {
                                // Copy parent metadata
                                if let Ok(parent_metadata) = crate::scanner::metadata::MetadataExtractor::extract(
                                    std::path::Path::new(&file_path)
                                ).await {
                                    sub_metadata.camera_info = parent_metadata.camera_info.clone();
                                    sub_metadata.timestamps.created = parent_metadata.timestamps.created;
                                }
                                
                                // Set parent relationship
                                let extraction_json = serde_json::to_string(&extraction_metadata).ok();
                                
                                // Insert sub-image with parent reference
                                if let Err(e) = db.insert_sub_image(&sub_metadata, &media_id, extraction_json).await {
                                    tracing::warn!("Failed to insert sub-image: {}", e);
                                } else {
                                    // Run face detection on sub-image
                                    if let Some(ref detector) = scanner.face_detector {
                                        match detector.detect_faces(&sub_image_path, &sub_metadata.id).await {
                                            Ok(faces) => {
                                                for face in faces {
                                                    if let Err(e) = db.insert_face(&face).await {
                                                        tracing::warn!("Failed to insert face from sub-image: {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!("Failed to detect faces in sub-image: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("No sub-images extracted from {}: {}", file_path, e);
                    }
                }
            }
            
            // Re-run face detection if enabled
            if let Some(ref detector) = scanner.face_detector {
                // First, delete existing faces for this media
                if let Err(e) = db.delete_faces_for_media(&media_id).await {
                    tracing::warn!("Failed to delete old faces: {}", e);
                }
                
                // Detect new faces
                match detector.detect_faces(std::path::Path::new(&file_path), &media_id).await {
                    Ok(faces) => {
                        tracing::info!("Detected {} faces in reprocessed image", faces.len());
                        let face_count = faces.len();
                        for face in faces {
                            if let Err(e) = db.insert_face(&face).await {
                                tracing::error!("Failed to insert face: {}", e);
                            }
                        }
                        
                        // Auto-group faces after insertion
                        if face_count > 0 {
                            if let Err(e) = db.auto_group_faces().await {
                                tracing::warn!("Failed to auto-group faces: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to detect faces: {}", e);
                    }
                }
            }
        }
        
        // Re-generate thumbnail if configured
        if let Some(ref gen) = scanner.thumbnail_generator {
            if media.media_type == "image" {
                match gen.generate_thumbnail(
                    std::path::Path::new(&file_path), 
                    &media_id
                ).await {
                    Ok(thumb_path) => {
                        // Update database with thumbnail path
                        if let Err(e) = db.update_thumbnail_path(&media_id, &thumb_path.to_string_lossy()).await {
                            tracing::warn!("Failed to update thumbnail path in database: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to regenerate thumbnail: {}", e);
                    }
                }
            } else if media.media_type == "video" {
                match gen.generate_video_thumbnail(
                    std::path::Path::new(&file_path),
                    &media_id
                ).await {
                    Ok(thumb_path) => {
                        // Update database with thumbnail path
                        if let Err(e) = db.update_thumbnail_path(&media_id, &thumb_path.to_string_lossy()).await {
                            tracing::warn!("Failed to update thumbnail path in database: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to regenerate video thumbnail: {}", e);
                    }
                }
            }
        }
    });

    Ok(Json(ApiResponse::success(
        format!("Reprocessing started for media {}", id)
    )))
}

#[derive(Debug, Deserialize)]
pub struct BatchReprocessRequest {
    pub media_ids: Vec<String>,
    pub reprocess_faces: bool,
    pub reprocess_thumbnails: bool,
    pub reprocess_metadata: bool,
}

pub async fn batch_reprocess(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchReprocessRequest>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let scanner = state.scanner.clone();
    let db = state.db.clone();
    let media_ids = req.media_ids.clone();
    
    tokio::spawn(async move {
        let mut processed = 0;
        let mut errors = 0;
        
        for media_id in media_ids {
            // Get media file
            let media = match db.get_media_by_id(&media_id).await {
                Ok(Some(m)) => m,
                Ok(None) => {
                    errors += 1;
                    continue;
                }
                Err(e) => {
                    tracing::error!("Failed to get media {}: {}", media_id, e);
                    errors += 1;
                    continue;
                }
            };
            
            let file_path = std::path::Path::new(&media.file_path);
            
            // Reprocess metadata if requested
            if req.reprocess_metadata {
                if let Ok(metadata) = crate::scanner::metadata::MetadataExtractor::extract(file_path).await {
                    if let Err(e) = db.insert_media_file(&metadata).await {
                        tracing::error!("Failed to update metadata for {}: {}", media_id, e);
                        errors += 1;
                    }
                }
            }
            
            // Reprocess faces if requested
            if req.reprocess_faces && media.media_type == "image" {
                if let Some(ref detector) = scanner.face_detector {
                    // Delete old faces
                    if let Err(e) = db.delete_faces_for_media(&media_id).await {
                        tracing::warn!("Failed to delete old faces for {}: {}", media_id, e);
                    }
                    
                    // Detect new faces
                    match detector.detect_faces(file_path, &media_id).await {
                        Ok(faces) => {
                            let face_count = faces.len();
                            for face in faces {
                                if let Err(e) = db.insert_face(&face).await {
                                    tracing::error!("Failed to insert face: {}", e);
                                }
                            }
                            
                            // Auto-group faces after insertion
                            if face_count > 0 {
                                if let Err(e) = db.auto_group_faces().await {
                                    tracing::warn!("Failed to auto-group faces: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to detect faces for {}: {}", media_id, e);
                            errors += 1;
                        }
                    }
                }
            }
            
            // Reprocess thumbnails if requested
            if req.reprocess_thumbnails {
                if let Some(ref gen) = scanner.thumbnail_generator {
                    if media.media_type == "image" {
                        if let Err(e) = gen.generate_thumbnail(file_path, &media_id).await {
                            tracing::error!("Failed to regenerate thumbnail for {}: {}", media_id, e);
                            errors += 1;
                        }
                    } else if media.media_type == "video" {
                        if let Err(e) = gen.generate_video_thumbnail(file_path, &media_id).await {
                            tracing::error!("Failed to regenerate video thumbnail for {}: {}", media_id, e);
                            errors += 1;
                        }
                    }
                }
            }
            
            processed += 1;
        }
        
        tracing::info!(
            "Batch reprocessing complete: {} processed, {} errors",
            processed, errors
        );
    });

    Ok(Json(ApiResponse::success(
        format!("Batch reprocessing started for {} media files", req.media_ids.len())
    )))
}

pub async fn get_faces_grouped(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, StatusCode> {
    // Get all face groups with their members
    match state.db.get_face_groups_with_members().await {
        Ok(groups) => Ok(Json(ApiResponse::success(groups))),
        Err(e) => {
            tracing::error!("Failed to get grouped faces: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Media Groups Endpoints

pub async fn get_media_groups(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<MediaGroup>>>, StatusCode> {
    match state.db.get_all_media_groups().await {
        Ok(groups) => Ok(Json(ApiResponse::success(groups))),
        Err(e) => {
            tracing::error!("Failed to get media groups: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_media_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<MediaGroupWithItems>>, StatusCode> {
    match state.db.get_media_group_with_items(&id).await {
        Ok(Some(group)) => Ok(Json(ApiResponse::success(group))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get media group: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AutoGroupRequest {
    pub group_type: String, // "date", "location", or "event"
}

pub async fn auto_group_media(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AutoGroupRequest>,
) -> Result<Json<ApiResponse<Vec<MediaGroup>>>, StatusCode> {
    use crate::scanner::grouping::MediaGrouper;
    
    let grouper = MediaGrouper::new(state.db.as_ref().clone());
    
    let groups = match req.group_type.as_str() {
        "date" => grouper.group_by_date().await,
        "location" => grouper.group_by_location().await,
        "event" => grouper.group_by_events().await,
        _ => {
            return Ok(Json(ApiResponse::error(
                "Invalid group type. Use 'date', 'location', or 'event'".to_string()
            )));
        }
    };
    
    match groups {
        Ok(groups) => Ok(Json(ApiResponse::success(groups))),
        Err(e) => {
            tracing::error!("Failed to auto-group media: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Smart Albums Endpoints

pub async fn get_smart_albums(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<SmartAlbum>>>, StatusCode> {
    match state.db.get_all_smart_albums().await {
        Ok(albums) => Ok(Json(ApiResponse::success(albums))),
        Err(e) => {
            tracing::error!("Failed to get smart albums: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_smart_album(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<SmartAlbum>>, StatusCode> {
    match state.db.get_smart_album(&id).await {
        Ok(Some(album)) => Ok(Json(ApiResponse::success(album))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get smart album: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_smart_album_media(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<MediaFile>>>, StatusCode> {
    match state.db.get_smart_album_media(&id).await {
        Ok(media) => Ok(Json(ApiResponse::success(media))),
        Err(e) => {
            tracing::error!("Failed to get smart album media: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSmartAlbumRequest {
    pub name: String,
    pub description: Option<String>,
    pub filter: SmartAlbumFilter,
}

pub async fn create_smart_album(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSmartAlbumRequest>,
) -> Result<Json<ApiResponse<SmartAlbum>>, StatusCode> {
    use crate::scanner::smart_albums::SmartAlbumManager;
    
    let manager = SmartAlbumManager::new(state.db.as_ref().clone());
    
    match manager.create_smart_album(req.name, req.description, req.filter).await {
        Ok(album) => Ok(Json(ApiResponse::success(album))),
        Err(e) => {
            tracing::error!("Failed to create smart album: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn refresh_smart_album(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    use crate::scanner::smart_albums::SmartAlbumManager;
    
    let manager = SmartAlbumManager::new(state.db.as_ref().clone());
    
    match manager.refresh_smart_album(&id).await {
        Ok(()) => Ok(Json(ApiResponse::success(()))),
        Err(e) => {
            tracing::error!("Failed to refresh smart album: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_default_smart_albums(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<SmartAlbum>>>, StatusCode> {
    use crate::scanner::smart_albums::SmartAlbumManager;
    
    let manager = SmartAlbumManager::new(state.db.as_ref().clone());
    
    match manager.create_default_smart_albums().await {
        Ok(albums) => Ok(Json(ApiResponse::success(albums))),
        Err(e) => {
            tracing::error!("Failed to create default smart albums: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn serve_index() -> Html<String> {
    match fs::read_to_string("static/index.html").await {
        Ok(html) => Html(html),
        Err(e) => {
            tracing::error!("Failed to read index.html: {}", e);
            // Fallback to embedded HTML
            Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Medianator - Media Catalog</title>
</head>
<body>
    <div style="text-align: center; padding: 50px; font-family: Arial, sans-serif;">
        <h1>Medianator</h1>
        <p>Error loading the main interface.</p>
        <p style="color: #666;">Make sure the static files are present in the static/ directory.</p>
        <p>Error: Could not read static/index.html</p>
        <hr>
        <p>API is running. You can access:</p>
        <ul style="list-style: none; padding: 0;">
            <li><a href="/health">/health</a> - Health check</li>
            <li><a href="/api/stats">/api/stats</a> - Statistics</li>
            <li><a href="/api/media">/api/media</a> - Media list</li>
            <li><a href="/metrics">/metrics</a> - Prometheus metrics</li>
        </ul>
    </div>
</body>
</html>
            "#.to_string())
        }
    }
}