use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn, error};
use opencv::{
    core::{Mat, Rect, Scalar, Size, Vector},
    dnn,
    imgcodecs,
    imgproc,
    objdetect::CascadeClassifier,
    prelude::*,
};
use image::{DynamicImage, RgbImage};

use crate::models::Face;

const CONFIDENCE_THRESHOLD: f32 = 0.5;
const NMS_THRESHOLD: f32 = 0.4;
const INPUT_WIDTH: i32 = 300;
const INPUT_HEIGHT: i32 = 300;

pub struct MLFaceDetector {
    detector_type: DetectorType,
    enabled: bool,
}

enum DetectorType {
    YuNet(dnn::Net),
    CaffeDNN(dnn::Net),
    HaarCascade(CascadeClassifier),
    None,
}

impl MLFaceDetector {
    pub fn new() -> Result<Self> {
        // Try to load models in order of preference
        let models_dir = PathBuf::from("models");
        
        // Try YuNet first (most accurate)
        if let Ok(detector) = Self::load_yunet(&models_dir) {
            info!("Loaded YuNet face detection model");
            return Ok(Self {
                detector_type: detector,
                enabled: true,
            });
        }
        
        // Try Caffe DNN model
        if let Ok(detector) = Self::load_caffe_dnn(&models_dir) {
            info!("Loaded Caffe DNN face detection model");
            return Ok(Self {
                detector_type: detector,
                enabled: true,
            });
        }
        
        // Try Haar Cascade as fallback
        if let Ok(detector) = Self::load_haar_cascade(&models_dir) {
            info!("Loaded Haar Cascade face detection model");
            return Ok(Self {
                detector_type: detector,
                enabled: true,
            });
        }
        
        warn!("No face detection models found. Run ./download_models.sh to download models.");
        Ok(Self {
            detector_type: DetectorType::None,
            enabled: false,
        })
    }
    
    fn load_yunet(models_dir: &Path) -> Result<DetectorType> {
        let model_path = models_dir.join("face_detection_yunet_2023mar.onnx");
        if !model_path.exists() {
            bail!("YuNet model not found at {:?}", model_path);
        }
        
        let net = dnn::read_net_from_onnx(&model_path.to_string_lossy())?;
        Ok(DetectorType::YuNet(net))
    }
    
    fn load_caffe_dnn(models_dir: &Path) -> Result<DetectorType> {
        let model_path = models_dir.join("opencv_face_detector.caffemodel");
        let config_path = models_dir.join("opencv_face_detector.prototxt");
        
        if !model_path.exists() || !config_path.exists() {
            bail!("Caffe model files not found");
        }
        
        let net = dnn::read_net_from_caffe(
            &config_path.to_string_lossy(),
            &model_path.to_string_lossy()
        )?;
        
        Ok(DetectorType::CaffeDNN(net))
    }
    
    fn load_haar_cascade(models_dir: &Path) -> Result<DetectorType> {
        let cascade_path = models_dir.join("haarcascade_frontalface_default.xml");
        if !cascade_path.exists() {
            bail!("Haar Cascade model not found at {:?}", cascade_path);
        }
        
        let mut cascade = CascadeClassifier::new(&cascade_path.to_string_lossy())?;
        Ok(DetectorType::HaarCascade(cascade))
    }
    
    pub async fn detect_faces(&self, image_path: &Path, media_id: &str) -> Result<Vec<Face>> {
        if !self.enabled {
            debug!("ML face detection is disabled");
            return Ok(Vec::new());
        }
        
        info!("Detecting faces in: {} using ML model", image_path.display());
        
        // Load image using OpenCV
        let img = imgcodecs::imread(
            &image_path.to_string_lossy(),
            imgcodecs::IMREAD_COLOR
        )?;
        
        if img.empty() {
            bail!("Failed to load image: {}", image_path.display());
        }
        
        // Detect faces based on detector type
        let face_rects = match &self.detector_type {
            DetectorType::YuNet(net) => self.detect_with_yunet(&img, net)?,
            DetectorType::CaffeDNN(net) => self.detect_with_dnn(&img, net)?,
            DetectorType::HaarCascade(cascade) => self.detect_with_haar(&img, cascade)?,
            DetectorType::None => {
                debug!("No face detector available");
                Vec::new()
            }
        };
        
        info!("Detected {} faces in {}", face_rects.len(), image_path.display());
        
        // Convert to Face objects
        let mut faces = Vec::new();
        for (i, (rect, confidence)) in face_rects.iter().enumerate() {
            let face = Face {
                id: format!("{}_{}", media_id, i),
                media_file_id: media_id.to_string(),
                face_embedding: self.extract_face_embedding(&img, rect)?,
                face_bbox: format!("{},{},{},{}", rect.x, rect.y, rect.width, rect.height),
                confidence: *confidence,
                detected_at: chrono::Utc::now(),
            };
            faces.push(face);
        }
        
        Ok(faces)
    }
    
    fn detect_with_yunet(&self, img: &Mat, net: &dnn::Net) -> Result<Vec<(Rect, f32)>> {
        // YuNet expects specific input size
        let input_size = Size::new(img.cols(), img.rows());
        
        // Create face detector
        let mut detector = dnn::FaceDetectorYN::create(
            "", // model path not needed as we pass the net
            "", // config path
            input_size,
            CONFIDENCE_THRESHOLD,
            NMS_THRESHOLD,
            5000, // top_k
            dnn::DNN_BACKEND_DEFAULT,
            dnn::DNN_TARGET_CPU
        )?;
        
        detector.set_input_size(input_size)?;
        
        let mut faces = Mat::default();
        detector.detect(img, &mut faces)?;
        
        let mut face_rects = Vec::new();
        
        // Parse detection results
        for i in 0..faces.rows() {
            let confidence = *faces.at_2d::<f32>(i, 14)?;
            if confidence > CONFIDENCE_THRESHOLD {
                let x = *faces.at_2d::<f32>(i, 0)? as i32;
                let y = *faces.at_2d::<f32>(i, 1)? as i32;
                let w = *faces.at_2d::<f32>(i, 2)? as i32;
                let h = *faces.at_2d::<f32>(i, 3)? as i32;
                
                face_rects.push((Rect::new(x, y, w, h), confidence));
            }
        }
        
        Ok(face_rects)
    }
    
    fn detect_with_dnn(&self, img: &Mat, net: &dnn::Net) -> Result<Vec<(Rect, f32)>> {
        let mut net = net.clone();
        
        // Prepare input blob
        let blob = dnn::blob_from_image(
            img,
            1.0,
            Size::new(INPUT_WIDTH, INPUT_HEIGHT),
            Scalar::new(104.0, 177.0, 123.0, 0.0),
            false,
            false,
            opencv::core::CV_32F
        )?;
        
        net.set_input(&blob, "", 1.0, Scalar::default())?;
        
        // Forward pass
        let mut output = Mat::default();
        net.forward(&mut output, "")?;
        
        // Parse detections
        let mut face_rects = Vec::new();
        let detection_mat = Mat::from_slice_2d(&[
            [output.cols(), output.rows()],
            [output.cols(), 7]
        ])?;
        
        for i in 0..detection_mat.rows() {
            let confidence = *detection_mat.at_2d::<f32>(i, 2)?;
            
            if confidence > CONFIDENCE_THRESHOLD {
                let x1 = (*detection_mat.at_2d::<f32>(i, 3)? * img.cols() as f32) as i32;
                let y1 = (*detection_mat.at_2d::<f32>(i, 4)? * img.rows() as f32) as i32;
                let x2 = (*detection_mat.at_2d::<f32>(i, 5)? * img.cols() as f32) as i32;
                let y2 = (*detection_mat.at_2d::<f32>(i, 6)? * img.rows() as f32) as i32;
                
                let rect = Rect::new(x1, y1, x2 - x1, y2 - y1);
                face_rects.push((rect, confidence));
            }
        }
        
        Ok(face_rects)
    }
    
    fn detect_with_haar(&self, img: &Mat, cascade: &CascadeClassifier) -> Result<Vec<(Rect, f32)>> {
        let mut gray = Mat::default();
        imgproc::cvt_color(img, &mut gray, imgproc::COLOR_BGR2GRAY, 0)?;
        
        let mut faces = Vector::<Rect>::new();
        cascade.detect_multi_scale(
            &gray,
            &mut faces,
            1.1,  // scale factor
            3,    // min neighbors
            0,    // flags
            Size::new(30, 30),  // min size
            Size::new(0, 0)     // max size
        )?;
        
        // Haar cascade doesn't provide confidence, use a fixed value
        let face_rects: Vec<(Rect, f32)> = faces
            .iter()
            .map(|rect| (rect, 0.8))
            .collect();
        
        Ok(face_rects)
    }
    
    fn extract_face_embedding(&self, img: &Mat, face_rect: &Rect) -> Result<String> {
        // Extract face region
        let face_roi = Mat::roi(img, *face_rect)?;
        
        // Simple embedding: resize to fixed size and compute histogram
        let mut resized = Mat::default();
        imgproc::resize(
            &face_roi,
            &mut resized,
            Size::new(64, 64),
            0.0,
            0.0,
            imgproc::INTER_LINEAR
        )?;
        
        // Convert to grayscale for histogram
        let mut gray = Mat::default();
        imgproc::cvt_color(&resized, &mut gray, imgproc::COLOR_BGR2GRAY, 0)?;
        
        // Calculate histogram as simple embedding
        let mut hist = Mat::default();
        let channels = Vector::<i32>::from_slice(&[0]);
        let hist_size = Vector::<i32>::from_slice(&[256]);
        let ranges = Vector::<f32>::from_slice(&[0.0, 256.0]);
        
        imgproc::calc_hist(
            &Vector::<Mat>::from_slice(&[gray]),
            &channels,
            &Mat::default(),
            &mut hist,
            &hist_size,
            &ranges,
            false
        )?;
        
        // Normalize histogram
        opencv::core::normalize(&hist, &mut hist, 0.0, 1.0, opencv::core::NORM_MINMAX, -1, &Mat::default())?;
        
        // Convert to vector and encode
        let mut embedding = Vec::new();
        for i in 0..256 {
            embedding.push(*hist.at::<f32>(i)?);
        }
        
        // Add spatial information
        embedding.push(face_rect.x as f32 / img.cols() as f32);
        embedding.push(face_rect.y as f32 / img.rows() as f32);
        embedding.push(face_rect.width as f32 / img.cols() as f32);
        embedding.push(face_rect.height as f32 / img.rows() as f32);
        
        Ok(base64_encode(&embedding))
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