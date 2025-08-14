use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};
use serde::{Deserialize, Serialize};

use crate::models::Face;

#[derive(Debug, Serialize, Deserialize)]
struct DetectedFace {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

pub struct OpenCVFaceDetector {
    cascade_path: Option<PathBuf>,
    enabled: bool,
}

impl OpenCVFaceDetector {
    pub fn new() -> Result<Self> {
        info!("Initializing OpenCV face detector");
        
        // Check if Python and OpenCV are available
        let python_check = Command::new("python3")
            .arg("-c")
            .arg("import cv2; print(cv2.__version__)")
            .output();
            
        match python_check {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                info!("OpenCV Python version: {}", version.trim());
                
                // Find cascade file
                match Self::find_cascade() {
                    Ok(cascade_path) => {
                        info!("OpenCV face detector initialized with cascade at: {}", cascade_path.display());
                        Ok(Self {
                            cascade_path: Some(cascade_path),
                            enabled: true,
                        })
                    }
                    Err(e) => {
                        warn!("Failed to find cascade file: {}", e);
                        Ok(Self {
                            cascade_path: None,
                            enabled: false,
                        })
                    }
                }
            }
            _ => {
                warn!("OpenCV Python not available. Face detection will be disabled.");
                Ok(Self {
                    cascade_path: None,
                    enabled: false,
                })
            }
        }
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
        ];
        
        for path in possible_paths {
            let p = PathBuf::from(path);
            if p.exists() {
                return Ok(p);
            }
        }
        
        bail!("Could not find Haar Cascade file")
    }
    
    pub async fn detect_faces(&self, image_path: &Path, media_id: &str) -> Result<Vec<Face>> {
        if !self.enabled || self.cascade_path.is_none() {
            return Ok(Vec::new());
        }
        
        let cascade_path = self.cascade_path.as_ref().unwrap();
        info!("Detecting faces using OpenCV Python: {}", image_path.display());
        
        // Create Python script to detect faces
        let python_script = format!(r#"
import cv2
import json
import sys

image_path = '{}'
cascade_path = '{}'

# Load the cascade
face_cascade = cv2.CascadeClassifier(cascade_path)

# Read the image
img = cv2.imread(image_path)
if img is None:
    print(json.dumps([]))
    sys.exit(0)

# Convert to grayscale
gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)

# Detect faces
faces = face_cascade.detectMultiScale(
    gray,
    scaleFactor=1.1,
    minNeighbors=5,
    minSize=(30, 30)
)

# Convert to JSON
result = []
for (x, y, w, h) in faces:
    result.append({{'x': int(x), 'y': int(y), 'width': int(w), 'height': int(h)}})

print(json.dumps(result))
"#, 
            image_path.to_string_lossy().replace('\'', "\\'"),
            cascade_path.to_string_lossy().replace('\'', "\\'")
        );
        
        // Execute Python script
        let output = Command::new("python3")
            .arg("-c")
            .arg(&python_script)
            .output()
            .context("Failed to execute OpenCV Python script")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("OpenCV Python script failed: {}", stderr);
        }
        
        // Parse the JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detected_faces: Vec<DetectedFace> = serde_json::from_str(stdout.trim())
            .context("Failed to parse face detection results")?;
        
        info!("OpenCV detected {} faces in {}", detected_faces.len(), image_path.display());
        
        // Convert to Face objects
        let mut faces = Vec::new();
        for (i, detection) in detected_faces.iter().enumerate() {
            let face = Face {
                id: format!("{}_{}", media_id, i),
                media_file_id: media_id.to_string(),
                face_embedding: self.create_embedding(detection),
                face_bbox: format!("{},{},{},{}", 
                    detection.x, detection.y, detection.width, detection.height),
                confidence: 0.95,  // OpenCV Haar cascades don't provide confidence
                detected_at: chrono::Utc::now(),
            };
            faces.push(face);
        }
        
        Ok(faces)
    }
    
    fn create_embedding(&self, detection: &DetectedFace) -> String {
        // Create a simple embedding based on face location and size
        // In a real implementation, this would extract facial features
        let embedding = vec![
            detection.x as f32 / 1000.0,
            detection.y as f32 / 1000.0,
            detection.width as f32 / 1000.0,
            detection.height as f32 / 1000.0,
            0.95,  // confidence placeholder
        ];
        
        base64_encode(&embedding)
    }
}

// Helper function for base64 encoding
use base64::Engine;

fn base64_encode(data: &[f32]) -> String {
    let bytes: Vec<u8> = data.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}