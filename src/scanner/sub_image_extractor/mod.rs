use anyhow::{Context, Result};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use uuid::Uuid;

mod grid_detector;
use grid_detector::GridDetector;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubImageRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionMetadata {
    pub source_region: SubImageRegion,
    pub extraction_method: String,
    pub extraction_timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct SubImageExtractor {
    min_region_size: u32,
    edge_threshold: f32,
    min_aspect_ratio: f32,
    max_aspect_ratio: f32,
    use_opencv_detector: bool,
}

impl SubImageExtractor {
    pub fn new() -> Self {
        Self {
            min_region_size: 100,
            edge_threshold: 30.0,
            min_aspect_ratio: 0.3,
            max_aspect_ratio: 3.0,
            use_opencv_detector: false,
        }
    }
    
    pub fn with_opencv(mut self) -> Self {
        self.use_opencv_detector = true;
        self
    }

    pub async fn extract_sub_images(
        &self,
        image_path: &Path,
        output_dir: &Path,
    ) -> Result<Vec<(PathBuf, ExtractionMetadata)>> {
        // Try OpenCV detector first if available
        let regions = if self.use_opencv_detector {
            info!("Attempting OpenCV collage detection for: {}", image_path.display());
            match self.detect_with_opencv(image_path).await {
                Ok(opencv_regions) if !opencv_regions.is_empty() => {
                    info!("OpenCV detector found {} sub-images in {}", opencv_regions.len(), image_path.display());
                    opencv_regions
                }
                Ok(_) => {
                    info!("OpenCV detector found no sub-images, falling back to standard detection");
                    let img = image::open(image_path)
                        .with_context(|| format!("Failed to open image: {}", image_path.display()))?;
                    self.detect_sub_images(&img)?
                }
                Err(e) => {
                    info!("OpenCV detection failed: {}, falling back to standard detection", e);
                    let img = image::open(image_path)
                        .with_context(|| format!("Failed to open image: {}", image_path.display()))?;
                    self.detect_sub_images(&img)?
                }
            }
        } else {
            debug!("Using standard edge detection for sub-images");
            let img = image::open(image_path)
                .with_context(|| format!("Failed to open image: {}", image_path.display()))?;
            self.detect_sub_images(&img)?
        };
        
        let img = image::open(image_path)
            .with_context(|| format!("Failed to open image: {}", image_path.display()))?;
        
        if regions.is_empty() {
            debug!("No sub-images detected in {}", image_path.display());
            return Ok(Vec::new());
        }

        info!("Detected {} sub-images in {}", regions.len(), image_path.display());
        
        let mut extracted_images = Vec::new();
        let parent_stem = image_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        for (index, region) in regions.iter().enumerate() {
            let sub_image = self.extract_region(&img, region)?;
            
            let sub_image_id = Uuid::new_v4();
            let filename = format!("{}_sub_{}_{}.jpg", parent_stem, index, sub_image_id);
            let output_path = output_dir.join(&filename);
            
            sub_image.save(&output_path)
                .with_context(|| format!("Failed to save sub-image: {}", output_path.display()))?;
            
            let metadata = ExtractionMetadata {
                source_region: region.clone(),
                extraction_method: if self.use_opencv_detector { 
                    "opencv_collage".to_string() 
                } else { 
                    "edge_detection".to_string() 
                },
                extraction_timestamp: chrono::Utc::now(),
            };
            
            extracted_images.push((output_path, metadata));
        }
        
        Ok(extracted_images)
    }

    fn detect_sub_images(&self, img: &DynamicImage) -> Result<Vec<SubImageRegion>> {
        let gray = img.to_luma8();
        let (width, height) = gray.dimensions();
        
        let edges = self.detect_edges(&gray)?;
        
        // Try grid detection first (for polaroid grids, photo albums, etc.)
        let grid_detector = GridDetector::new();
        let grid_rectangles = grid_detector.detect_rectangles(&edges);
        
        if !grid_rectangles.is_empty() {
            info!("Grid detector found {} rectangles", grid_rectangles.len());
            let mut regions = Vec::new();
            for rect in grid_rectangles {
                // Only keep rectangles that are valid regions
                let region = SubImageRegion {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                    confidence: rect.score,
                };
                if self.is_valid_region(&region, width, height) {
                    regions.push(region);
                }
            }
            return Ok(regions);
        }
        
        // Fall back to flood-fill method for other types of composite images
        debug!("Grid detector found no rectangles, trying flood-fill method");
        let mut regions = Vec::new();
        let mut visited = vec![vec![false; width as usize]; height as usize];
        
        for y in 0..height {
            for x in 0..width {
                if edges[(x, y)][0] > self.edge_threshold as u8 && !visited[y as usize][x as usize] {
                    if let Some(region) = self.find_region(&edges, &mut visited, x, y) {
                        if self.is_valid_region(&region, width, height) {
                            regions.push(region);
                        }
                    }
                }
            }
        }
        
        regions = self.merge_overlapping_regions(regions);
        regions = self.filter_nested_regions(regions);
        
        Ok(regions)
    }

    fn detect_edges(&self, gray: &image::GrayImage) -> Result<image::GrayImage> {
        let (width, height) = gray.dimensions();
        let mut edges = image::GrayImage::new(width, height);
        
        let sobel_x = [
            [-1.0, 0.0, 1.0],
            [-2.0, 0.0, 2.0],
            [-1.0, 0.0, 1.0],
        ];
        
        let sobel_y = [
            [-1.0, -2.0, -1.0],
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 1.0],
        ];
        
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let mut gx = 0.0;
                let mut gy = 0.0;
                
                for dy in 0..3 {
                    for dx in 0..3 {
                        let pixel = gray.get_pixel(x + dx - 1, y + dy - 1)[0] as f32;
                        gx += pixel * sobel_x[dy as usize][dx as usize];
                        gy += pixel * sobel_y[dy as usize][dx as usize];
                    }
                }
                
                let magnitude = (gx * gx + gy * gy).sqrt();
                edges.put_pixel(x, y, image::Luma([magnitude.min(255.0) as u8]));
            }
        }
        
        Ok(edges)
    }

    fn find_region(
        &self,
        edges: &image::GrayImage,
        visited: &mut Vec<Vec<bool>>,
        start_x: u32,
        start_y: u32,
    ) -> Option<SubImageRegion> {
        let (width, height) = edges.dimensions();
        let mut min_x = start_x;
        let mut max_x = start_x;
        let mut min_y = start_y;
        let mut max_y = start_y;
        
        let mut stack = vec![(start_x, start_y)];
        visited[start_y as usize][start_x as usize] = true;
        
        while let Some((x, y)) = stack.pop() {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    
                    let nx = (x as i32 + dx) as u32;
                    let ny = (y as i32 + dy) as u32;
                    
                    if nx < width && ny < height && !visited[ny as usize][nx as usize] {
                        if edges[(nx, ny)][0] > self.edge_threshold as u8 {
                            visited[ny as usize][nx as usize] = true;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
        }
        
        let region_width = max_x - min_x + 1;
        let region_height = max_y - min_y + 1;
        
        if region_width >= self.min_region_size && region_height >= self.min_region_size {
            Some(SubImageRegion {
                x: min_x,
                y: min_y,
                width: region_width,
                height: region_height,
                confidence: 0.8,
            })
        } else {
            None
        }
    }

    fn is_valid_region(&self, region: &SubImageRegion, img_width: u32, img_height: u32) -> bool {
        let aspect_ratio = region.width as f32 / region.height as f32;
        
        aspect_ratio >= self.min_aspect_ratio 
            && aspect_ratio <= self.max_aspect_ratio
            && region.width < img_width * 9 / 10
            && region.height < img_height * 9 / 10
    }

    fn merge_overlapping_regions(&self, mut regions: Vec<SubImageRegion>) -> Vec<SubImageRegion> {
        let mut merged = Vec::new();
        regions.sort_by_key(|r| (r.x, r.y));
        
        for region in regions {
            let mut was_merged = false;
            
            for existing in &mut merged {
                if self.regions_overlap(&region, existing) {
                    *existing = self.merge_regions(&region, existing);
                    was_merged = true;
                    break;
                }
            }
            
            if !was_merged {
                merged.push(region);
            }
        }
        
        merged
    }

    fn regions_overlap(&self, r1: &SubImageRegion, r2: &SubImageRegion) -> bool {
        let r1_right = r1.x + r1.width;
        let r1_bottom = r1.y + r1.height;
        let r2_right = r2.x + r2.width;
        let r2_bottom = r2.y + r2.height;
        
        !(r1.x >= r2_right || r2.x >= r1_right || r1.y >= r2_bottom || r2.y >= r1_bottom)
    }

    fn merge_regions(&self, r1: &SubImageRegion, r2: &SubImageRegion) -> SubImageRegion {
        let min_x = r1.x.min(r2.x);
        let min_y = r1.y.min(r2.y);
        let max_x = (r1.x + r1.width).max(r2.x + r2.width);
        let max_y = (r1.y + r1.height).max(r2.y + r2.height);
        
        SubImageRegion {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
            confidence: r1.confidence.max(r2.confidence),
        }
    }

    fn filter_nested_regions(&self, mut regions: Vec<SubImageRegion>) -> Vec<SubImageRegion> {
        regions.sort_by_key(|r| (r.width * r.height));
        
        let mut filtered = Vec::new();
        
        for region in regions.iter().rev() {
            let mut is_nested = false;
            
            for existing in &filtered {
                if self.region_contains(existing, region) {
                    is_nested = true;
                    break;
                }
            }
            
            if !is_nested {
                filtered.push(region.clone());
            }
        }
        
        filtered
    }

    fn region_contains(&self, outer: &SubImageRegion, inner: &SubImageRegion) -> bool {
        outer.x <= inner.x
            && outer.y <= inner.y
            && outer.x + outer.width >= inner.x + inner.width
            && outer.y + outer.height >= inner.y + inner.height
    }

    fn extract_region(&self, img: &DynamicImage, region: &SubImageRegion) -> Result<DynamicImage> {
        let sub_image = img.crop_imm(region.x, region.y, region.width, region.height);
        Ok(sub_image)
    }
    
    #[cfg(feature = "opencv-face")]
    async fn detect_with_opencv(&self, image_path: &Path) -> Result<Vec<SubImageRegion>> {
        use crate::scanner::opencv_collage_detector::OpenCVCollageDetector;
        
        let detector = OpenCVCollageDetector::new()?;
        let photos = detector.detect_photos(image_path).await?;
        
        let regions: Vec<SubImageRegion> = photos
            .into_iter()
            .map(|photo| photo.to_extraction_region())
            .collect();
        
        Ok(regions)
    }
    
    #[cfg(not(feature = "opencv-face"))]
    async fn detect_with_opencv(&self, _image_path: &Path) -> Result<Vec<SubImageRegion>> {
        // OpenCV not available, return empty
        Ok(Vec::new())
    }
}