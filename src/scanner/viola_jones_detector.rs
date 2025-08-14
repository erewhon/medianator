use anyhow::{Result, Context};
use std::path::Path;
use tracing::info;
use image::GrayImage;
use imageproc::integral_image::{integral_image, sum_image_pixels};
use imageproc::rect::Rect as IRect;

use crate::models::Face;

const CONFIDENCE_THRESHOLD: f32 = 0.7;  // Balance between false positives and negatives
const MIN_FACE_SIZE: u32 = 60;  // Reasonable minimum face size
const MAX_FACE_SIZE: u32 = 600;
const SCALE_FACTOR: f32 = 1.15;  // Finer scale steps
const NMS_IOU_THRESHOLD: f32 = 0.3;  // For non-maximum suppression

pub struct ViolaJonesFaceDetector {
    enabled: bool,
    cascade_features: Vec<HaarFeature>,
}

#[derive(Clone, Debug)]
struct HaarFeature {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    feature_type: FeatureType,
    threshold: f32,
    weight: f32,
}

#[derive(Clone, Debug)]
enum FeatureType {
    TwoRectangleHorizontal,  // Edge features
    TwoRectangleVertical,
    ThreeRectangleHorizontal, // Line features
    #[allow(dead_code)]
    ThreeRectangleVertical,
    FourRectangle,            // Diagonal features
}

impl ViolaJonesFaceDetector {
    pub fn new() -> Result<Self> {
        info!("Initializing Viola-Jones face detector");
        
        // Initialize with basic Haar-like features for face detection
        let cascade_features = Self::initialize_haar_features();
        
        Ok(Self {
            enabled: true,
            cascade_features,
        })
    }
    
    fn initialize_haar_features() -> Vec<HaarFeature> {
        // Define key Haar-like features that are commonly found in faces
        // These are simplified but based on common face patterns
        vec![
            // Eye region features (darker than cheeks)
            HaarFeature {
                x: 20, y: 20, width: 60, height: 30,
                feature_type: FeatureType::TwoRectangleHorizontal,
                threshold: 0.01, weight: 2.0,  // Lower threshold for feature matching
            },
            // Nose bridge feature (vertical)
            HaarFeature {
                x: 45, y: 30, width: 10, height: 40,
                feature_type: FeatureType::TwoRectangleVertical,
                threshold: 0.01, weight: 1.5,
            },
            // Mouth region (horizontal line)
            HaarFeature {
                x: 25, y: 60, width: 50, height: 20,
                feature_type: FeatureType::ThreeRectangleHorizontal,
                threshold: 0.01, weight: 1.8,
            },
            // Forehead to eyes transition
            HaarFeature {
                x: 15, y: 15, width: 70, height: 25,
                feature_type: FeatureType::TwoRectangleHorizontal,
                threshold: 0.01, weight: 1.6,
            },
            // Cheek symmetry
            HaarFeature {
                x: 10, y: 35, width: 80, height: 40,
                feature_type: FeatureType::FourRectangle,
                threshold: 0.01, weight: 1.3,
            },
        ]
    }
    
    pub async fn detect_faces(&self, image_path: &Path, media_id: &str) -> Result<Vec<Face>> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        
        info!("Detecting faces using Viola-Jones algorithm: {}", image_path.display());
        
        // Load and convert to grayscale
        let img = image::open(image_path)
            .context("Failed to open image for face detection")?;
        let gray = img.to_luma8();
        let (width, height) = gray.dimensions();
        
        // Compute integral image for fast feature calculation
        let integral = integral_image(&gray);
        
        // Multi-scale detection
        let mut all_detections = Vec::new();
        let mut scale = 1.0;
        
        while (MIN_FACE_SIZE as f32 * scale) < width.min(height) as f32 {
            let window_size = (MIN_FACE_SIZE as f32 * scale) as u32;
            
            if window_size > MAX_FACE_SIZE {
                break;
            }
            
            // Scan image with sliding window
            let detections = self.scan_image_at_scale(&integral, window_size, width, height);
            all_detections.extend(detections);
            
            scale *= SCALE_FACTOR;
        }
        
        // Apply non-maximum suppression
        let final_detections = self.non_maximum_suppression(all_detections);
        
        info!("Detected {} faces in {}", final_detections.len(), image_path.display());
        
        // Convert to Face objects
        let mut faces = Vec::new();
        for (i, detection) in final_detections.iter().enumerate() {
            let face = Face {
                id: format!("{}_{}", media_id, i),
                media_file_id: media_id.to_string(),
                face_embedding: self.extract_embedding(&gray, detection),
                face_bbox: format!("{},{},{},{}", 
                    detection.x, detection.y, detection.width, detection.height),
                confidence: detection.confidence,
                detected_at: chrono::Utc::now(),
            };
            faces.push(face);
        }
        
        Ok(faces)
    }
    
    fn scan_image_at_scale(
        &self,
        integral: &GrayImage,
        window_size: u32,
        img_width: u32,
        img_height: u32,
    ) -> Vec<Detection> {
        let mut detections = Vec::new();
        let step = (window_size / 4).max(10); // Larger step to reduce overlapping windows
        
        for y in (0..img_height.saturating_sub(window_size)).step_by(step as usize) {
            for x in (0..img_width.saturating_sub(window_size)).step_by(step as usize) {
                let window_rect = IRect::at(x as i32, y as i32)
                    .of_size(window_size, window_size);
                
                // Evaluate cascade classifier
                let confidence = self.evaluate_cascade(integral, &window_rect, window_size);
                
                if confidence > CONFIDENCE_THRESHOLD {
                    detections.push(Detection {
                        x: x as i32,
                        y: y as i32,
                        width: window_size as i32,
                        height: window_size as i32,
                        confidence,
                    });
                }
            }
        }
        
        detections
    }
    
    fn evaluate_cascade(&self, integral: &GrayImage, window: &IRect, window_size: u32) -> f32 {
        let mut total_score = 0.0;
        let mut total_weight = 0.0;
        
        for feature in &self.cascade_features {
            // Scale feature to window size
            let scale = window_size as f32 / 100.0; // Assuming features are defined for 100x100 window
            
            let feature_rect = IRect::at(
                window.left() + (feature.x as f32 * scale) as i32,
                window.top() + (feature.y as f32 * scale) as i32,
            ).of_size(
                (feature.width as f32 * scale) as u32,
                (feature.height as f32 * scale) as u32,
            );
            
            // Calculate feature value
            let feature_value = self.calculate_haar_feature(integral, &feature_rect, &feature.feature_type);
            
            // Apply threshold
            if feature_value > feature.threshold {
                total_score += feature.weight;
            }
            total_weight += feature.weight;
        }
        
        if total_weight > 0.0 {
            total_score / total_weight
        } else {
            0.0
        }
    }
    
    fn calculate_haar_feature(&self, integral: &GrayImage, rect: &IRect, feature_type: &FeatureType) -> f32 {
        let (width, height) = integral.dimensions();
        
        // Ensure rect is within bounds
        if rect.left() < 0 || rect.top() < 0 ||
           rect.right() >= width as i32 || rect.bottom() >= height as i32 {
            return 0.0;
        }
        
        match feature_type {
            FeatureType::TwoRectangleHorizontal => {
                // Calculate difference between top and bottom halves
                let mid_y = rect.top() + (rect.height() / 2) as i32;
                
                let top_rect = IRect::at(rect.left(), rect.top())
                    .of_size(rect.width() as u32, (rect.height() / 2) as u32);
                let bottom_rect = IRect::at(rect.left(), mid_y)
                    .of_size(rect.width() as u32, (rect.height() / 2) as u32);
                
                let top_sum = sum_image_pixels(integral, top_rect.left() as u32, top_rect.top() as u32, 
                    top_rect.right() as u32, top_rect.bottom() as u32)[0] as f32;
                let bottom_sum = sum_image_pixels(integral, bottom_rect.left() as u32, bottom_rect.top() as u32,
                    bottom_rect.right() as u32, bottom_rect.bottom() as u32)[0] as f32;
                
                (top_sum - bottom_sum).abs() / (rect.width() * rect.height()) as f32
            }
            FeatureType::TwoRectangleVertical => {
                // Calculate difference between left and right halves
                let mid_x = rect.left() + (rect.width() / 2) as i32;
                
                let left_rect = IRect::at(rect.left(), rect.top())
                    .of_size((rect.width() / 2) as u32, rect.height() as u32);
                let right_rect = IRect::at(mid_x, rect.top())
                    .of_size((rect.width() / 2) as u32, rect.height() as u32);
                
                let left_sum = sum_image_pixels(integral, left_rect.left() as u32, left_rect.top() as u32,
                    left_rect.right() as u32, left_rect.bottom() as u32)[0] as f32;
                let right_sum = sum_image_pixels(integral, right_rect.left() as u32, right_rect.top() as u32,
                    right_rect.right() as u32, right_rect.bottom() as u32)[0] as f32;
                
                (left_sum - right_sum).abs() / (rect.width() * rect.height()) as f32
            }
            FeatureType::ThreeRectangleHorizontal => {
                // Three horizontal strips - middle should be different
                let h3 = rect.height() / 3;
                
                let top_rect = IRect::at(rect.left(), rect.top()).of_size(rect.width() as u32, h3 as u32);
                let mid_rect = IRect::at(rect.left(), rect.top() + h3 as i32).of_size(rect.width() as u32, h3 as u32);
                let bot_rect = IRect::at(rect.left(), rect.top() + 2 * h3 as i32).of_size(rect.width() as u32, h3 as u32);
                
                let top = sum_image_pixels(integral, top_rect.left() as u32, top_rect.top() as u32,
                    top_rect.right() as u32, top_rect.bottom() as u32)[0] as f32;
                let middle = sum_image_pixels(integral, mid_rect.left() as u32, mid_rect.top() as u32,
                    mid_rect.right() as u32, mid_rect.bottom() as u32)[0] as f32;
                let bottom = sum_image_pixels(integral, bot_rect.left() as u32, bot_rect.top() as u32,
                    bot_rect.right() as u32, bot_rect.bottom() as u32)[0] as f32;
                
                ((top + bottom) - 2.0 * middle).abs() / (rect.width() * rect.height()) as f32
            }
            _ => 0.0, // Simplified - implement other feature types as needed
        }
    }
    
    fn non_maximum_suppression(&self, mut detections: Vec<Detection>) -> Vec<Detection> {
        if detections.is_empty() {
            return detections;
        }
        
        // Sort by confidence
        detections.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        
        let mut keep = Vec::new();
        let iou_threshold = NMS_IOU_THRESHOLD;
        
        while !detections.is_empty() {
            let current = detections.remove(0);
            keep.push(current.clone());
            
            // Remove overlapping detections
            detections.retain(|det| {
                let iou = self.calculate_iou(&current, det);
                iou < iou_threshold
            });
        }
        
        keep
    }
    
    fn calculate_iou(&self, a: &Detection, b: &Detection) -> f32 {
        let x1 = a.x.max(b.x);
        let y1 = a.y.max(b.y);
        let x2 = (a.x + a.width).min(b.x + b.width);
        let y2 = (a.y + a.height).min(b.y + b.height);
        
        if x2 < x1 || y2 < y1 {
            return 0.0;
        }
        
        let intersection = (x2 - x1) * (y2 - y1);
        let area_a = a.width * a.height;
        let area_b = b.width * b.height;
        let union = area_a + area_b - intersection;
        
        intersection as f32 / union as f32
    }
    
    fn extract_embedding(&self, gray: &GrayImage, detection: &Detection) -> String {
        // Extract face region and create embedding
        
        // Calculate local binary pattern or histogram as embedding
        let mut embedding = vec![
            detection.x as f32 / gray.width() as f32,
            detection.y as f32 / gray.height() as f32,
            detection.width as f32 / gray.width() as f32,
            detection.height as f32 / gray.height() as f32,
            detection.confidence,
        ];
        
        // Add histogram features
        let mut histogram = vec![0u32; 16]; // 16-bin histogram
        for y in detection.y..(detection.y + detection.height).min(gray.height() as i32) {
            for x in detection.x..(detection.x + detection.width).min(gray.width() as i32) {
                if x >= 0 && y >= 0 {
                    let pixel = gray.get_pixel(x as u32, y as u32)[0];
                    let bin = (pixel / 16).min(15) as usize;
                    histogram[bin] += 1;
                }
            }
        }
        
        // Normalize and add to embedding
        let total = histogram.iter().sum::<u32>() as f32;
        if total > 0.0 {
            for h in histogram {
                embedding.push(h as f32 / total);
            }
        }
        
        base64_encode(&embedding)
    }
}

#[derive(Clone, Debug)]
struct Detection {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    confidence: f32,
}

// Helper function for base64 encoding
use base64::Engine;

fn base64_encode(data: &[f32]) -> String {
    let bytes: Vec<u8> = data.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}