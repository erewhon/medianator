use anyhow::Result;
use image::{DynamicImage, ImageFormat};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info};

const THUMBNAIL_SIZE: u32 = 256;
#[allow(dead_code)]
const THUMBNAIL_QUALITY: u8 = 85;

pub struct ThumbnailGenerator {
    output_dir: PathBuf,
}

impl ThumbnailGenerator {
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    pub async fn ensure_output_dir(&self) -> Result<()> {
        if !self.output_dir.exists() {
            fs::create_dir_all(&self.output_dir).await?;
            info!("Created thumbnail directory: {:?}", self.output_dir);
        }
        Ok(())
    }

    pub async fn generate_thumbnail(
        &self,
        image_path: &Path,
        media_id: &str,
    ) -> Result<PathBuf> {
        self.ensure_output_dir().await?;

        let thumbnail_filename = format!("{}_thumb.jpg", media_id);
        let thumbnail_path = self.output_dir.join(thumbnail_filename);

        // Check if thumbnail already exists
        if thumbnail_path.exists() {
            debug!("Thumbnail already exists: {:?}", thumbnail_path);
            return Ok(thumbnail_path);
        }

        // Load and resize image
        let image_path_owned = image_path.to_owned();
        let thumbnail_path_clone = thumbnail_path.clone();
        
        let result = tokio::task::spawn_blocking(move || {
            generate_thumbnail_sync(&image_path_owned, &thumbnail_path_clone)
        })
        .await??;

        info!("Generated thumbnail for {}: {:?}", media_id, thumbnail_path);
        Ok(result)
    }

    pub async fn generate_video_thumbnail(
        &self,
        video_path: &Path,
        media_id: &str,
    ) -> Result<PathBuf> {
        self.ensure_output_dir().await?;

        let thumbnail_filename = format!("{}_thumb.jpg", media_id);
        let thumbnail_path = self.output_dir.join(thumbnail_filename);

        if thumbnail_path.exists() {
            debug!("Video thumbnail already exists: {:?}", thumbnail_path);
            return Ok(thumbnail_path);
        }

        // Use ffmpeg to extract a frame from the video
        let output = tokio::process::Command::new("ffmpeg")
            .arg("-i")
            .arg(video_path)
            .arg("-ss")
            .arg("00:00:01") // Extract frame at 1 second
            .arg("-vframes")
            .arg("1")
            .arg("-vf")
            .arg(format!("scale={}:{}", THUMBNAIL_SIZE, THUMBNAIL_SIZE))
            .arg("-q:v")
            .arg("2")
            .arg(&thumbnail_path)
            .arg("-y")
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                info!("Generated video thumbnail for {}: {:?}", media_id, thumbnail_path);
                Ok(thumbnail_path)
            }
            Ok(output) => {
                error!("ffmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
                Err(anyhow::anyhow!("Failed to generate video thumbnail"))
            }
            Err(e) => {
                error!("Failed to run ffmpeg: {}", e);
                Err(anyhow::anyhow!("ffmpeg not available: {}", e))
            }
        }
    }

    pub fn get_thumbnail_path(&self, media_id: &str) -> PathBuf {
        self.output_dir.join(format!("{}_thumb.jpg", media_id))
    }
}

fn generate_thumbnail_sync(image_path: &Path, output_path: &Path) -> Result<PathBuf> {
    let img = image::open(image_path)?;
    
    let thumbnail = if img.width() > img.height() {
        img.resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE * img.height() / img.width(), image::imageops::FilterType::Lanczos3)
    } else {
        img.resize(THUMBNAIL_SIZE * img.width() / img.height(), THUMBNAIL_SIZE, image::imageops::FilterType::Lanczos3)
    };

    // Convert to RGB8 if needed (removes alpha channel)
    let thumbnail = DynamicImage::ImageRgb8(thumbnail.to_rgb8());
    
    // Save as JPEG with specified quality
    thumbnail.save_with_format(output_path, ImageFormat::Jpeg)?;
    
    Ok(output_path.to_path_buf())
}