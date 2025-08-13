pub mod metadata;

use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use walkdir::WalkDir;

use crate::db::Database;
use crate::models::{MediaMetadata, ScanProgress};
use metadata::MetadataExtractor;

pub struct MediaScanner {
    db: Database,
}

impl MediaScanner {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn scan_directory(&self, path: &Path) -> Result<ScanStats> {
        info!("Starting scan of directory: {}", path.display());
        
        let scan_id = self.db.create_scan_history(&path.to_string_lossy()).await?;
        
        let (tx, mut rx) = mpsc::channel::<ScanItem>(100);
        
        let path_clone = path.to_path_buf();
        let file_walker: JoinHandle<Result<Vec<PathBuf>>> = tokio::task::spawn_blocking(move || {
            let mut paths = Vec::new();
            for entry in WalkDir::new(path_clone)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    if Self::is_media_file(entry.path()) {
                        paths.push(entry.path().to_path_buf());
                    }
                }
            }
            Ok(paths)
        });

        let paths = file_walker.await??;
        let total_files = paths.len();
        info!("Found {} media files to process", total_files);

        let processor = tokio::spawn(async move {
            let mut stats = ScanStats::default();
            
            for path in paths {
                match Self::process_file(&path).await {
                    Ok(metadata) => {
                        if let Err(e) = tx.send(ScanItem::Media(metadata)).await {
                            error!("Failed to send metadata: {}", e);
                            break;
                        }
                        stats.files_scanned += 1;
                    }
                    Err(e) => {
                        warn!("Failed to process file {}: {}", path.display(), e);
                        stats.error_count += 1;
                    }
                }
            }
            
            let _ = tx.send(ScanItem::Complete).await;
            stats
        });

        let mut stats = ScanStats::default();
        let mut files_added = 0;
        let mut files_updated = 0;

        while let Some(item) = rx.recv().await {
            match item {
                ScanItem::Media(metadata) => {
                    let is_new = self.db.get_media_by_path(&metadata.file_path).await?.is_none();
                    
                    if let Err(e) = self.db.insert_media_file(&metadata).await {
                        error!("Failed to insert media file: {}", e);
                        stats.error_count += 1;
                    } else if is_new {
                        files_added += 1;
                    } else {
                        files_updated += 1;
                    }

                    if (files_added + files_updated) % 100 == 0 {
                        self.db
                            .update_scan_progress(
                                scan_id,
                                (files_added + files_updated) as i32,
                                files_added as i32,
                                files_updated as i32,
                                stats.error_count as i32,
                            )
                            .await?;
                    }
                }
                ScanItem::Complete => break,
            }
        }

        let processor_stats = processor.await?;
        stats.files_scanned = processor_stats.files_scanned;
        stats.files_added = files_added;
        stats.files_updated = files_updated;
        stats.error_count += processor_stats.error_count;

        self.db
            .complete_scan(scan_id, if stats.error_count > 0 { "completed" } else { "completed" })
            .await?;

        info!(
            "Scan completed: {} files scanned, {} added, {} updated, {} errors",
            stats.files_scanned, stats.files_added, stats.files_updated, stats.error_count
        );

        Ok(stats)
    }

    async fn process_file(path: &Path) -> Result<MediaMetadata> {
        MetadataExtractor::extract(path).await
    }

    fn is_media_file(path: &Path) -> bool {
        let supported_extensions = [
            "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "tif", "svg", "ico",
            "mp4", "avi", "mov", "wmv", "flv", "mkv", "webm", "m4v", "mpg", "mpeg",
            "mp3", "wav", "flac", "aac", "ogg", "wma", "m4a", "opus", "aiff", "ape",
        ];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| supported_extensions.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }
}

#[derive(Debug, Default)]
pub struct ScanStats {
    pub files_scanned: usize,
    pub files_added: usize,
    pub files_updated: usize,
    pub error_count: usize,
}

enum ScanItem {
    Media(MediaMetadata),
    Complete,
}