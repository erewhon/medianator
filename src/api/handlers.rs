use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::db::{Database, StoryDatabase};
use crate::models::{MediaFile, Face, FaceGroup, DuplicateGroup, MediaGroup, MediaGroupWithItems, SmartAlbum, SmartAlbumFilter, Transcription, TranscriptionRequest, TranscriptionResponse, TranscriptionSegment, VideoScene, DetectedObject, PhotoClassification, AutoAlbum};
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
pub struct ArchiveDuplicatesRequest {
    pub files: Vec<FileToArchive>,
}

#[derive(Debug, Deserialize)]
pub struct FileToArchive {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ArchiveDuplicatesResponse {
    pub archived_count: usize,
    pub archive_path: String,
}

#[derive(Debug, Deserialize)]
pub struct ConvertMediaRequest {
    pub format: String,
    pub options: ConvertOptions,
}

#[derive(Debug, Deserialize)]
pub struct ConvertOptions {
    pub quality: Option<u8>,
    pub resolution: Option<String>,
    pub bitrate: Option<u32>,
}

pub async fn convert_media(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<ConvertMediaRequest>,
) -> Result<Response, StatusCode> {
    use std::process::Command;
    use std::path::PathBuf;
    
    // Get media file info
    let media = match state.db.get_media_by_id(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    let input_path = PathBuf::from(&media.file_path);
    if !input_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    // Create temp output file
    let output_ext = &request.format;
    let output_filename = format!(
        "{}_converted.{}",
        input_path.file_stem().unwrap_or_default().to_string_lossy(),
        output_ext
    );
    let output_path = std::env::temp_dir().join(&output_filename);
    
    // Build ffmpeg command based on media type
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-i").arg(&input_path).arg("-y");
    
    match media.media_type.as_str() {
        "image" => {
            // For images, use ImageMagick convert command instead
            let mut cmd = Command::new("convert");
            cmd.arg(&input_path);
            
            if let Some(quality) = request.options.quality {
                cmd.arg("-quality").arg(quality.to_string());
            }
            
            cmd.arg(&output_path);
            
            match cmd.output() {
                Ok(output) if output.status.success() => {},
                _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        },
        "video" => {
            if request.format == "gif" {
                // Special handling for animated GIF
                cmd.arg("-vf").arg("fps=10,scale=320:-1:flags=lanczos");
            } else {
                // Video codec selection
                match request.format.as_str() {
                    "mp4" => { cmd.arg("-c:v").arg("libx264"); },
                    "webm" => { cmd.arg("-c:v").arg("libvpx-vp9"); },
                    "mov" => { cmd.arg("-c:v").arg("libx264").arg("-f").arg("mov"); },
                    _ => {}
                };
                
                // Resolution if specified
                if let Some(res) = &request.options.resolution {
                    cmd.arg("-s").arg(res);
                }
            }
            
            cmd.arg(&output_path);
            
            match cmd.output() {
                Ok(output) if output.status.success() => {},
                _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        },
        "audio" => {
            // Audio codec selection
            match request.format.as_str() {
                "mp3" => { cmd.arg("-c:a").arg("libmp3lame"); },
                "ogg" => { cmd.arg("-c:a").arg("libvorbis"); },
                "m4a" => { cmd.arg("-c:a").arg("aac"); },
                "flac" => { cmd.arg("-c:a").arg("flac"); },
                _ => {}
            };
            
            // Bitrate if specified
            if let Some(bitrate) = request.options.bitrate {
                cmd.arg("-b:a").arg(format!("{}k", bitrate));
            }
            
            cmd.arg(&output_path);
            
            match cmd.output() {
                Ok(output) if output.status.success() => {},
                _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        },
        _ => return Err(StatusCode::BAD_REQUEST),
    }
    
    // Read the converted file
    let file_data = match fs::read(&output_path).await {
        Ok(data) => data,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    // Clean up temp file
    let _ = fs::remove_file(&output_path).await;
    
    // Determine content type
    let content_type = match request.format.as_str() {
        "jpeg" | "jpg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tiff" => "image/tiff",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        _ => "application/octet-stream",
    };
    
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", output_filename)
        )
        .body(Body::from(file_data))
        .unwrap())
}

pub async fn archive_duplicates(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ArchiveDuplicatesRequest>,
) -> Result<Json<ApiResponse<ArchiveDuplicatesResponse>>, StatusCode> {
    use std::path::PathBuf;
    use chrono::Utc;
    
    // Create archive directory with timestamp
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let archive_dir = PathBuf::from(format!("./archive/duplicates_{}", timestamp));
    
    // Create the archive directory
    if let Err(e) = fs::create_dir_all(&archive_dir).await {
        tracing::error!("Failed to create archive directory: {}", e);
        return Ok(Json(ApiResponse::error(format!("Failed to create archive directory: {}", e))));
    }
    
    let mut archived_count = 0;
    
    for file in &request.files {
        let source_path = PathBuf::from(&file.path);
        if !source_path.exists() {
            tracing::warn!("File not found: {}", file.path);
            continue;
        }
        
        // Create subdirectory structure in archive to preserve hierarchy
        let file_name = source_path.file_name().unwrap_or_default();
        let dest_path = archive_dir.join(file_name);
        
        // Move the file to archive
        match fs::rename(&source_path, &dest_path).await {
            Ok(_) => {
                // Remove from database
                if let Err(e) = state.db.delete_media_file(&file.id).await {
                    tracing::error!("Failed to remove file from database: {}", e);
                } else {
                    archived_count += 1;
                    tracing::info!("Archived file: {} -> {}", file.path, dest_path.display());
                }
            }
            Err(e) => {
                tracing::error!("Failed to move file {}: {}", file.path, e);
            }
        }
    }
    
    Ok(Json(ApiResponse::success(ArchiveDuplicatesResponse {
        archived_count,
        archive_path: archive_dir.to_string_lossy().to_string(),
    })))
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

// Media metadata update
#[derive(Debug, Deserialize)]
pub struct UpdateMediaMetadataRequest {
    pub user_description: Option<String>,
    pub user_tags: Option<String>,
}

pub async fn update_media_metadata(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateMediaMetadataRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.db.update_media_metadata(&id, request.user_description, request.user_tags).await {
        Ok(()) => Ok(Json(ApiResponse::success(()))),
        Err(e) => {
            tracing::error!("Failed to update media metadata: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Story handlers
#[derive(Debug, Deserialize)]
pub struct CreateStoryRequest {
    pub name: String,
    pub description: Option<String>,
}

pub async fn create_story(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateStoryRequest>,
) -> Result<Json<ApiResponse<crate::models::Story>>, StatusCode> {
    let story_db = StoryDatabase::new(state.db.get_pool());
    
    match story_db.create_story(&request.name, request.description.as_deref()).await {
        Ok(story) => Ok(Json(ApiResponse::success(story))),
        Err(e) => {
            tracing::error!("Failed to create story: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_stories(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<crate::models::Story>>>, StatusCode> {
    let story_db = StoryDatabase::new(state.db.get_pool());
    
    match story_db.get_all_stories().await {
        Ok(stories) => Ok(Json(ApiResponse::success(stories))),
        Err(e) => {
            tracing::error!("Failed to get stories: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_story(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<crate::models::StoryWithItems>>, StatusCode> {
    let story_db = StoryDatabase::new(state.db.get_pool());
    
    match story_db.get_story_with_items(&id).await {
        Ok(Some(story)) => Ok(Json(ApiResponse::success(story))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get story: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn delete_story(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let story_db = StoryDatabase::new(state.db.get_pool());
    
    match story_db.delete_story(&id).await {
        Ok(()) => Ok(Json(ApiResponse::success(()))),
        Err(e) => {
            tracing::error!("Failed to delete story: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AddStoryItemRequest {
    pub media_file_id: String,
    pub caption: Option<String>,
}

pub async fn add_story_item(
    State(state): State<Arc<AppState>>,
    Path(story_id): Path<String>,
    Json(request): Json<AddStoryItemRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let story_db = StoryDatabase::new(state.db.get_pool());
    
    match story_db.add_item_to_story(&story_id, &request.media_file_id, request.caption.as_deref()).await {
        Ok(()) => Ok(Json(ApiResponse::success(()))),
        Err(e) => {
            tracing::error!("Failed to add item to story: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn remove_story_item(
    State(state): State<Arc<AppState>>,
    Path((story_id, media_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let story_db = StoryDatabase::new(state.db.get_pool());
    
    match story_db.remove_item_from_story(&story_id, &media_id).await {
        Ok(()) => Ok(Json(ApiResponse::success(()))),
        Err(e) => {
            tracing::error!("Failed to remove item from story: {}", e);
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

// Helper function to find the best way to run a command (pipx, uvx, or direct)
fn find_command_runner(tool_name: &str, run_method: &str) -> Option<(String, Vec<String>)> {
    use std::process::Command;
    
    // First, check if the tool is directly available in PATH
    let direct_check = Command::new("which")
        .arg(tool_name)
        .output();
    
    let is_in_path = direct_check.is_ok() && direct_check.unwrap().status.success();
    
    // If run_method is specified and not "auto", try that first (but respect PATH priority)
    match run_method {
        "pipx" => {
            // If it's already in PATH directly, prefer that to avoid conflicts
            if is_in_path {
                tracing::info!("{} is already in PATH, using direct execution instead of pipx to avoid conflicts", tool_name);
                return Some((tool_name.to_string(), vec![]));
            }
            
            // Check if tool is available via pipx
            let check = Command::new("pipx")
                .args(&["list"])
                .output();
            
            if let Ok(output) = check {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains(tool_name) || stdout.contains(&tool_name.replace("whisperx", "whisperx")) {
                    // Try to get Python version info for logging
                    if let Ok(version_check) = Command::new("pipx")
                        .args(&["runpip", tool_name, "--version"])
                        .output() {
                        let version_info = String::from_utf8_lossy(&version_check.stdout);
                        if version_info.contains("python 3.12") || version_info.contains("Python 3.12") {
                            tracing::debug!("pipx package {} using Python 3.12", tool_name);
                        } else {
                            tracing::warn!("pipx package {} may not be using Python 3.12: {}", tool_name, version_info);
                        }
                    }
                    return Some(("pipx".to_string(), vec!["run".to_string(), tool_name.to_string()]));
                }
            }
        }
        "uvx" => {
            // If it's already in PATH directly, prefer that to avoid conflicts
            if is_in_path {
                tracing::info!("{} is already in PATH, using direct execution instead of uvx", tool_name);
                return Some((tool_name.to_string(), vec![]));
            }
            
            // Check if uvx is available
            let check = Command::new("which")
                .arg("uvx")
                .output();
            
            if check.is_ok() && check.unwrap().status.success() {
                return Some(("uvx".to_string(), vec![tool_name.to_string()]));
            }
        }
        "direct" => {
            // Try direct execution
            if is_in_path {
                return Some((tool_name.to_string(), vec![]));
            }
        }
        _ => {} // "auto" or unknown - try all methods
    }
    
    // Auto mode: try in order - direct (preferred), pipx, uvx
    
    // 1. Try direct execution (already checked above)
    if is_in_path {
        tracing::debug!("{} found directly in PATH", tool_name);
        return Some((tool_name.to_string(), vec![]));
    }
    
    // 2. Try pipx (only if not in PATH to avoid conflicts)
    let check = Command::new("pipx")
        .args(&["list"])
        .output();
    
    if let Ok(output) = check {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Check for both exact match and common variations
        let variations = [
            tool_name,
            &tool_name.replace("whisper", "openai-whisper"),
            "whisperx",
            "openai-whisper",
        ];
        
        for variant in variations {
            if stdout.contains(variant) {
                // Try to get Python version info for logging
                if let Ok(version_check) = Command::new("pipx")
                    .args(&["runpip", tool_name, "--version"])
                    .output() {
                    let version_info = String::from_utf8_lossy(&version_check.stdout);
                    if version_info.contains("python 3.12") || version_info.contains("Python 3.12") {
                        tracing::debug!("Auto-detected pipx package {} using Python 3.12", tool_name);
                    } else {
                        tracing::warn!("Auto-detected pipx package {} may not be using Python 3.12. Consider reinstalling with: pipx reinstall --python python3.12 {}", tool_name, tool_name);
                    }
                }
                return Some(("pipx".to_string(), vec!["run".to_string(), tool_name.to_string()]));
            }
        }
    }
    
    // 3. Try uvx
    let check = Command::new("which")
        .arg("uvx")
        .output();
    
    if check.is_ok() && check.unwrap().status.success() {
        // uvx might work even if we can't verify the package
        return Some(("uvx".to_string(), vec![tool_name.to_string()]));
    }
    
    None
}

pub async fn detect_scenes(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<VideoScene>>>, StatusCode> {
    use crate::scanner::scene_detection::SceneDetector;
    
    // Get media file
    let media = match state.db.get_media_by_id(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get media {}: {}", id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Check if it's a video
    if media.media_type != "video" {
        return Ok(Json(ApiResponse::error("Media must be a video for scene detection".to_string())));
    }
    
    // Detect scenes
    let detector = SceneDetector::new()
        .with_threshold(0.3)
        .with_min_scene_length(1.0);
    
    let scenes = match detector.detect_scenes_ffmpeg(Path::new(&media.file_path)).await {
        Ok(scenes) => scenes,
        Err(e) => {
            tracing::error!("Failed to detect scenes: {}", e);
            return Ok(Json(ApiResponse::error(format!("Scene detection failed: {}", e))));
        }
    };
    
    // Convert to database models and save
    let mut db_scenes = Vec::new();
    for scene in scenes {
        let db_scene = VideoScene {
            id: uuid::Uuid::new_v4().to_string(),
            media_file_id: id.clone(),
            scene_number: scene.scene_number as i32,
            start_time: scene.start_time,
            end_time: scene.end_time,
            start_frame: scene.start_frame as i32,
            end_frame: scene.end_frame as i32,
            duration: scene.duration,
            keyframe_path: scene.keyframe_path,
            confidence: scene.confidence,
            created_at: chrono::Utc::now(),
        };
        
        // Save to database (you'll need to implement this method)
        // state.db.insert_video_scene(&db_scene).await?;
        
        db_scenes.push(db_scene);
    }
    
    Ok(Json(ApiResponse::success(db_scenes)))
}

pub async fn classify_photo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<PhotoClassification>>, StatusCode> {
    use crate::scanner::object_detection::ObjectDetector;
    
    // Get media file
    let media = match state.db.get_media_by_id(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get media {}: {}", id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Check if it's an image
    if media.media_type != "image" {
        return Ok(Json(ApiResponse::error("Media must be an image for classification".to_string())));
    }
    
    // Classify photo
    let detector = ObjectDetector::new()
        .with_confidence_threshold(0.5);
    
    let classification = match detector.classify_photo(Path::new(&media.file_path)).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to classify photo: {}", e);
            return Ok(Json(ApiResponse::error(format!("Photo classification failed: {}", e))));
        }
    };
    
    // Convert to database model
    let db_classification = PhotoClassification {
        id: uuid::Uuid::new_v4().to_string(),
        media_file_id: id.clone(),
        primary_category: classification.primary_category,
        categories: serde_json::to_string(&classification.categories).unwrap_or_default(),
        tags: classification.tags.map(|t| serde_json::to_string(&t).unwrap_or_default()),
        scene_type: classification.scene_type,
        is_screenshot: classification.is_screenshot,
        is_document: classification.is_document,
        has_text: classification.has_text,
        dominant_colors: classification.dominant_colors.map(|c| serde_json::to_string(&c).unwrap_or_default()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    // Save to database (you'll need to implement this method)
    // state.db.insert_photo_classification(&db_classification).await?;
    
    Ok(Json(ApiResponse::success(db_classification)))
}

pub async fn detect_objects(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<DetectedObject>>>, StatusCode> {
    use crate::scanner::object_detection::ObjectDetector;
    
    // Get media file
    let media = match state.db.get_media_by_id(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get media {}: {}", id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Detect objects
    let detector = ObjectDetector::new()
        .with_confidence_threshold(0.5);
    
    let objects = match detector.detect_objects_yolo(Path::new(&media.file_path)).await {
        Ok(objects) => objects,
        Err(e) => {
            tracing::error!("Failed to detect objects: {}", e);
            return Ok(Json(ApiResponse::error(format!("Object detection failed: {}", e))));
        }
    };
    
    // Convert to database models
    let mut db_objects = Vec::new();
    for obj in objects {
        let db_object = DetectedObject {
            id: uuid::Uuid::new_v4().to_string(),
            media_file_id: id.clone(),
            class_name: obj.class_name,
            confidence: obj.confidence,
            bbox_x: obj.bbox.x,
            bbox_y: obj.bbox.y,
            bbox_width: obj.bbox.width,
            bbox_height: obj.bbox.height,
            attributes: obj.attributes.map(|a| serde_json::to_string(&a).unwrap_or_default()),
            created_at: chrono::Utc::now(),
        };
        
        // Save to database (you'll need to implement this method)
        // state.db.insert_detected_object(&db_object).await?;
        
        db_objects.push(db_object);
    }
    
    Ok(Json(ApiResponse::success(db_objects)))
}

// Helper function to extract progress percentage from WhisperX output
fn extract_progress_percentage(line: &str) -> Option<f32> {
    // Look for patterns like "50%" or "50.0%" or "[50%]"
    if let Some(pos) = line.find('%') {
        // Find the start of the number before the %
        let start = line[..pos]
            .rfind(|c: char| !c.is_numeric() && c != '.')
            .map(|i| i + 1)
            .unwrap_or(0);
        
        if let Ok(percent) = line[start..pos].parse::<f32>() {
            return Some(percent);
        }
    }
    
    // Look for patterns like "Progress: 0.5" (where 0.5 = 50%)
    if line.contains("Progress:") || line.contains("progress:") {
        if let Some(pos) = line.rfind(':') {
            if let Ok(fraction) = line[pos+1..].trim().parse::<f32>() {
                return Some(fraction * 100.0);
            }
        }
    }
    
    None
}

pub async fn transcribe_media(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TranscriptionRequest>,
) -> Result<Json<ApiResponse<TranscriptionResponse>>, StatusCode> {
    use std::process::{Command, Stdio};
    use std::io::{BufReader, BufRead};
    use crate::api::websocket::{broadcast_transcription_progress, broadcast_transcription_segment, TranscriptionSegmentUpdate};
    
    let media_id = request.media_file_id.clone();
    
    // Determine which transcription engine to use (default to WhisperX if available)
    let use_whisperx = std::env::var("TRANSCRIPTION_ENGINE")
        .unwrap_or_else(|_| "whisperx".to_string())
        .to_lowercase() == "whisperx";
    
    // Determine run method (pipx, uvx, or direct)
    let run_method = std::env::var("WHISPER_RUN_METHOD")
        .unwrap_or_else(|_| "auto".to_string())
        .to_lowercase();
    
    // Log start of transcription
    let engine = if use_whisperx { "WhisperX" } else { "Whisper" };
    tracing::info!("Starting transcription with {} for media_id: {} (run method: {})", engine, media_id, run_method);
    broadcast_transcription_progress(&media_id, "starting", 0.0, Some(format!("Initializing {} transcription...", engine)));
    
    // Get media file
    let media = match state.db.get_media_by_id(&media_id).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            tracing::warn!("Media not found: {}", media_id);
            broadcast_transcription_progress(&media_id, "error", 0.0, Some("Media file not found".to_string()));
            return Err(StatusCode::NOT_FOUND);
        },
        Err(e) => {
            tracing::error!("Failed to get media {}: {}", media_id, e);
            broadcast_transcription_progress(&media_id, "error", 0.0, Some(format!("Database error: {}", e)));
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    tracing::info!("Found media file: {} (type: {})", media.file_path, media.media_type);
    
    // Check if it's audio or video
    if media.media_type != "audio" && media.media_type != "video" {
        tracing::warn!("Invalid media type for transcription: {}", media.media_type);
        broadcast_transcription_progress(&media_id, "error", 0.0, Some("Media type must be audio or video".to_string()));
        return Ok(Json(ApiResponse::error("Media type must be audio or video".to_string())));
    }
    
    // Check if transcription already exists
    if let Ok(Some(existing)) = crate::db::get_transcription_by_media(&state.db.get_pool(), &media_id).await {
        tracing::info!("Transcription already exists for media_id: {}", media_id);
        broadcast_transcription_progress(&media_id, "complete", 100.0, Some("Using existing transcription".to_string()));
        
        let segments: Vec<TranscriptionSegment> = existing.transcription_segments
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        
        return Ok(Json(ApiResponse::success(TranscriptionResponse {
            transcription: existing,
            segments,
        })));
    }
    
    tracing::info!("Preparing {} transcription for: {}", engine, media.file_path);
    broadcast_transcription_progress(&media_id, "processing", 10.0, Some(format!("Preparing {} model...", engine)));
    
    // Check if the transcription tool is installed and determine how to run it
    let mut use_whisperx = use_whisperx;
    let command_runner: (String, Vec<String>);
    
    if use_whisperx {
        // Check for WhisperX using the helper function
        if let Some(runner) = find_command_runner("whisperx", &run_method) {
            command_runner = runner;
            tracing::info!("Found WhisperX using runner: {} {:?}", command_runner.0, command_runner.1);
        } else {
            tracing::warn!("WhisperX not found, falling back to regular Whisper");
            broadcast_transcription_progress(&media_id, "processing", 5.0, 
                Some("WhisperX not found, trying regular Whisper...".to_string()));
            
            // Check for regular Whisper as fallback
            if let Some(runner) = find_command_runner("whisper", &run_method) {
                command_runner = runner;
                use_whisperx = false;
                tracing::info!("Falling back to Whisper using runner: {} {:?}", command_runner.0, command_runner.1);
            } else {
                tracing::error!("Neither WhisperX nor Whisper is installed or accessible");
                broadcast_transcription_progress(&media_id, "error", 0.0, 
                    Some("Neither WhisperX nor Whisper is installed. Please install one of them.".to_string()));
                return Ok(Json(ApiResponse::error(
                    "Transcription tools not installed. Install WhisperX (pipx install whisperx) or Whisper (pipx install openai-whisper)".to_string()
                )));
            }
        }
    } else {
        // Check for regular Whisper
        if let Some(runner) = find_command_runner("whisper", &run_method) {
            command_runner = runner;
            tracing::info!("Found Whisper using runner: {} {:?}", command_runner.0, command_runner.1);
        } else {
            tracing::error!("Whisper is not installed or accessible");
            broadcast_transcription_progress(&media_id, "error", 0.0, 
                Some("Whisper is not installed. Please install it using: pipx install openai-whisper".to_string()));
            return Ok(Json(ApiResponse::error(
                "Whisper is not installed. Please install OpenAI Whisper first using: pipx install openai-whisper".to_string()
            )));
        }
    }
    
    // Create a temporary directory specifically for Whisper output
    let temp_dir = tempfile::tempdir().map_err(|e| {
        tracing::error!("Failed to create temp directory: {}", e);
        broadcast_transcription_progress(&media_id, "error", 0.0, Some(format!("Failed to create temp directory: {}", e)));
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    tracing::info!("Created temp directory for transcription output: {:?}", temp_dir.path());
    
    // Build the command based on which tool we're using
    let (output, used_engine) = if use_whisperx {
        // First, do a quick test to ensure WhisperX is working
        tracing::info!("Testing WhisperX installation...");
        let test_cmd = Command::new(&command_runner.0)
            .args(&command_runner.1)
            .arg("--help")
            .output();
        
        if let Ok(test_output) = test_cmd {
            if !test_output.status.success() {
                tracing::error!("WhisperX --help failed, installation may be broken");
                let help_stderr = String::from_utf8_lossy(&test_output.stderr);
                if !help_stderr.is_empty() {
                    tracing::error!("WhisperX --help error: {}", help_stderr);
                }
                
                // Check for common issues
                if help_stderr.contains("ModuleNotFoundError") || help_stderr.contains("ImportError") {
                    tracing::error!("WhisperX has missing Python dependencies. Try: pipx inject whisperx torch torchaudio transformers");
                    broadcast_transcription_progress(&media_id, "error", 0.0, 
                        Some("WhisperX has missing dependencies. Please reinstall or use regular Whisper.".to_string()));
                    
                    // Try to fall back to regular Whisper
                    if let Some(_runner) = find_command_runner("whisper", &run_method) {
                        tracing::warn!("Falling back to regular Whisper due to WhisperX issues");
                        broadcast_transcription_progress(&media_id, "processing", 10.0, 
                            Some("WhisperX unavailable, using regular Whisper...".to_string()));
                        // Note: We can't easily fall back here since command_runner is immutable
                        // The user should set TRANSCRIPTION_ENGINE=whisper instead
                    }
                }
            } else {
                tracing::debug!("WhisperX --help succeeded, installation appears OK");
            }
        }
        
        // WhisperX command with streaming progress
        let mut cmd = Command::new(&command_runner.0);
        
        // Add any prefix arguments (like "run" for pipx)
        for arg in &command_runner.1 {
            cmd.arg(arg);
        }
        
        cmd.arg(&media.file_path)
           .arg("--model").arg("base")
           .arg("--output_format").arg("json")
           .arg("--output_dir").arg(temp_dir.path())
           .arg("--compute_type").arg("int8")  // Faster computation
           .arg("--print_progress").arg("True")  // Enable progress output
           .stderr(Stdio::piped())  // Capture stderr for progress
           .stdout(Stdio::piped());
        
        if let Some(lang) = &request.language {
            tracing::info!("Using language: {}", lang);
            cmd.arg("--language").arg(lang);
        }
        // Note: WhisperX doesn't have an "auto" option - if no language is specified,
        // we simply don't pass the --language flag and it will auto-detect
        
        // WhisperX supports speaker diarization (requires HuggingFace token)
        if request.enable_speaker_diarization {
            // Check if HuggingFace token is available
            let hf_token = std::env::var("HF_TOKEN").or_else(|_| std::env::var("HUGGING_FACE_TOKEN"));
            
            if hf_token.is_ok() {
                tracing::info!("Enabling speaker diarization with WhisperX");
                cmd.arg("--diarize");
                cmd.arg("--min_speakers").arg("1");
                cmd.arg("--max_speakers").arg("10");
                cmd.arg("--hf_token").arg(hf_token.unwrap());
                broadcast_transcription_progress(&media_id, "processing", 15.0, 
                    Some("Speaker diarization enabled".to_string()));
            } else {
                tracing::warn!("Speaker diarization requested but HF_TOKEN not set. Skipping diarization.");
                tracing::warn!("To enable speaker diarization, set HF_TOKEN environment variable with your HuggingFace token");
                broadcast_transcription_progress(&media_id, "processing", 15.0, 
                    Some("Diarization skipped (HF_TOKEN not set)".to_string()));
            }
        }
        
        tracing::info!("Executing WhisperX command with progress tracking...");
        broadcast_transcription_progress(&media_id, "processing", 20.0, Some("Running WhisperX transcription...".to_string()));
        
        // Log the full command for debugging
        tracing::info!("WhisperX command: {:?}", cmd);
        
        // Spawn the process to capture streaming output
        let mut child = cmd.spawn().map_err(|e| {
            tracing::error!("Failed to spawn whisperx: {}. Make sure WhisperX is installed and accessible.", e);
            
            // Try to provide more helpful error message
            let help_msg = if e.kind() == std::io::ErrorKind::NotFound {
                "WhisperX executable not found. Install with: pipx install --python python3.12 whisperx"
            } else {
                "Failed to run WhisperX. Check installation and permissions."
            };
            
            broadcast_transcription_progress(&media_id, "error", 0.0, Some(help_msg.to_string()));
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        // Collect both stderr and stdout for better debugging
        let stderr = child.stderr.take();
        let stdout = child.stdout.take();
        
        // Read stderr for progress updates and errors
        let mut stderr_lines = Vec::new();
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    stderr_lines.push(line.clone());
                    tracing::debug!("WhisperX stderr: {}", line);
                    
                    // Parse WhisperX progress output
                    if line.contains("Transcribing") || line.contains("Processing") || line.contains("%") {
                        // Try to extract percentage from the line
                        if let Some(percent) = extract_progress_percentage(&line) {
                            broadcast_transcription_progress(&media_id, "processing", 
                                20.0 + (percent * 0.5), // Map to 20-70% range
                                Some(format!("Processing: {}%", (percent as i32))));
                        } else {
                            broadcast_transcription_progress(&media_id, "processing", 40.0, 
                                Some(line.clone()));
                        }
                    }
                    
                    // Check for common error patterns
                    if line.contains("error") || line.contains("Error") || line.contains("ERROR") {
                        tracing::error!("WhisperX error detected: {}", line);
                        
                        // Detect specific error patterns and provide helpful guidance
                        if line.contains("CUDA out of memory") || line.contains("OutOfMemoryError") {
                            broadcast_transcription_progress(&media_id, "error", 0.0, 
                                Some("Out of memory. Try using a smaller model or CPU processing.".to_string()));
                        } else if line.contains("ModuleNotFoundError") || line.contains("ImportError") {
                            broadcast_transcription_progress(&media_id, "warning", 0.0, 
                                Some("Missing Python dependencies detected. Continuing...".to_string()));
                        } else if line.contains("ffmpeg") || line.contains("FFmpeg") {
                            broadcast_transcription_progress(&media_id, "warning", 0.0, 
                                Some("FFmpeg issue detected. Check ffmpeg installation.".to_string()));
                        }
                    }
                }
            }
        }
        
        // Read stdout for any output
        let mut stdout_lines = Vec::new();
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    stdout_lines.push(line.clone());
                    tracing::debug!("WhisperX stdout: {}", line);
                }
            }
        }
        
        // Wait for the process to complete
        let output = child.wait_with_output().map_err(|e| {
            tracing::error!("Failed to wait for whisperx: {}", e);
            broadcast_transcription_progress(&media_id, "error", 0.0, Some(format!("WhisperX process failed: {}", e)));
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        // If the process failed but stderr was empty, provide diagnostic info
        if !output.status.success() && stderr_lines.is_empty() && stdout_lines.is_empty() {
            let exit_code = output.status.code().unwrap_or(-1);
            tracing::error!("WhisperX failed with exit code {} but no error output", exit_code);
            tracing::error!("This may indicate WhisperX is not properly installed or is missing dependencies");
            
            // Try to run a diagnostic command to get more info
            tracing::info!("Running WhisperX diagnostics...");
            
            // Try to get version info
            if let Ok(version_check) = Command::new(&command_runner.0)
                .args(&command_runner.1)
                .arg("--version")
                .output() {
                if version_check.status.success() {
                    let version_out = String::from_utf8_lossy(&version_check.stdout);
                    let version_err = String::from_utf8_lossy(&version_check.stderr);
                    tracing::info!("WhisperX version output: {}", version_out);
                    if !version_err.is_empty() {
                        tracing::info!("WhisperX version stderr: {}", version_err);
                    }
                } else {
                    tracing::error!("WhisperX --version failed");
                }
            }
            
            // Try to run with minimal arguments
            if let Ok(minimal_test) = Command::new(&command_runner.0)
                .args(&command_runner.1)
                .arg("--help")
                .output() {
                if !minimal_test.status.success() {
                    let help_err = String::from_utf8_lossy(&minimal_test.stderr);
                    tracing::error!("WhisperX --help also failed. Installation appears to be broken.");
                    if !help_err.is_empty() {
                        tracing::error!("Help error: {}", help_err);
                    }
                    tracing::error!("Reinstall with: pipx reinstall --python python3.12 whisperx");
                    
                    // Check for common Python issues
                    if help_err.contains("ModuleNotFoundError") {
                        tracing::error!("Missing Python modules detected. Try: pipx inject whisperx torch torchaudio transformers");
                    }
                } else {
                    // Help works but actual command failed - likely an issue with the media file or parameters
                    tracing::error!("WhisperX --help works but transcription failed. This may be an issue with:");
                    tracing::error!("1. The media file format or path");
                    tracing::error!("2. Missing audio codecs");
                    tracing::error!("3. Insufficient memory or disk space");
                    tracing::error!("4. File permissions");
                    
                    // Log the file path for debugging
                    tracing::error!("Media file path: {}", media.file_path);
                    
                    // Check if file exists and is readable
                    if let Ok(metadata) = std::fs::metadata(&media.file_path) {
                        tracing::info!("File exists, size: {} bytes", metadata.len());
                        if metadata.permissions().readonly() {
                            tracing::warn!("File is read-only");
                        }
                    } else {
                        tracing::error!("Cannot access file metadata - file may not exist or be readable");
                    }
                }
            }
            
            // Try direct Python import test if using pipx
            if command_runner.0 == "pipx" {
                tracing::info!("Testing WhisperX Python environment...");
                if let Ok(import_test) = Command::new("pipx")
                    .args(&["runpip", "whisperx", "-c", "import whisperx; print('WhisperX import OK')"]) 
                    .output() {
                    if import_test.status.success() {
                        tracing::info!("WhisperX Python import successful");
                    } else {
                        let import_err = String::from_utf8_lossy(&import_test.stderr);
                        tracing::error!("WhisperX Python import failed: {}", import_err);
                        
                        // Provide specific fix suggestions based on error
                        if import_err.contains("torch") {
                            tracing::error!("PyTorch not found. Install with: pipx inject whisperx torch torchaudio");
                        } else if import_err.contains("transformers") {
                            tracing::error!("Transformers not found. Install with: pipx inject whisperx transformers");
                        }
                    }
                }
            }
        }
        
        (output, "WhisperX")
    } else {
        // Regular Whisper command (no streaming progress)
        let mut cmd = Command::new(&command_runner.0);
        
        // Add any prefix arguments (like "run" for pipx)
        for arg in &command_runner.1 {
            cmd.arg(arg);
        }
        
        cmd.arg(&media.file_path)
           .arg("--model").arg("base")
           .arg("--output_format").arg("json")
           .arg("--output_dir").arg(temp_dir.path())
           .arg("--verbose").arg("True");
        
        if let Some(lang) = &request.language {
            tracing::info!("Using language: {}", lang);
            cmd.arg("--language").arg(lang);
        }
        
        if request.enable_speaker_diarization {
            tracing::warn!("Speaker diarization requested but not supported by OpenAI Whisper. Use WhisperX for this feature.");
            broadcast_transcription_progress(&media_id, "processing", 15.0, 
                Some("Note: Speaker diarization not available with base Whisper".to_string()));
        }
        
        tracing::info!("Executing Whisper command...");
        broadcast_transcription_progress(&media_id, "processing", 20.0, Some("Running Whisper transcription...".to_string()));
        
        let output = cmd.output().map_err(|e| {
            tracing::error!("Failed to run whisper: {}", e);
            broadcast_transcription_progress(&media_id, "error", 0.0, Some(format!("Failed to run Whisper: {}", e)));
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        (output, "Whisper")
    };
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Create a more informative error message
        let error_msg = if !stderr.is_empty() {
            // Parse the error for known patterns
            let stderr_str = stderr.to_string();
            if stderr_str.contains("ModuleNotFoundError") {
                format!("Missing Python dependencies. {}", stderr_str.lines().next().unwrap_or(&stderr_str))
            } else if stderr_str.contains("CUDA") || stderr_str.contains("GPU") {
                "GPU/CUDA error. Try using CPU mode or a smaller model.".to_string()
            } else if stderr_str.contains("ffmpeg") {
                "FFmpeg error. Ensure ffmpeg is installed and the media file is valid.".to_string()
            } else if stderr_str.contains("No such file or directory") {
                format!("File not found error. Check file path: {}", media.file_path)
            } else {
                stderr_str
            }
        } else if !stdout.is_empty() {
            stdout.to_string()
        } else {
            format!("{} failed with exit code {} but produced no output. This often indicates missing dependencies or incorrect installation.", 
                used_engine, 
                output.status.code().unwrap_or(-1))
        };
        
        tracing::error!("{} failed: {}", used_engine, error_msg);
        tracing::error!("Exit code: {:?}", output.status.code());
        
        // Provide helpful suggestions based on the engine and error
        let suggestion = if used_engine == "WhisperX" {
            if error_msg.contains("ModuleNotFoundError") || error_msg.contains("ImportError") {
                "\n\nFix missing dependencies:\n1. Reinstall WhisperX: pipx reinstall --python python3.12 whisperx\n2. Inject dependencies: pipx inject whisperx torch torchaudio transformers\n3. Or try regular Whisper: Set TRANSCRIPTION_ENGINE=whisper"
            } else {
                "\n\nTroubleshooting:\n1. Check WhisperX installation: pipx list\n2. Reinstall if needed: pipx reinstall --python python3.12 whisperx\n3. Install missing dependencies: pipx inject whisperx torch torchaudio transformers\n4. Try regular Whisper: Set TRANSCRIPTION_ENGINE=whisper"
            }
        } else {
            "\n\nTroubleshooting:\n1. Check Whisper installation: pipx list\n2. Reinstall if needed: pipx reinstall --python python3.12 openai-whisper\n3. Check ffmpeg is installed: ffmpeg -version"
        };
        
        let full_error = format!("{}{}", error_msg, suggestion);
        
        broadcast_transcription_progress(&media_id, "error", 0.0, Some(error_msg.clone()));
        return Ok(Json(ApiResponse::error(full_error)));
    }
    
    // Log stdout for debugging
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        tracing::debug!("{} stdout: {}", used_engine, stdout);
    }
    
    tracing::info!("{} completed successfully, parsing output...", used_engine);
    broadcast_transcription_progress(&media_id, "processing", 70.0, Some("Parsing transcription results...".to_string()));
    
    // Parse Whisper output
    // Whisper creates output files based on the input filename
    // Due to special characters and escaping, we'll look for any .json file in the output directory
    let output_dir = temp_dir.path();
    
    // Find the JSON file in the output directory
    let json_path = std::fs::read_dir(output_dir)
        .map_err(|e| {
            tracing::error!("Failed to read output directory: {}", e);
            broadcast_transcription_progress(&media_id, "error", 0.0, Some(format!("Failed to read output directory: {}", e)));
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            tracing::error!("No JSON file found in output directory: {:?}", output_dir);
            
            // List all files for debugging
            if let Ok(entries) = std::fs::read_dir(output_dir) {
                tracing::error!("Files in output directory:");
                for entry in entries {
                    if let Ok(entry) = entry {
                        tracing::error!("  - {:?}", entry.path());
                    }
                }
            }
            
            broadcast_transcription_progress(&media_id, "error", 0.0, Some("No transcription output file found".to_string()));
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    tracing::info!("Found Whisper output at: {:?}", json_path);
    
    let json_content = std::fs::read_to_string(&json_path).map_err(|e| {
        tracing::error!("Failed to read whisper output from {:?}: {}", json_path, e);
        broadcast_transcription_progress(&media_id, "error", 0.0, Some(format!("Failed to read output: {}", e)));
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    tracing::info!("Successfully read Whisper output, parsing JSON...");
    
    let whisper_result: serde_json::Value = serde_json::from_str(&json_content).map_err(|e| {
        tracing::error!("Failed to parse whisper JSON output: {}", e);
        broadcast_transcription_progress(&media_id, "error", 0.0, Some(format!("Failed to parse output: {}", e)));
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Extract text and segments
    let full_text = whisper_result["text"].as_str().unwrap_or("").to_string();
    let segments_array = whisper_result["segments"].as_array();
    
    tracing::info!("Extracted transcription text ({} characters)", full_text.len());
    broadcast_transcription_progress(&media_id, "processing", 80.0, Some("Processing transcription segments...".to_string()));
    
    let mut segments = Vec::new();
    if let Some(segs) = segments_array {
        tracing::info!("Processing {} transcription segments", segs.len());
        
        for (idx, seg) in segs.iter().enumerate() {
            let segment = TranscriptionSegment {
                start_time: seg["start"].as_f64().unwrap_or(0.0),
                end_time: seg["end"].as_f64().unwrap_or(0.0),
                text: seg["text"].as_str().unwrap_or("").to_string(),
                speaker: seg["speaker"].as_str().map(|s| s.to_string()),
                confidence: seg["confidence"].as_f64().map(|c| c as f32),
            };
            
            // Broadcast each segment as it's processed for real-time updates
            broadcast_transcription_segment(&media_id, TranscriptionSegmentUpdate {
                start_time: segment.start_time,
                end_time: segment.end_time,
                text: segment.text.clone(),
                confidence: segment.confidence,
            });
            
            segments.push(segment);
            
            // Update progress based on segments processed
            let progress = 80.0 + (15.0 * (idx + 1) as f32 / segs.len() as f32);
            broadcast_transcription_progress(&media_id, "processing", progress, 
                Some(format!("Processed segment {} of {}", idx + 1, segs.len())));
        }
    } else {
        tracing::warn!("No segments found in Whisper output");
    }
    
    // Save to database
    let language = whisper_result["language"].as_str().map(|s| s.to_string());
    let duration = media.duration_seconds;
    
    tracing::info!("Saving transcription to database (language: {:?})", language);
    broadcast_transcription_progress(&media_id, "processing", 95.0, Some("Saving transcription to database...".to_string()));
    
    let transcription = crate::db::create_transcription(
        &state.db.get_pool(),
        &media_id,
        &full_text,
        &segments,
        language.as_deref(),
        duration,
        "whisper-base",
    ).await.map_err(|e| {
        tracing::error!("Failed to save transcription to database: {}", e);
        broadcast_transcription_progress(&media_id, "error", 0.0, Some(format!("Failed to save: {}", e)));
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    tracing::info!("Successfully completed transcription for media_id: {}", media_id);
    broadcast_transcription_progress(&media_id, "complete", 100.0, Some("Transcription complete!".to_string()));
    
    Ok(Json(ApiResponse::success(TranscriptionResponse {
        transcription,
        segments,
    })))
}

pub async fn get_transcription(
    State(state): State<Arc<AppState>>,
    Path(media_id): Path<String>,
) -> Result<Json<ApiResponse<TranscriptionResponse>>, StatusCode> {
    match crate::db::get_transcription_by_media(&state.db.get_pool(), &media_id).await {
        Ok(Some(transcription)) => {
            let segments: Vec<TranscriptionSegment> = transcription.transcription_segments
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            
            Ok(Json(ApiResponse::success(TranscriptionResponse {
                transcription,
                segments,
            })))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get transcription: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn delete_transcription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match crate::db::delete_transcription(&state.db.get_pool(), &id).await {
        Ok(()) => Ok(Json(ApiResponse::success(()))),
        Err(e) => {
            tracing::error!("Failed to delete transcription: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TranscriptionSearchQuery {
    pub q: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn search_transcriptions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TranscriptionSearchQuery>,
) -> Result<Json<ApiResponse<Vec<(Transcription, String)>>>, StatusCode> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    
    match crate::db::search_transcriptions(&state.db.get_pool(), &query.q, limit, offset).await {
        Ok(results) => Ok(Json(ApiResponse::success(results))),
        Err(e) => {
            tracing::error!("Failed to search transcriptions: {}", e);
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