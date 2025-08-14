// Alternative implementation that doesn't use OpenCV directly in async contexts
// This avoids Send/Sync issues with OpenCV's raw pointers

use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use tracing::info;
use crate::models::Face;

pub struct OpenCVRustDetector {
    cascade_path: PathBuf,
    enabled: bool,
}

impl OpenCVRustDetector {
    pub fn new() -> Result<Self> {
        info!("Initializing OpenCV Rust face detector (v2)");
        
        // Find cascade file
        let cascade_path = Self::find_cascade()?;
        info!("Found cascade at: {}", cascade_path.display());
        
        Ok(Self {
            cascade_path,
            enabled: true,
        })
    }
    
    fn find_cascade() -> Result<PathBuf> {
        // Check local models directory first
        let local_cascade = PathBuf::from("models/haarcascade_frontalface_default.xml");
        if local_cascade.exists() {
            return Ok(local_cascade);
        }
        
        // Try to find OpenCV's cascade files
        let possible_paths = vec![
            "/opt/homebrew/share/opencv4/haarcascades/haarcascade_frontalface_default.xml",
            "/usr/local/share/opencv4/haarcascades/haarcascade_frontalface_default.xml",
            "/usr/share/opencv4/haarcascades/haarcascade_frontalface_default.xml",
            "/opt/homebrew/Cellar/opencv/4.10.0_12/share/opencv4/haarcascades/haarcascade_frontalface_default.xml",
        ];
        
        for path in possible_paths {
            let p = PathBuf::from(path);
            if p.exists() {
                return Ok(p);
            }
        }
        
        bail!("Could not find Haar Cascade file. Please download it to models/haarcascade_frontalface_default.xml")
    }
    
    pub async fn detect_faces(&self, image_path: &Path, media_id: &str) -> Result<Vec<Face>> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        
        info!("Detecting faces using OpenCV Rust (v2): {}", image_path.display());
        
        // Spawn blocking task to handle OpenCV operations
        let image_path = image_path.to_path_buf();
        let media_id = media_id.to_string();
        let cascade_path = self.cascade_path.clone();
        
        let faces = tokio::task::spawn_blocking(move || -> Result<Vec<Face>> {
            Self::detect_faces_sync(&cascade_path, &image_path, &media_id)
        })
        .await
        .context("Failed to spawn blocking task")??;
        
        Ok(faces)
    }
    
    #[cfg(feature = "opencv-face")]
    fn detect_faces_sync(cascade_path: &Path, image_path: &Path, media_id: &str) -> Result<Vec<Face>> {
        use opencv::{core, imgcodecs, imgproc, objdetect, prelude::*};
        
        // Load the cascade
        let cascade = objdetect::CascadeClassifier::new(&cascade_path.to_string_lossy())
            .context("Failed to load Haar cascade")?;
        
        // Verify cascade is loaded
        if cascade.empty()? {
            bail!("Cascade classifier is empty");
        }
        
        // Load the image
        let img = imgcodecs::imread(
            &image_path.to_string_lossy(),
            imgcodecs::IMREAD_COLOR,
        ).context("Failed to load image")?;
        
        if img.empty() {
            bail!("Failed to load image: empty");
        }
        
        // Convert to grayscale
        let mut gray = Mat::default();
        imgproc::cvt_color(&img, &mut gray, imgproc::COLOR_BGR2GRAY, 0, core::AlgorithmHint::ALGO_HINT_DEFAULT)
            .context("Failed to convert to grayscale")?;
        
        // Equalize histogram for better detection
        let mut equalized = Mat::default();
        imgproc::equalize_hist(&gray, &mut equalized)
            .context("Failed to equalize histogram")?;
        
        // Detect faces
        let mut faces = core::Vector::<core::Rect>::new();
        
        // Use cascade directly (it's already mutable in the newer API)
        let mut cascade_mut = cascade;
        cascade_mut.detect_multi_scale(
            &equalized,
            &mut faces,
            1.1,  // scale factor
            5,    // min neighbors
            0,    // flags
            core::Size::new(30, 30),  // min size
            core::Size::new(0, 0),    // max size (0 = no limit)
        ).context("Failed to detect faces")?;
        
        info!("OpenCV Rust detected {} faces in {}", faces.len(), image_path.display());
        
        // Convert to Face objects
        let mut result = Vec::new();
        for (i, rect) in faces.iter().enumerate() {
            let face = Face {
                id: format!("{}_{}", media_id, i),
                media_file_id: media_id.to_string(),
                face_embedding: Self::create_embedding(&rect),
                face_bbox: format!("{},{},{},{}", 
                    rect.x, rect.y, rect.width, rect.height),
                confidence: 0.95,  // Haar cascades don't provide confidence
                detected_at: chrono::Utc::now(),
            };
            result.push(face);
        }
        
        Ok(result)
    }
    
    #[cfg(not(feature = "opencv-face"))]
    fn detect_faces_sync(_cascade_path: &Path, _image_path: &Path, _media_id: &str) -> Result<Vec<Face>> {
        bail!("OpenCV face detection not available. Build with --features opencv-face")
    }
    
    #[cfg(feature = "opencv-face")]
    fn create_embedding(rect: &opencv::core::Rect) -> String {
        // Create a simple embedding based on face location and size
        let embedding = vec![
            rect.x as f32 / 1000.0,
            rect.y as f32 / 1000.0,
            rect.width as f32 / 1000.0,
            rect.height as f32 / 1000.0,
            0.95,  // confidence placeholder
        ];
        
        base64_encode(&embedding)
    }
}

// Helper function for base64 encoding
#[cfg(feature = "opencv-face")]
use base64::Engine;

#[cfg(feature = "opencv-face")]
fn base64_encode(data: &[f32]) -> String {
    let bytes: Vec<u8> = data.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}