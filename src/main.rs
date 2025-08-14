use anyhow::Result;
use medianator::{
    api,
    config::Config,
    db::Database,
    scanner::MediaScanner,
};
use std::net::SocketAddr;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "medianator=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Medianator Media Catalog Engine");

    let config = Config::from_env()?;
    info!("Configuration loaded");

    let db = Database::new(&config.database.url).await?;
    info!("Database initialized");

    let mut scanner = MediaScanner::new(db.clone());
    
    // Enable thumbnail generation if configured
    if let Ok(thumbnails_dir) = std::env::var("THUMBNAILS_DIR") {
        scanner = scanner.with_thumbnail_generator(std::path::PathBuf::from(thumbnails_dir));
        info!("Thumbnail generation enabled");
    }
    
    // Enable face detection if configured
    if std::env::var("ENABLE_FACE_DETECTION").unwrap_or_default() == "true" {
        // Check if OpenCV should be used (default to true)
        let use_opencv = std::env::var("USE_OPENCV")
            .unwrap_or_else(|_| "true".to_string()) == "true";
        
        scanner = match scanner.with_face_detection(use_opencv) {
            Ok(s) => {
                info!("Face detection enabled (OpenCV: {})", use_opencv);
                s
            }
            Err(e) => {
                warn!("Failed to enable face detection: {}", e);
                // Create a new scanner since the old one was consumed
                MediaScanner::new(db.clone())
                    .with_thumbnail_generator(std::path::PathBuf::from(
                        std::env::var("THUMBNAILS_DIR").unwrap_or_default()
                    ))
            }
        };
    }

    if !config.scanner.auto_scan_paths.is_empty() {
        info!("Starting initial scan of configured paths");
        for path in &config.scanner.auto_scan_paths {
            if path.exists() && path.is_dir() {
                info!("Scanning: {}", path.display());
                match scanner.scan_directory(path).await {
                    Ok(stats) => {
                        info!(
                            "Scan completed for {}: {} files scanned, {} added, {} updated",
                            path.display(),
                            stats.files_scanned,
                            stats.files_added,
                            stats.files_updated
                        );
                    }
                    Err(e) => {
                        warn!("Failed to scan {}: {}", path.display(), e);
                    }
                }
            } else {
                warn!("Skipping invalid path: {}", path.display());
            }
        }
    }

    let app = api::create_app(db, scanner);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down");
        },
        _ = terminate => {
            info!("Received terminate signal, shutting down");
        },
    }
}