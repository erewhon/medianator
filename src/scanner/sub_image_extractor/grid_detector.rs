use image::GrayImage;

#[derive(Debug, Clone)]
pub struct Rectangle {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub score: f32,
}

/// Detects rectangular regions in a grid layout (like polaroid photos)
pub struct GridDetector {
    min_region_size: u32,
    edge_threshold: u8,
    min_aspect_ratio: f32,
    max_aspect_ratio: f32,
    min_fill_ratio: f32,  // Minimum ratio of edge pixels in rectangle perimeter
}

impl GridDetector {
    pub fn new() -> Self {
        Self {
            min_region_size: 100,
            edge_threshold: 30,
            min_aspect_ratio: 0.5,
            max_aspect_ratio: 2.0,
            min_fill_ratio: 0.3,  // At least 30% of perimeter should have edges
        }
    }

    pub fn detect_rectangles(&self, edges: &GrayImage) -> Vec<Rectangle> {
        let (_width, _height) = edges.dimensions();
        let mut rectangles = Vec::new();
        
        // Find horizontal and vertical lines
        let h_lines = self.find_horizontal_lines(edges);
        let v_lines = self.find_vertical_lines(edges);
        
        tracing::debug!("Found {} horizontal lines and {} vertical lines", h_lines.len(), v_lines.len());
        
        // Find candidate rectangles from line intersections
        for &y1 in &h_lines {
            for &y2 in &h_lines {
                if y2 <= y1 + self.min_region_size {
                    continue;
                }
                
                for &x1 in &v_lines {
                    for &x2 in &v_lines {
                        if x2 <= x1 + self.min_region_size {
                            continue;
                        }
                        
                        let rect = Rectangle {
                            x: x1,
                            y: y1,
                            width: x2 - x1,
                            height: y2 - y1,
                            score: 0.0,
                        };
                        
                        // Check if this rectangle has good edge support
                        if self.is_valid_rectangle(&rect, edges) {
                            let mut rect_with_score = rect;
                            rect_with_score.score = self.calculate_rectangle_score(&rect_with_score, edges);
                            tracing::debug!("Found valid rectangle at ({}, {}) size {}x{} score {}", 
                                rect_with_score.x, rect_with_score.y, 
                                rect_with_score.width, rect_with_score.height, 
                                rect_with_score.score);
                            rectangles.push(rect_with_score);
                        }
                    }
                }
            }
        }
        
        // Remove overlapping rectangles, keeping the ones with better scores
        self.filter_overlapping_rectangles(rectangles)
    }

    fn find_horizontal_lines(&self, edges: &GrayImage) -> Vec<u32> {
        let (width, height) = edges.dimensions();
        let mut lines = Vec::new();
        let min_line_length = width / 4;  // Line must be at least 1/4 of image width
        
        for y in 0..height {
            let mut edge_count = 0;
            for x in 0..width {
                if edges[(x, y)][0] > self.edge_threshold {
                    edge_count += 1;
                }
            }
            
            if edge_count >= min_line_length {
                // Check if this is a peak (local maximum)
                let is_peak = (y == 0 || self.count_edge_pixels_in_row(edges, y) > self.count_edge_pixels_in_row(edges, y - 1))
                    && (y == height - 1 || self.count_edge_pixels_in_row(edges, y) > self.count_edge_pixels_in_row(edges, y + 1));
                
                if is_peak {
                    lines.push(y);
                }
            }
        }
        
        // Merge nearby lines
        self.merge_nearby_values(lines, 10)
    }

    fn find_vertical_lines(&self, edges: &GrayImage) -> Vec<u32> {
        let (width, height) = edges.dimensions();
        let mut lines = Vec::new();
        let min_line_length = height / 4;  // Line must be at least 1/4 of image height
        
        for x in 0..width {
            let mut edge_count = 0;
            for y in 0..height {
                if edges[(x, y)][0] > self.edge_threshold {
                    edge_count += 1;
                }
            }
            
            if edge_count >= min_line_length {
                // Check if this is a peak (local maximum)
                let is_peak = (x == 0 || self.count_edge_pixels_in_column(edges, x) > self.count_edge_pixels_in_column(edges, x - 1))
                    && (x == width - 1 || self.count_edge_pixels_in_column(edges, x) > self.count_edge_pixels_in_column(edges, x + 1));
                
                if is_peak {
                    lines.push(x);
                }
            }
        }
        
        // Merge nearby lines
        self.merge_nearby_values(lines, 10)
    }

    fn count_edge_pixels_in_row(&self, edges: &GrayImage, y: u32) -> u32 {
        let (width, _) = edges.dimensions();
        let mut count = 0;
        for x in 0..width {
            if edges[(x, y)][0] > self.edge_threshold {
                count += 1;
            }
        }
        count
    }

    fn count_edge_pixels_in_column(&self, edges: &GrayImage, x: u32) -> u32 {
        let (_, height) = edges.dimensions();
        let mut count = 0;
        for y in 0..height {
            if edges[(x, y)][0] > self.edge_threshold {
                count += 1;
            }
        }
        count
    }

    fn merge_nearby_values(&self, mut values: Vec<u32>, threshold: u32) -> Vec<u32> {
        if values.is_empty() {
            return values;
        }
        
        values.sort_unstable();
        let mut merged = vec![values[0]];
        
        for &val in values.iter().skip(1) {
            if val - merged.last().unwrap() > threshold {
                merged.push(val);
            }
        }
        
        merged
    }

    fn is_valid_rectangle(&self, rect: &Rectangle, edges: &GrayImage) -> bool {
        // Check aspect ratio
        let aspect_ratio = rect.width as f32 / rect.height as f32;
        if aspect_ratio < self.min_aspect_ratio || aspect_ratio > self.max_aspect_ratio {
            return false;
        }
        
        // Check if enough of the perimeter has edges
        let perimeter_pixels = self.count_perimeter_edges(rect, edges);
        let total_perimeter = 2 * (rect.width + rect.height);
        let fill_ratio = perimeter_pixels as f32 / total_perimeter as f32;
        
        fill_ratio >= self.min_fill_ratio
    }

    fn count_perimeter_edges(&self, rect: &Rectangle, edges: &GrayImage) -> u32 {
        let (img_width, img_height) = edges.dimensions();
        let mut count = 0;
        
        // Top and bottom edges
        for x in rect.x..rect.x.min(img_width).min(rect.x + rect.width) {
            if rect.y < img_height && edges[(x, rect.y)][0] > self.edge_threshold {
                count += 1;
            }
            let bottom_y = (rect.y + rect.height - 1).min(img_height - 1);
            if edges[(x, bottom_y)][0] > self.edge_threshold {
                count += 1;
            }
        }
        
        // Left and right edges
        for y in rect.y..rect.y.min(img_height).min(rect.y + rect.height) {
            if rect.x < img_width && edges[(rect.x, y)][0] > self.edge_threshold {
                count += 1;
            }
            let right_x = (rect.x + rect.width - 1).min(img_width - 1);
            if edges[(right_x, y)][0] > self.edge_threshold {
                count += 1;
            }
        }
        
        count
    }

    fn calculate_rectangle_score(&self, rect: &Rectangle, edges: &GrayImage) -> f32 {
        // Score based on how well-defined the rectangle edges are
        let perimeter_edges = self.count_perimeter_edges(rect, edges);
        let total_perimeter = 2 * (rect.width + rect.height);
        let edge_score = perimeter_edges as f32 / total_perimeter as f32;
        
        // Bonus for rectangles that are not too small or too large
        let size_score = if rect.width > 150 && rect.height > 150 && rect.width < 1000 && rect.height < 1000 {
            1.0
        } else {
            0.5
        };
        
        edge_score * size_score
    }

    fn filter_overlapping_rectangles(&self, mut rectangles: Vec<Rectangle>) -> Vec<Rectangle> {
        // Sort by score (descending)
        rectangles.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        
        let mut kept = Vec::new();
        
        for rect in rectangles {
            let overlap = self.check_overlap(&rect, &kept);
            if overlap < 0.5 {  // Allow up to 50% overlap
                kept.push(rect);
            }
        }
        
        kept
    }

    fn check_overlap(&self, rect: &Rectangle, existing: &[Rectangle]) -> f32 {
        let mut max_overlap = 0.0f32;
        
        for other in existing {
            let overlap = self.calculate_overlap(rect, other);
            max_overlap = max_overlap.max(overlap);
        }
        
        max_overlap
    }

    fn calculate_overlap(&self, r1: &Rectangle, r2: &Rectangle) -> f32 {
        let x_overlap = (r1.x + r1.width).min(r2.x + r2.width).saturating_sub(r1.x.max(r2.x));
        let y_overlap = (r1.y + r1.height).min(r2.y + r2.height).saturating_sub(r1.y.max(r2.y));
        
        if x_overlap == 0 || y_overlap == 0 {
            return 0.0;
        }
        
        let overlap_area = x_overlap * y_overlap;
        let r1_area = r1.width * r1.height;
        let r2_area = r2.width * r2.height;
        let min_area = r1_area.min(r2_area);
        
        overlap_area as f32 / min_area as f32
    }
}