pub mod handlers;
pub mod metrics;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
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
        .route("/api/scan", post(handlers::start_scan))
        .route("/api/stats", get(handlers::get_stats))
        .route("/api/scan/history", get(handlers::get_scan_history))
        .route("/metrics", get(metrics::metrics_handler))
        .layer(MetricsMiddleware::new())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}