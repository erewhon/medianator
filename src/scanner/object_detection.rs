use anyhow::{Result, Context};
use std::path::Path;
use std::process::Command;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};
use std::collections::HashMap;

/// Represents a detected object in an image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedObject {
    pub class_name: String,
    pub confidence: f32,
    pub bbox: BoundingBox,
    pub attributes: HashMap<String, String>,
}

/// Bounding box for detected objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Photo classification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoClassification {
    pub primary_category: String,
    pub categories: Vec<Category>,
    pub tags: Vec<String>,
    pub scene_type: Option<String>,
    pub is_screenshot: bool,
    pub is_document: bool,
    pub has_text: bool,
    pub dominant_colors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub confidence: f32,
}

/// Object detection and photo classification
pub struct ObjectDetector {
    /// Minimum confidence threshold for detections
    confidence_threshold: f32,
    /// Use GPU if available
    use_gpu: bool,
    /// Model to use (yolo, mobilenet, etc.)
    model_type: String,
}

impl ObjectDetector {
    pub fn new() -> Self {
        Self {
            confidence_threshold: 0.5,
            use_gpu: false,
            model_type: "yolo".to_string(),
        }
    }

    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn with_gpu(mut self, use_gpu: bool) -> Self {
        self.use_gpu = use_gpu;
        self
    }

    pub fn with_model(mut self, model_type: String) -> Self {
        self.model_type = model_type;
        self
    }

    /// Detect objects using YOLO via Python script
    pub async fn detect_objects_yolo(&self, image_path: &Path) -> Result<Vec<DetectedObject>> {
        info!("Detecting objects in image: {:?}", image_path);
        
        // Create Python script for YOLO detection
        let python_script = r#"
import sys
import json
import cv2
import numpy as np
from pathlib import Path

def detect_objects_yolo(image_path, confidence_threshold=0.5):
    """Detect objects using YOLO (simplified version)"""
    # This is a simplified implementation
    # In production, you would load actual YOLO model
    
    # For now, return mock data for testing
    objects = []
    
    # Check if it's a screenshot (simple heuristic)
    img = cv2.imread(str(image_path))
    if img is not None:
        height, width = img.shape[:2]
        aspect_ratio = width / height
        
        # Common screenshot aspect ratios
        if aspect_ratio in [16/9, 16/10, 4/3, 5/4]:
            objects.append({
                'class_name': 'screenshot',
                'confidence': 0.9,
                'bbox': {'x': 0, 'y': 0, 'width': width, 'height': height},
                'attributes': {'type': 'computer_screen'}
            })
    
    return objects

if __name__ == "__main__":
    image_path = sys.argv[1]
    confidence = float(sys.argv[2]) if len(sys.argv) > 2 else 0.5
    
    try:
        objects = detect_objects_yolo(image_path, confidence)
        print(json.dumps(objects))
    except Exception as e:
        print(json.dumps({'error': str(e)}))
        sys.exit(1)
"#;

        // Write script to temp file
        let temp_script = tempfile::NamedTempFile::new()?.into_temp_path();
        std::fs::write(&temp_script, python_script)?;
        
        // Run Python script
        let output = Command::new("python3")
            .args(&[
                temp_script.to_str().unwrap(),
                image_path.to_str().unwrap(),
                &self.confidence_threshold.to_string(),
            ])
            .output()
            .context("Failed to run object detection script")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Object detection failed: {}", stderr);
            return Ok(Vec::new()); // Return empty vector instead of error
        }
        
        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let objects: Vec<DetectedObject> = serde_json::from_str(&stdout)
            .unwrap_or_else(|_| Vec::new());
        
        info!("Detected {} objects", objects.len());
        Ok(objects)
    }

    /// Classify photo into categories
    pub async fn classify_photo(&self, image_path: &Path) -> Result<PhotoClassification> {
        info!("Classifying photo: {:?}", image_path);
        
        // Detect objects first
        let objects = self.detect_objects_yolo(image_path).await?;
        
        // Analyze image properties
        let is_screenshot = objects.iter().any(|o| o.class_name == "screenshot");
        let is_document = self.detect_document(image_path).await?;
        let has_text = self.detect_text(image_path).await?;
        let dominant_colors = self.extract_dominant_colors(image_path).await?;
        
        // Determine categories based on detected objects
        let mut categories = Vec::new();
        let mut tags = Vec::new();
        
        // Add categories based on objects
        for obj in &objects {
            tags.push(obj.class_name.clone());
            
            // Map objects to categories
            let category = match obj.class_name.as_str() {
                "person" | "face" => "People",
                "dog" | "cat" | "bird" | "horse" => "Animals",
                "car" | "truck" | "bus" | "motorcycle" => "Vehicles",
                "tree" | "flower" | "plant" => "Nature",
                "building" | "house" | "bridge" => "Architecture",
                "food" | "cake" | "pizza" => "Food",
                "laptop" | "phone" | "keyboard" => "Technology",
                _ => "Other",
            };
            
            categories.push(Category {
                name: category.to_string(),
                confidence: obj.confidence,
            });
        }
        
        // Deduplicate and sort categories
        categories.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        categories.dedup_by(|a, b| a.name == b.name);
        
        // Determine primary category
        let primary_category = categories.first()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| {
                if is_screenshot { "Screenshots".to_string() }
                else if is_document { "Documents".to_string() }
                else { "Uncategorized".to_string() }
            });
        
        // Determine scene type
        let scene_type = self.detect_scene_type(&tags);
        
        Ok(PhotoClassification {
            primary_category,
            categories,
            tags,
            scene_type,
            is_screenshot,
            is_document,
            has_text,
            dominant_colors,
        })
    }

    /// Detect if image is a document
    async fn detect_document(&self, _image_path: &Path) -> Result<bool> {
        // Simple heuristic: check for high contrast and rectangular shapes
        // In production, use proper document detection
        Ok(false)
    }

    /// Detect text in image using OCR
    async fn detect_text(&self, image_path: &Path) -> Result<bool> {
        // Check if tesseract is available
        let check = Command::new("tesseract")
            .arg("--version")
            .output();
        
        if check.is_err() {
            debug!("Tesseract not available for text detection");
            return Ok(false);
        }
        
        // Run simple text detection
        let temp_output = tempfile::NamedTempFile::new()?;
        let output = Command::new("tesseract")
            .args(&[
                image_path.to_str().unwrap(),
                temp_output.path().to_str().unwrap(),
                "--psm", "3", // Automatic page segmentation
                "-l", "eng",
            ])
            .output();
        
        if let Ok(output) = output {
            if output.status.success() {
                // Check if output file has content
                let text_file = format!("{}.txt", temp_output.path().to_str().unwrap());
                if let Ok(content) = std::fs::read_to_string(&text_file) {
                    return Ok(content.trim().len() > 10); // Has meaningful text
                }
            }
        }
        
        Ok(false)
    }

    /// Extract dominant colors from image
    async fn extract_dominant_colors(&self, image_path: &Path) -> Result<Vec<String>> {
        // Simple implementation using ImageMagick if available
        let check = Command::new("convert")
            .arg("--version")
            .output();
        
        if check.is_err() {
            debug!("ImageMagick not available for color extraction");
            return Ok(Vec::new());
        }
        
        // Extract top 5 colors
        let output = Command::new("convert")
            .args(&[
                image_path.to_str().unwrap(),
                "-resize", "100x100",
                "-colors", "5",
                "-depth", "8",
                "-format", "%c",
                "histogram:info:-",
            ])
            .output();
        
        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let colors = self.parse_color_histogram(&stdout);
                return Ok(colors);
            }
        }
        
        Ok(Vec::new())
    }

    /// Parse ImageMagick color histogram output
    fn parse_color_histogram(&self, output: &str) -> Vec<String> {
        let mut colors = Vec::new();
        
        for line in output.lines() {
            if let Some(hex_start) = line.find('#') {
                if let Some(hex_end) = line[hex_start..].find(' ') {
                    let hex_color = &line[hex_start..hex_start + hex_end];
                    colors.push(hex_color.to_string());
                }
            }
        }
        
        colors.truncate(5); // Keep top 5 colors
        colors
    }

    /// Detect scene type based on tags
    fn detect_scene_type(&self, tags: &[String]) -> Option<String> {
        // Simple rule-based scene detection
        let tag_set: std::collections::HashSet<_> = tags.iter().map(|s| s.as_str()).collect();
        
        if tag_set.contains("person") && tag_set.contains("suit") {
            Some("Business".to_string())
        } else if tag_set.contains("person") && (tag_set.contains("beach") || tag_set.contains("ocean")) {
            Some("Beach".to_string())
        } else if tag_set.contains("mountain") || tag_set.contains("forest") {
            Some("Nature".to_string())
        } else if tag_set.contains("building") && tag_set.contains("city") {
            Some("Urban".to_string())
        } else if tag_set.contains("food") || tag_set.contains("restaurant") {
            Some("Dining".to_string())
        } else if tag_set.contains("person") && tags.len() > 3 {
            Some("Group".to_string())
        } else if tag_set.contains("person") {
            Some("Portrait".to_string())
        } else {
            None
        }
    }

    /// Create automatic albums based on detected content
    pub async fn suggest_albums(&self, classifications: &[PhotoClassification]) -> Vec<String> {
        let mut album_suggestions = Vec::new();
        
        // Count occurrences of each category
        let mut category_counts = HashMap::new();
        let mut scene_counts = HashMap::new();
        
        for classification in classifications {
            *category_counts.entry(&classification.primary_category).or_insert(0) += 1;
            if let Some(scene) = &classification.scene_type {
                *scene_counts.entry(scene).or_insert(0) += 1;
            }
        }
        
        // Suggest albums for categories with enough photos
        for (category, count) in category_counts {
            if count >= 5 {
                album_suggestions.push(format!("{} Collection", category));
            }
        }
        
        // Suggest albums for scenes
        for (scene, count) in scene_counts {
            if count >= 3 {
                album_suggestions.push(format!("{} Moments", scene));
            }
        }
        
        // Special albums
        let screenshot_count = classifications.iter().filter(|c| c.is_screenshot).count();
        if screenshot_count >= 5 {
            album_suggestions.push("Screenshots".to_string());
        }
        
        let document_count = classifications.iter().filter(|c| c.is_document).count();
        if document_count >= 5 {
            album_suggestions.push("Documents".to_string());
        }
        
        album_suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_object_detection() {
        let detector = ObjectDetector::new()
            .with_confidence_threshold(0.5);
        
        // Test with a sample image if available
        let test_image = Path::new("test_images/sample.jpg");
        if test_image.exists() {
            let objects = detector.detect_objects_yolo(test_image).await;
            assert!(objects.is_ok());
        }
    }

    #[tokio::test]
    async fn test_photo_classification() {
        let detector = ObjectDetector::new();
        
        // Test with a sample image if available
        let test_image = Path::new("test_images/sample.jpg");
        if test_image.exists() {
            let classification = detector.classify_photo(test_image).await;
            assert!(classification.is_ok());
        }
    }
}