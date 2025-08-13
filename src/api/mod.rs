pub mod handlers;
pub mod metrics;

use axum::{
    routing::{get, post},
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
        .route("/api/media", get(handlers::list_media))
        .route("/api/media/search", get(handlers::search_media))
        .route("/api/media/:id", get(handlers::get_media_by_id))
        .route("/api/media/:id/image", get(handlers::get_image))
        .route("/api/media/:id/thumbnail", get(handlers::get_thumbnail))
        .route("/api/media/:id/faces", get(handlers::get_faces_for_media))
        .route("/api/scan", post(handlers::start_scan))
        .route("/api/stats", get(handlers::get_stats))
        .route("/api/scan/history", get(handlers::get_scan_history))
        .route("/api/upload", post(handlers::upload_file))
        .route("/api/duplicates", get(handlers::get_duplicates))
        .route("/api/duplicates/stats", get(handlers::get_duplicate_stats))
        .route("/api/duplicates/cleanup", get(handlers::suggest_duplicate_cleanup))
        .route("/api/faces/groups", get(handlers::get_face_groups))
        .route("/api/faces/groups", post(handlers::create_face_group))
        .route("/api/faces/groups/add", post(handlers::add_face_to_group))
        .route("/api/faces/grouped", get(handlers::get_faces_grouped))
        .route("/api/media/:id/reprocess", post(handlers::reprocess_media))
        .route("/api/batch/reprocess", post(handlers::batch_reprocess))
        .route("/metrics", get(metrics::metrics_handler))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(MetricsMiddleware::new())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}