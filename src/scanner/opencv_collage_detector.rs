// OpenCV-based collage and polaroid detection
// Detects individual photos within photo collages, scrapbook pages, and polaroid collections

use anyhow::{Result, Context, bail};
use std::path::Path;
use tracing::info;

#[cfg(feature = "opencv-face")]
use opencv::{core, imgcodecs, imgproc, prelude::*};

pub struct OpenCVCollageDetector {
    enabled: bool,
}

#[derive(Debug, Clone)]
pub struct DetectedPhoto {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub angle: f64,
    pub confidence: f32,
    pub photo_type: PhotoType,
}

#[derive(Debug, Clone)]
pub enum PhotoType {
    Polaroid,
    Regular,
    Partial,
    Unknown,
}

impl OpenCVCollageDetector {
    pub fn new() -> Result<Self> {
        #[cfg(feature = "opencv-face")]
        {
            info!("Initializing OpenCV collage detector");
            Ok(Self { enabled: true })
        }
        
        #[cfg(not(feature = "opencv-face"))]
        {
            bail!("OpenCV collage detector requires opencv-face feature")
        }
    }
    
    pub async fn detect_photos(&self, image_path: &Path) -> Result<Vec<DetectedPhoto>> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        
        let image_path = image_path.to_path_buf();
        
        // Run OpenCV operations in blocking thread
        let photos = tokio::task::spawn_blocking(move || -> Result<Vec<DetectedPhoto>> {
            Self::detect_photos_sync(&image_path)
        })
        .await
        .context("Failed to spawn blocking task")??;
        
        Ok(photos)
    }
    
    #[cfg(feature = "opencv-face")]
    fn detect_photos_sync(image_path: &Path) -> Result<Vec<DetectedPhoto>> {
        info!("Detecting photos in collage: {}", image_path.display());
        
        // Load image
        let img = imgcodecs::imread(
            &image_path.to_string_lossy(),
            imgcodecs::IMREAD_COLOR,
        ).context("Failed to load image")?;
        
        if img.empty() {
            bail!("Failed to load image: empty");
        }
        
        let mut detected_photos = Vec::new();
        
        // Method 1: Edge-based detection for polaroids and regular photos
        let edge_photos = Self::detect_by_edges(&img)?;
        detected_photos.extend(edge_photos);
        
        // Method 2: Color-based detection for white-bordered polaroids
        let polaroid_photos = Self::detect_polaroids(&img)?;
        detected_photos.extend(polaroid_photos);
        
        // Method 3: Template matching for standard photo sizes
        let template_photos = Self::detect_by_templates(&img)?;
        detected_photos.extend(template_photos);
        
        // Remove duplicates and overlapping detections
        let filtered_photos = Self::filter_overlapping(detected_photos);
        
        info!("Detected {} photos in collage", filtered_photos.len());
        Ok(filtered_photos)
    }
    
    #[cfg(feature = "opencv-face")]
    fn detect_by_edges(img: &Mat) -> Result<Vec<DetectedPhoto>> {
        let mut photos = Vec::new();
        
        // Convert to grayscale
        let mut gray = Mat::default();
        imgproc::cvt_color(img, &mut gray, imgproc::COLOR_BGR2GRAY, 0, core::AlgorithmHint::ALGO_HINT_DEFAULT)?;
        
        // Apply bilateral filter to reduce noise while keeping edges sharp
        let mut filtered = Mat::default();
        imgproc::bilateral_filter(&gray, &mut filtered, 9, 75.0, 75.0, core::BORDER_DEFAULT)?;
        
        // Detect edges using Canny
        let mut edges = Mat::default();
        imgproc::canny(&filtered, &mut edges, 50.0, 150.0, 3, false)?;
        
        // Dilate to connect broken edges
        let kernel = imgproc::get_structuring_element(
            imgproc::MORPH_RECT,
            core::Size::new(3, 3),
            core::Point::new(-1, -1),
        )?;
        let mut dilated = Mat::default();
        imgproc::dilate(&edges, &mut dilated, &kernel, core::Point::new(-1, -1), 1, core::BORDER_CONSTANT, core::Scalar::all(0.0))?;
        
        // Find contours
        let mut contours = core::Vector::<core::Vector<core::Point>>::new();
        imgproc::find_contours(
            &dilated,
            &mut contours,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_SIMPLE,
            core::Point::new(0, 0),
        )?;
        
        // Process each contour
        for contour in contours.iter() {
            let area = imgproc::contour_area(&contour, false)?;
            
            // Filter by area (adjust based on image size)
            let img_area = (img.rows() * img.cols()) as f64;
            if area < img_area * 0.005 || area > img_area * 0.5 {
                continue;
            }
            
            // Approximate contour to polygon
            let mut approx = core::Vector::<core::Point>::new();
            let epsilon = 0.02 * imgproc::arc_length(&contour, true)?;
            imgproc::approx_poly_dp(&contour, &mut approx, epsilon, true)?;
            
            // Check if it's roughly rectangular (4-6 vertices for some tolerance)
            if approx.len() >= 4 && approx.len() <= 8 {
                // Get bounding rectangle
                let rect = imgproc::bounding_rect(&contour)?;
                
                // Check aspect ratio (photos typically have certain ratios)
                let aspect_ratio = rect.width as f32 / rect.height as f32;
                if aspect_ratio > 0.5 && aspect_ratio < 2.0 {
                    // Get minimum area rectangle for rotation
                    let min_rect = imgproc::min_area_rect(&contour)?;
                    
                    photos.push(DetectedPhoto {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height,
                        angle: min_rect.angle as f64,
                        confidence: Self::calculate_rectangularity(&contour, &rect)?,
                        photo_type: Self::classify_photo_type(rect.width, rect.height, aspect_ratio),
                    });
                }
            }
        }
        
        Ok(photos)
    }
    
    #[cfg(feature = "opencv-face")]
    fn detect_polaroids(img: &Mat) -> Result<Vec<DetectedPhoto>> {
        let mut photos = Vec::new();
        
        // Convert to HSV for better white detection
        let mut hsv = Mat::default();
        imgproc::cvt_color(img, &mut hsv, imgproc::COLOR_BGR2HSV, 0, core::AlgorithmHint::ALGO_HINT_DEFAULT)?;
        
        // Threshold for white borders (typical of polaroids)
        let mut white_mask = Mat::default();
        let lower = Mat::from_slice(&[0u8, 0, 200])?;  // Low saturation, high value
        let upper = Mat::from_slice(&[180u8, 30, 255])?;
        core::in_range(&hsv, &lower, &upper, &mut white_mask)?;
        
        // Morphological operations to clean up
        let kernel = imgproc::get_structuring_element(
            imgproc::MORPH_RECT,
            core::Size::new(5, 5),
            core::Point::new(-1, -1),
        )?;
        
        let mut cleaned = Mat::default();
        imgproc::morphology_ex(&white_mask, &mut cleaned, imgproc::MORPH_CLOSE, &kernel, core::Point::new(-1, -1), 1, core::BORDER_CONSTANT, core::Scalar::all(0.0))?;
        imgproc::morphology_ex(&cleaned, &mut white_mask, imgproc::MORPH_OPEN, &kernel, core::Point::new(-1, -1), 1, core::BORDER_CONSTANT, core::Scalar::all(0.0))?;
        
        // Find contours in white regions
        let mut contours = core::Vector::<core::Vector<core::Point>>::new();
        imgproc::find_contours(
            &white_mask,
            &mut contours,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_SIMPLE,
            core::Point::new(0, 0),
        )?;
        
        for contour in contours.iter() {
            let area = imgproc::contour_area(&contour, false)?;
            let img_area = (img.rows() * img.cols()) as f64;
            
            // Polaroids are typically smaller portions of the image
            if area < img_area * 0.01 || area > img_area * 0.3 {
                continue;
            }
            
            let rect = imgproc::bounding_rect(&contour)?;
            let aspect_ratio = rect.width as f32 / rect.height as f32;
            
            // Polaroid aspect ratio is roughly 0.8-1.0 (slightly taller than wide)
            if aspect_ratio > 0.7 && aspect_ratio < 1.1 {
                // Check for the characteristic bottom margin of polaroids
                if Self::has_polaroid_margin(&img, &rect)? {
                    let min_rect = imgproc::min_area_rect(&contour)?;
                    
                    photos.push(DetectedPhoto {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height,
                        angle: min_rect.angle as f64,
                        confidence: 0.9,
                        photo_type: PhotoType::Polaroid,
                    });
                }
            }
        }
        
        Ok(photos)
    }
    
    #[cfg(feature = "opencv-face")]
    fn detect_by_templates(img: &Mat) -> Result<Vec<DetectedPhoto>> {
        let mut photos = Vec::new();
        
        // Convert to grayscale for template matching
        let mut gray = Mat::default();
        imgproc::cvt_color(img, &mut gray, imgproc::COLOR_BGR2GRAY, 0, core::AlgorithmHint::ALGO_HINT_DEFAULT)?;
        
        // Apply adaptive threshold to find rectangular regions
        let mut binary = Mat::default();
        imgproc::adaptive_threshold(
            &gray,
            &mut binary,
            255.0,
            imgproc::ADAPTIVE_THRESH_MEAN_C,
            imgproc::THRESH_BINARY,
            11,
            2.0,
        )?;
        
        // Find connected components
        let mut labels = Mat::default();
        let mut stats = Mat::default();
        let mut centroids = Mat::default();
        let num_components = imgproc::connected_components_with_stats(
            &binary,
            &mut labels,
            &mut stats,
            &mut centroids,
            8,
            core::CV_32S,
        )?;
        
        // Analyze each component
        for i in 1..num_components {  // Skip background (0)
            let area = *stats.at_2d::<i32>(i, imgproc::CC_STAT_AREA)?;
            let x = *stats.at_2d::<i32>(i, imgproc::CC_STAT_LEFT)?;
            let y = *stats.at_2d::<i32>(i, imgproc::CC_STAT_TOP)?;
            let width = *stats.at_2d::<i32>(i, imgproc::CC_STAT_WIDTH)?;
            let height = *stats.at_2d::<i32>(i, imgproc::CC_STAT_HEIGHT)?;
            
            let img_area = (img.rows() * img.cols()) as i32;
            if area < img_area / 200 || area > img_area / 3 {
                continue;
            }
            
            let aspect_ratio = width as f32 / height as f32;
            if aspect_ratio > 0.5 && aspect_ratio < 2.0 {
                photos.push(DetectedPhoto {
                    x,
                    y,
                    width,
                    height,
                    angle: 0.0,
                    confidence: 0.7,
                    photo_type: PhotoType::Regular,
                });
            }
        }
        
        Ok(photos)
    }
    
    #[cfg(feature = "opencv-face")]
    fn calculate_rectangularity(contour: &core::Vector<core::Point>, rect: &core::Rect) -> Result<f32> {
        let contour_area = imgproc::contour_area(contour, false)?;
        let rect_area = (rect.width * rect.height) as f64;
        Ok((contour_area / rect_area) as f32)
    }
    
    #[cfg(feature = "opencv-face")]
    fn has_polaroid_margin(img: &Mat, rect: &core::Rect) -> Result<bool> {
        // Check if the bottom 20% of the rectangle is mostly white (polaroid margin)
        let margin_height = rect.height / 5;
        let margin_y = rect.y + rect.height - margin_height;
        
        if margin_y + margin_height > img.rows() {
            return Ok(false);
        }
        
        let roi = Mat::roi(img, core::Rect::new(
            rect.x,
            margin_y,
            rect.width,
            margin_height,
        ))?;
        
        // Convert ROI to grayscale
        let mut gray_roi = Mat::default();
        imgproc::cvt_color(&roi, &mut gray_roi, imgproc::COLOR_BGR2GRAY, 0, core::AlgorithmHint::ALGO_HINT_DEFAULT)?;
        
        // Calculate mean brightness
        let mean = core::mean(&gray_roi, &core::no_array())?;
        
        // If mean brightness > 200, likely a white margin
        Ok(mean[0] > 200.0)
    }
    
    #[cfg(feature = "opencv-face")]
    fn classify_photo_type(width: i32, _height: i32, aspect_ratio: f32) -> PhotoType {
        if aspect_ratio > 0.7 && aspect_ratio < 1.1 && width < 500 {
            PhotoType::Polaroid
        } else if aspect_ratio > 1.3 && aspect_ratio < 1.5 {
            PhotoType::Regular  // Standard 4:3 or 3:2 photo
        } else {
            PhotoType::Unknown
        }
    }
    
    #[cfg(feature = "opencv-face")]
    fn filter_overlapping(mut photos: Vec<DetectedPhoto>) -> Vec<DetectedPhoto> {
        // Sort by confidence
        photos.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        
        let mut filtered = Vec::new();
        
        for photo in photos {
            let overlaps = filtered.iter().any(|existing: &DetectedPhoto| {
                Self::calculate_iou(
                    &(photo.x, photo.y, photo.width, photo.height),
                    &(existing.x, existing.y, existing.width, existing.height),
                ) > 0.3  // 30% overlap threshold
            });
            
            if !overlaps {
                filtered.push(photo);
            }
        }
        
        filtered
    }
    
    fn calculate_iou(box1: &(i32, i32, i32, i32), box2: &(i32, i32, i32, i32)) -> f32 {
        let x1 = box1.0.max(box2.0);
        let y1 = box1.1.max(box2.1);
        let x2 = (box1.0 + box1.2).min(box2.0 + box2.2);
        let y2 = (box1.1 + box1.3).min(box2.1 + box2.3);
        
        if x2 < x1 || y2 < y1 {
            return 0.0;
        }
        
        let intersection = (x2 - x1) * (y2 - y1);
        let area1 = box1.2 * box1.3;
        let area2 = box2.2 * box2.3;
        let union = area1 + area2 - intersection;
        
        intersection as f32 / union as f32
    }
    
    #[cfg(not(feature = "opencv-face"))]
    fn detect_photos_sync(_image_path: &Path) -> Result<Vec<DetectedPhoto>> {
        bail!("OpenCV collage detection not available. Build with --features opencv-face")
    }
}

// Integration with existing sub-image extractor
impl DetectedPhoto {
    pub fn to_extraction_region(&self) -> crate::scanner::sub_image_extractor::SubImageRegion {
        crate::scanner::sub_image_extractor::SubImageRegion {
            x: self.x as u32,
            y: self.y as u32,
            width: self.width as u32,
            height: self.height as u32,
            confidence: self.confidence,
        }
    }
}