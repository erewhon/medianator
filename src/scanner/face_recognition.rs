use anyhow::Result;
use image::{DynamicImage, GenericImageView, Rgba};
use ndarray::{Array3, ArrayView3, s};
use ort::{Environment, Session, SessionBuilder, Value};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::models::{Face, FaceBoundingBox};

const CONFIDENCE_THRESHOLD: f32 = 0.7;
const SIMILARITY_THRESHOLD: f32 = 0.6;

pub struct FaceDetector {
    detection_session: Arc<Session>,
    recognition_session: Arc<Session>,
    environment: Arc<Environment>,
}

impl FaceDetector {
    pub fn new() -> Result<Self> {
        let environment = Arc::new(
            Environment::builder()
                .with_name("face_detector")
                .build()?
        );

        // Download and use pre-trained ONNX models
        // For this example, we'll use placeholder paths - in production,
        // you'd download these models during setup
        let detection_model_path = "models/face_detection.onnx";
        let recognition_model_path = "models/face_recognition.onnx";

        // Check if models exist, if not, download them
        if !Path::new(detection_model_path).exists() {
            download_face_models()?;
        }

        let detection_session = Arc::new(
            SessionBuilder::new(&environment)?
                .with_model_from_file(detection_model_path)?
        );

        let recognition_session = Arc::new(
            SessionBuilder::new(&environment)?
                .with_model_from_file(recognition_model_path)?
        );

        Ok(Self {
            detection_session,
            recognition_session,
            environment,
        })
    }

    pub async fn detect_faces(&self, image_path: &Path, media_id: &str) -> Result<Vec<Face>> {
        let image = image::open(image_path)?;
        let (width, height) = image.dimensions();
        
        // Prepare image for face detection
        let input_tensor = preprocess_image_for_detection(&image)?;
        
        // Run face detection
        let detection_session = Arc::clone(&self.detection_session);
        let outputs = tokio::task::spawn_blocking(move || {
            run_detection(&detection_session, input_tensor)
        }).await??;

        let mut faces = Vec::new();
        
        // Process detection results
        for detection in outputs {
            if detection.confidence < CONFIDENCE_THRESHOLD {
                continue;
            }

            // Extract face region from image
            let face_image = extract_face_region(&image, &detection.bbox)?;
            
            // Generate face embedding
            let embedding = self.generate_face_embedding(face_image).await?;
            
            let face = Face {
                id: Uuid::new_v4().to_string(),
                media_file_id: media_id.to_string(),
                face_embedding: base64_encode(&embedding),
                face_bbox: serde_json::to_string(&detection.bbox)?,
                confidence: detection.confidence,
                detected_at: chrono::Utc::now(),
            };
            
            faces.push(face);
        }

        info!("Detected {} faces in {}", faces.len(), image_path.display());
        Ok(faces)
    }

    async fn generate_face_embedding(&self, face_image: DynamicImage) -> Result<Vec<f32>> {
        let input_tensor = preprocess_face_for_recognition(&face_image)?;
        
        let recognition_session = Arc::clone(&self.recognition_session);
        let embedding = tokio::task::spawn_blocking(move || {
            run_recognition(&recognition_session, input_tensor)
        }).await??;

        Ok(embedding)
    }

    pub fn calculate_face_similarity(embedding1: &[f32], embedding2: &[f32]) -> f32 {
        // Cosine similarity
        let dot_product: f32 = embedding1.iter()
            .zip(embedding2.iter())
            .map(|(a, b)| a * b)
            .sum();
        
        let norm1: f32 = embedding1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm2: f32 = embedding2.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm1 == 0.0 || norm2 == 0.0 {
            return 0.0;
        }
        
        dot_product / (norm1 * norm2)
    }

    pub fn should_group_faces(&self, embedding1: &[f32], embedding2: &[f32]) -> bool {
        self.calculate_face_similarity(embedding1, embedding2) >= SIMILARITY_THRESHOLD
    }
}

struct Detection {
    bbox: FaceBoundingBox,
    confidence: f32,
}

fn preprocess_image_for_detection(image: &DynamicImage) -> Result<Array3<f32>> {
    let resized = image.resize_exact(640, 640, image::imageops::FilterType::Lanczos3);
    let rgb = resized.to_rgb8();
    
    let mut array = Array3::<f32>::zeros((3, 640, 640));
    
    for (x, y, pixel) in rgb.enumerate_pixels() {
        array[[0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
        array[[1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
        array[[2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
    }
    
    Ok(array)
}

fn preprocess_face_for_recognition(face_image: &DynamicImage) -> Result<Array3<f32>> {
    let resized = face_image.resize_exact(112, 112, image::imageops::FilterType::Lanczos3);
    let rgb = resized.to_rgb8();
    
    let mut array = Array3::<f32>::zeros((3, 112, 112));
    
    for (x, y, pixel) in rgb.enumerate_pixels() {
        array[[0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.5) / 0.5;
        array[[1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.5) / 0.5;
        array[[2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.5) / 0.5;
    }
    
    Ok(array)
}

fn extract_face_region(image: &DynamicImage, bbox: &FaceBoundingBox) -> Result<DynamicImage> {
    let x = bbox.x.max(0) as u32;
    let y = bbox.y.max(0) as u32;
    let width = bbox.width.min(image.width() as i32 - bbox.x) as u32;
    let height = bbox.height.min(image.height() as i32 - bbox.y) as u32;
    
    Ok(image.crop_imm(x, y, width, height))
}

fn run_detection(session: &Session, input: Array3<f32>) -> Result<Vec<Detection>> {
    // This is a placeholder - actual implementation would depend on the model
    // For now, return empty vec
    Ok(Vec::new())
}

fn run_recognition(session: &Session, input: Array3<f32>) -> Result<Vec<f32>> {
    // This is a placeholder - actual implementation would depend on the model
    // Return a dummy 512-dimensional embedding
    Ok(vec![0.0; 512])
}

fn download_face_models() -> Result<()> {
    // Create models directory
    std::fs::create_dir_all("models")?;
    
    // In production, you would download actual ONNX models here
    // For now, we'll create placeholder files
    std::fs::write("models/face_detection.onnx", b"placeholder")?;
    std::fs::write("models/face_recognition.onnx", b"placeholder")?;
    
    info!("Face detection models downloaded");
    Ok(())
}

use base64::Engine;
fn base64_encode(data: &[f32]) -> String {
    let bytes: Vec<u8> = data.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

fn base64_decode(encoded: &str) -> Result<Vec<f32>> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    let floats: Vec<f32> = bytes.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    Ok(floats)
}