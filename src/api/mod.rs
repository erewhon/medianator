pub mod handlers;
pub mod metrics;
pub mod websocket;
pub mod album_generator;

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};

use crate::db::Database;
use crate::scanner::MediaScanner;
use handlers::AppState;
use metrics::MetricsMiddleware;

pub fn create_app(db: Database, scanner: MediaScanner) -> Router {
    let state = Arc::new(AppState {
        db: Arc::new(db),
        scanner: Arc::new(scanner),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(handlers::health_check))
        .route("/ws", get(websocket::websocket_handler))
        .route("/api/media", get(handlers::list_media))
        .route("/api/media/search", get(handlers::search_media))
        .route("/api/media/:id", get(handlers::get_media_by_id))
        .route("/api/media/:id/metadata", put(handlers::update_media_metadata))
        .route("/api/media/:id/image", get(handlers::get_image))
        .route("/api/media/:id/video", get(handlers::get_video))
        .route("/api/media/:id/audio", get(handlers::get_audio))
        .route("/api/media/:id/thumbnail", get(handlers::get_thumbnail))
        .route("/api/media/:id/faces", get(handlers::get_faces_for_media))
        .route("/api/media/:id/sub-images", get(handlers::get_sub_images))
        .route("/api/sub-images/:id/parent", get(handlers::get_parent_image))
        .route("/api/scan", post(handlers::start_scan))
        .route("/api/stats", get(handlers::get_stats))
        .route("/api/scan/history", get(handlers::get_scan_history))
        .route("/api/upload", post(handlers::upload_file))
        .route("/api/duplicates", get(handlers::get_duplicates))
        .route("/api/duplicates/stats", get(handlers::get_duplicate_stats))
        .route("/api/duplicates/cleanup", get(handlers::suggest_duplicate_cleanup))
        .route("/api/duplicates/archive", post(handlers::archive_duplicates))
        .route("/api/faces/groups", get(handlers::get_face_groups))
        .route("/api/faces/groups", post(handlers::create_face_group))
        .route("/api/faces/groups/add", post(handlers::add_face_to_group))
        .route("/api/faces/grouped", get(handlers::get_faces_grouped))
        .route("/api/faces/:face_id/thumbnail", get(handlers::get_face_thumbnail))
        .route("/api/media/:id/reprocess", post(handlers::reprocess_media))
        .route("/api/media/:id/convert", post(handlers::convert_media))
        .route("/api/batch/reprocess", post(handlers::batch_reprocess))
        // Media Groups endpoints
        .route("/api/groups", get(handlers::get_media_groups))
        .route("/api/groups/:id", get(handlers::get_media_group))
        .route("/api/groups/auto", post(handlers::auto_group_media))
        // Smart Albums endpoints
        .route("/api/albums", get(handlers::get_smart_albums))
        .route("/api/albums", post(handlers::create_smart_album))
        .route("/api/albums/defaults", post(handlers::create_default_smart_albums))
        .route("/api/albums/:id", get(handlers::get_smart_album))
        .route("/api/albums/:id/media", get(handlers::get_smart_album_media))
        .route("/api/albums/:id/refresh", post(handlers::refresh_smart_album))
        // Stories endpoints
        .route("/api/stories", get(handlers::get_stories))
        .route("/api/stories", post(handlers::create_story))
        .route("/api/stories/:id", get(handlers::get_story))
        .route("/api/stories/:id", delete(handlers::delete_story))
        .route("/api/stories/:id/items", post(handlers::add_story_item))
        .route("/api/stories/:story_id/items/:media_id", delete(handlers::remove_story_item))
        // Transcription endpoints
        .route("/api/transcribe", post(handlers::transcribe_media))
        .route("/api/media/:id/detect-scenes", post(handlers::detect_scenes))
        .route("/api/media/:id/classify", post(handlers::classify_photo))
        .route("/api/media/:id/detect-objects", post(handlers::detect_objects))
        .route("/api/transcriptions/media/:media_id", get(handlers::get_transcription))
        .route("/api/transcriptions/:id", delete(handlers::delete_transcription))
        .route("/api/transcriptions/search", get(handlers::search_transcriptions))
        // Auto Albums endpoints
        .route("/api/auto-albums/generate", post(handlers::generate_albums))
        .route("/api/auto-albums", get(handlers::get_auto_albums))
        .route("/api/auto-albums/:id/media", get(handlers::get_auto_album_media))
        .route("/metrics", get(metrics::metrics_handler))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(MetricsMiddleware::new())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}