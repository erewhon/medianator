use anyhow::{Result, Context};
use std::path::Path;
use tracing::{debug, info};
use image::DynamicImage;

use crate::models::Face;

const CONFIDENCE_THRESHOLD: f64 = 0.7;
const SIMILARITY_THRESHOLD: f32 = 0.6;
const MIN_FACE_SIZE: u32 = 40; // Increased for better accuracy

// Face similarity calculation functions
pub fn calculate_face_similarity(embedding1: &[f32], embedding2: &[f32]) -> f32 {
    // Cosine similarity calculation
    if embedding1.len() != embedding2.len() {
        return 0.0;
    }
    
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

pub fn should_group_faces(embedding1: &[f32], embedding2: &[f32]) -> bool {
    calculate_face_similarity(embedding1, embedding2) >= SIMILARITY_THRESHOLD
}

// Alternative implementation using simple face detection with image processing
pub struct SimpleFaceDetector {
    enabled: bool,
}

impl SimpleFaceDetector {
    pub fn new() -> Result<Self> {
        info!("Using simple face detector based on image processing");
        Ok(Self { enabled: true })
    }

    pub async fn detect_faces(&self, image_path: &Path, media_id: &str) -> Result<Vec<Face>> {
        if !self.enabled {
            debug!("Face detection is disabled");
            return Ok(Vec::new());
        }

        info!("Detecting faces in: {}", image_path.display());

        // Load the image
        let img = image::open(image_path)
            .context("Failed to open image for face detection")?;
        
        // For simplicity, we'll use a basic skin tone detection approach
        // This is not as accurate as ML-based detection but works without models
        let faces = self.detect_skin_regions(&img)?;
        
        info!("Found {} potential face regions in {}", faces.len(), image_path.display());
        
        // Convert detected regions to Face objects
        let mut result = Vec::new();
        for (i, region) in faces.iter().enumerate() {
            let face = Face {
                id: format!("{}_{}", media_id, i),
                media_file_id: media_id.to_string(),
                face_embedding: self.generate_region_embedding(region),
                face_bbox: format!("{},{},{},{}", 
                    region.0, region.1, region.2, region.3),
                confidence: 0.5, // Lower confidence for simple detection
                detected_at: chrono::Utc::now(),
            };
            result.push(face);
        }
        
        if !result.is_empty() {
            info!("Detected {} face regions in {}", result.len(), image_path.display());
        } else {
            debug!("No faces detected in {}", image_path.display());
        }
        
        Ok(result)
    }

    fn detect_skin_regions(&self, img: &DynamicImage) -> Result<Vec<(u32, u32, u32, u32)>> {
        // Simple skin tone detection
        // Returns bounding boxes (x, y, width, height) of potential face regions
        
        let rgb = img.to_rgb8();
        let (width, height) = rgb.dimensions();
        
        debug!("Analyzing image: {}x{}", width, height);
        
        // Create a binary mask for skin pixels
        let mut skin_mask = vec![vec![false; width as usize]; height as usize];
        let mut skin_pixel_count = 0;
        
        for y in 0..height {
            for x in 0..width {
                let pixel = rgb.get_pixel(x, y);
                if self.is_skin_color(pixel[0], pixel[1], pixel[2]) {
                    skin_mask[y as usize][x as usize] = true;
                    skin_pixel_count += 1;
                }
            }
        }
        
        debug!("Found {} skin-colored pixels", skin_pixel_count);
        
        // Find connected components and extract bounding boxes
        let regions = self.find_connected_regions(&skin_mask, width, height);
        
        debug!("Found {} connected regions", regions.len());
        
        // Filter regions by size and aspect ratio (potential faces)
        let face_regions: Vec<_> = regions.into_iter()
            .filter(|r| {
                let w = r.2;
                let h = r.3;
                let aspect_ratio = w as f32 / h as f32;
                
                // Face-like dimensions
                let size_ok = w >= MIN_FACE_SIZE && h >= MIN_FACE_SIZE;
                let not_too_large = w <= width / 2 && h <= height / 2;
                let aspect_ok = aspect_ratio >= 0.5 && aspect_ratio <= 2.0; // Faces are roughly square
                
                if !size_ok {
                    debug!("Region rejected: too small ({}x{})", w, h);
                } else if !not_too_large {
                    debug!("Region rejected: too large ({}x{})", w, h);
                } else if !aspect_ok {
                    debug!("Region rejected: bad aspect ratio ({})", aspect_ratio);
                }
                
                size_ok && not_too_large && aspect_ok
            })
            .collect();
        
        debug!("Filtered to {} face-like regions", face_regions.len());
        
        Ok(face_regions)
    }

    fn is_skin_color(&self, r: u8, g: u8, b: u8) -> bool {
        // Improved skin detection using multiple color space rules
        // More inclusive for different skin tones
        let r = r as f32;
        let g = g as f32;
        let b = b as f32;
        
        // Convert to normalized RGB
        let sum = r + g + b;
        if sum == 0.0 {
            return false;
        }
        
        let nr = r / sum;
        let ng = g / sum;
        
        // Multiple skin tone detection rules for better coverage
        // Rule 1: Basic RGB range for lighter skin tones
        let rule1 = r > 95.0 && g > 40.0 && b > 20.0 &&
                   r > g && r > b &&
                   (r - g).abs() > 15.0 &&
                   r - b > 15.0;
        
        // Rule 2: Normalized RGB for medium skin tones
        let rule2 = nr > 0.36 && ng > 0.28 && ng < 0.365;
        
        // Rule 3: YCbCr color space check (more reliable)
        let y = 0.299 * r + 0.587 * g + 0.114 * b;
        let cb = -0.168736 * r - 0.331264 * g + 0.5 * b + 128.0;
        let cr = 0.5 * r - 0.418688 * g - 0.081312 * b + 128.0;
        
        let rule3 = y > 80.0 && 
                   cb > 77.0 && cb < 127.0 &&
                   cr > 133.0 && cr < 173.0;
        
        // Rule 4: HSV check for darker skin tones
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        
        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        
        let s = if max == 0.0 { 0.0 } else { delta / max };
        let v = max / 255.0;
        
        let rule4 = (h >= 0.0 && h <= 50.0) && 
                   (s >= 0.23 && s <= 0.68) &&
                   (v >= 0.35);
        
        // Return true if any rule matches
        rule1 || rule2 || rule3 || rule4
    }

    fn find_connected_regions(&self, mask: &[Vec<bool>], width: u32, height: u32) -> Vec<(u32, u32, u32, u32)> {
        // Simple connected component analysis
        let mut regions = Vec::new();
        let mut visited = vec![vec![false; width as usize]; height as usize];
        
        for y in 0..height as usize {
            for x in 0..width as usize {
                if mask[y][x] && !visited[y][x] {
                    // Start a new region
                    let mut min_x = x;
                    let mut max_x = x;
                    let mut min_y = y;
                    let mut max_y = y;
                    
                    // Simple flood fill to find the region bounds
                    let mut stack = vec![(x, y)];
                    while let Some((cx, cy)) = stack.pop() {
                        if cx >= width as usize || cy >= height as usize || visited[cy][cx] || !mask[cy][cx] {
                            continue;
                        }
                        
                        visited[cy][cx] = true;
                        min_x = min_x.min(cx);
                        max_x = max_x.max(cx);
                        min_y = min_y.min(cy);
                        max_y = max_y.max(cy);
                        
                        // Add neighbors
                        if cx > 0 { stack.push((cx - 1, cy)); }
                        if cx < width as usize - 1 { stack.push((cx + 1, cy)); }
                        if cy > 0 { stack.push((cx, cy - 1)); }
                        if cy < height as usize - 1 { stack.push((cx, cy + 1)); }
                    }
                    
                    let region_width = (max_x - min_x + 1) as u32;
                    let region_height = (max_y - min_y + 1) as u32;
                    regions.push((min_x as u32, min_y as u32, region_width, region_height));
                }
            }
        }
        
        regions
    }

    fn generate_region_embedding(&self, region: &(u32, u32, u32, u32)) -> String {
        // Generate a simple embedding for the region
        let embedding = vec![
            region.0 as f32 / 1000.0,
            region.1 as f32 / 1000.0,
            region.2 as f32 / 1000.0,
            region.3 as f32 / 1000.0,
            0.5, // Fixed confidence for simple detection
        ];
        base64_encode(&embedding)
    }
}

// Helper functions for embedding encoding/decoding

use base64::Engine;

pub fn base64_encode(data: &[f32]) -> String {
    let bytes: Vec<u8> = data.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

pub fn base64_decode(encoded: &str) -> Result<Vec<f32>> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    let floats: Vec<f32> = bytes.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    Ok(floats)
}