pub mod metadata;
pub mod thumbnail;
pub mod face_recognition;
pub mod duplicate;
pub mod viola_jones_detector;
pub mod opencv_face_detector;
pub mod sub_image_extractor;

use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

use crate::db::Database;
use crate::models::{MediaMetadata, ScanProgress};
use metadata::MetadataExtractor;
use thumbnail::ThumbnailGenerator;
use viola_jones_detector::ViolaJonesFaceDetector;
use opencv_face_detector::OpenCVFaceDetector;
use sub_image_extractor::{SubImageExtractor, ExtractionMetadata};

pub enum FaceDetectorType {
    ViolaJones(ViolaJonesFaceDetector),
    OpenCV(OpenCVFaceDetector),
}

impl FaceDetectorType {
    pub async fn detect_faces(&self, image_path: &Path, media_id: &str) -> Result<Vec<crate::models::Face>> {
        match self {
            FaceDetectorType::ViolaJones(detector) => detector.detect_faces(image_path, media_id).await,
            FaceDetectorType::OpenCV(detector) => detector.detect_faces(image_path, media_id).await,
        }
    }
}

pub struct MediaScanner {
    db: Database,
    pub thumbnail_generator: Option<ThumbnailGenerator>,
    pub face_detector: Option<FaceDetectorType>,
    pub sub_image_extractor: Option<SubImageExtractor>,
    pub sub_image_output_dir: Option<PathBuf>,
}

impl MediaScanner {
    pub fn new(db: Database) -> Self {
        Self { 
            db,
            thumbnail_generator: None,
            face_detector: None,
            sub_image_extractor: None,
            sub_image_output_dir: None,
        }
    }

    pub fn with_thumbnail_generator(mut self, output_dir: PathBuf) -> Self {
        self.thumbnail_generator = Some(ThumbnailGenerator::new(output_dir));
        self
    }

    pub fn with_face_detection(mut self, use_opencv: bool) -> Result<Self> {
        if use_opencv {
            // Try OpenCV first
            match OpenCVFaceDetector::new() {
                Ok(detector) => {
                    info!("Using OpenCV face detector");
                    self.face_detector = Some(FaceDetectorType::OpenCV(detector));
                }
                Err(e) => {
                    warn!("Failed to initialize OpenCV detector: {}, falling back to Viola-Jones", e);
                    self.face_detector = Some(FaceDetectorType::ViolaJones(ViolaJonesFaceDetector::new()?));
                }
            }
        } else {
            self.face_detector = Some(FaceDetectorType::ViolaJones(ViolaJonesFaceDetector::new()?));
        }
        Ok(self)
    }

    pub fn with_sub_image_extraction(mut self, output_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&output_dir).ok();
        self.sub_image_extractor = Some(SubImageExtractor::new());
        self.sub_image_output_dir = Some(output_dir);
        self
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
                    } else {
                        if is_new {
                            files_added += 1;
                        } else {
                            files_updated += 1;
                        }

                        // Generate thumbnail for images and videos
                        if let Some(ref gen) = self.thumbnail_generator {
                            if metadata.media_type == crate::models::MediaType::Image {
                                match gen.generate_thumbnail(Path::new(&metadata.file_path), &metadata.id).await {
                                    Ok(thumb_path) => {
                                        // Update database with thumbnail path
                                        if let Err(e) = self.db.update_thumbnail_path(&metadata.id, &thumb_path.to_string_lossy()).await {
                                            warn!("Failed to update thumbnail path in database: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to generate thumbnail for {}: {}", metadata.file_path, e);
                                    }
                                }
                            } else if metadata.media_type == crate::models::MediaType::Video {
                                match gen.generate_video_thumbnail(Path::new(&metadata.file_path), &metadata.id).await {
                                    Ok(thumb_path) => {
                                        // Update database with thumbnail path
                                        if let Err(e) = self.db.update_thumbnail_path(&metadata.id, &thumb_path.to_string_lossy()).await {
                                            warn!("Failed to update thumbnail path in database: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to generate video thumbnail for {}: {}", metadata.file_path, e);
                                    }
                                }
                            }
                        }

                        // Extract sub-images for album pages
                        if metadata.media_type == crate::models::MediaType::Image {
                            if let (Some(ref extractor), Some(ref output_dir)) = (&self.sub_image_extractor, &self.sub_image_output_dir) {
                                match extractor.extract_sub_images(Path::new(&metadata.file_path), output_dir).await {
                                    Ok(sub_images) => {
                                        for (sub_image_path, extraction_metadata) in sub_images {
                                            // Process each sub-image as a new media file
                                            if let Ok(mut sub_metadata) = MetadataExtractor::extract(&sub_image_path).await {
                                                // Copy parent metadata
                                                sub_metadata.camera_info = metadata.camera_info.clone();
                                                sub_metadata.timestamps.created = metadata.timestamps.created;
                                                
                                                // Set parent relationship
                                                let extraction_json = serde_json::to_string(&extraction_metadata).ok();
                                                
                                                // Insert sub-image with parent reference
                                                if let Err(e) = self.db.insert_sub_image(&sub_metadata, &metadata.id, extraction_json).await {
                                                    warn!("Failed to insert sub-image: {}", e);
                                                } else {
                                                    // Run face detection on sub-image
                                                    if let Some(ref detector) = self.face_detector {
                                                        match detector.detect_faces(&sub_image_path, &sub_metadata.id).await {
                                                            Ok(faces) => {
                                                                for face in faces {
                                                                    if let Err(e) = self.db.insert_face(&face).await {
                                                                        warn!("Failed to insert face from sub-image: {}", e);
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                warn!("Failed to detect faces in sub-image: {}", e);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        debug!("No sub-images extracted from {}: {}", metadata.file_path, e);
                                    }
                                }
                            }

                            // Detect faces in main image
                            if let Some(ref detector) = self.face_detector {
                                match detector.detect_faces(Path::new(&metadata.file_path), &metadata.id).await {
                                    Ok(faces) => {
                                        let face_count = faces.len();
                                        for face in faces {
                                            if let Err(e) = self.db.insert_face(&face).await {
                                                warn!("Failed to insert face: {}", e);
                                            }
                                        }
                                        
                                        // Auto-group faces after insertion
                                        if face_count > 0 {
                                            if let Err(e) = self.db.auto_group_faces().await {
                                                warn!("Failed to auto-group faces: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to detect faces in {}: {}", metadata.file_path, e);
                                    }
                                }
                            }
                        }
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

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
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