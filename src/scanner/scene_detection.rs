use anyhow::{Result, Context};
use std::path::Path;
use std::process::Command;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

/// Represents a detected scene in a video
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub scene_number: usize,
    pub start_time: f64,
    pub end_time: f64,
    pub start_frame: usize,
    pub end_frame: usize,
    pub duration: f64,
    pub keyframe_path: Option<String>,
    pub confidence: f32,
}

/// Scene detection using multiple methods
pub struct SceneDetector {
    /// Threshold for detecting scene changes (0.0 to 1.0)
    threshold: f32,
    /// Minimum scene duration in seconds
    min_scene_length: f64,
    /// Extract keyframes for each scene
    extract_keyframes: bool,
    /// Output directory for keyframes
    output_dir: Option<String>,
}

impl SceneDetector {
    pub fn new() -> Self {
        Self {
            threshold: 0.3,
            min_scene_length: 1.0,
            extract_keyframes: true,
            output_dir: None,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn with_min_scene_length(mut self, length: f64) -> Self {
        self.min_scene_length = length.max(0.1);
        self
    }

    pub fn with_keyframe_extraction(mut self, extract: bool, output_dir: Option<String>) -> Self {
        self.extract_keyframes = extract;
        self.output_dir = output_dir;
        self
    }

    /// Detect scenes using FFmpeg's scene detection filter
    pub async fn detect_scenes_ffmpeg(&self, video_path: &Path) -> Result<Vec<Scene>> {
        info!("Detecting scenes in video: {:?}", video_path);
        
        // First, get video information
        let duration = self.get_video_duration(video_path)?;
        let fps = self.get_video_fps(video_path)?;
        
        // Use FFmpeg's scene detection filter
        let output = Command::new("ffmpeg")
            .args(&[
                "-i", video_path.to_str().unwrap(),
                "-filter_complex",
                &format!("select='gt(scene,{})',showinfo", self.threshold),
                "-f", "null",
                "-"
            ])
            .output()
            .context("Failed to run FFmpeg scene detection")?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let scenes = self.parse_ffmpeg_output(&stderr, fps)?;
        
        info!("Detected {} scenes", scenes.len());
        
        // Extract keyframes if requested
        if self.extract_keyframes && !scenes.is_empty() {
            self.extract_keyframes_for_scenes(video_path, &scenes).await?;
        }
        
        Ok(scenes)
    }

    /// Alternative scene detection using PySceneDetect (if available)
    pub async fn detect_scenes_pyscenedetect(&self, video_path: &Path) -> Result<Vec<Scene>> {
        // Check if PySceneDetect is available
        let check = Command::new("scenedetect")
            .arg("--version")
            .output();
        
        if check.is_err() {
            warn!("PySceneDetect not available, falling back to FFmpeg");
            return self.detect_scenes_ffmpeg(video_path).await;
        }
        
        info!("Using PySceneDetect for scene detection");
        
        // Create temp directory for output
        let temp_dir = tempfile::tempdir()?;
        let output_file = temp_dir.path().join("scenes.csv");
        
        // Run PySceneDetect
        let output = Command::new("scenedetect")
            .args(&[
                "--input", video_path.to_str().unwrap(),
                "--output", temp_dir.path().to_str().unwrap(),
                "detect-content",
                "--threshold", &self.threshold.to_string(),
                "--min-scene-len", &format!("{}s", self.min_scene_length),
                "list-scenes",
                "--filename", output_file.to_str().unwrap(),
            ])
            .output()
            .context("Failed to run PySceneDetect")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("PySceneDetect failed: {}", stderr);
            return Err(anyhow::anyhow!("PySceneDetect failed"));
        }
        
        // Parse the CSV output
        let scenes = self.parse_pyscenedetect_output(&output_file)?;
        
        info!("Detected {} scenes using PySceneDetect", scenes.len());
        
        // Extract keyframes if requested
        if self.extract_keyframes && !scenes.is_empty() {
            self.extract_keyframes_for_scenes(video_path, &scenes).await?;
        }
        
        Ok(scenes)
    }

    /// Detect scenes using OpenCV (histogram comparison)
    pub async fn detect_scenes_opencv(&self, video_path: &Path) -> Result<Vec<Scene>> {
        // This would require OpenCV bindings
        // For now, we'll use FFmpeg as the primary method
        warn!("OpenCV scene detection not yet implemented, using FFmpeg");
        self.detect_scenes_ffmpeg(video_path).await
    }

    /// Extract keyframes for detected scenes
    async fn extract_keyframes_for_scenes(&self, video_path: &Path, scenes: &[Scene]) -> Result<()> {
        let output_dir = self.output_dir.as_ref()
            .map(|s| s.as_str())
            .unwrap_or("/tmp/scene_keyframes");
        
        // Create output directory
        std::fs::create_dir_all(output_dir)?;
        
        let video_name = video_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("video");
        
        for (i, scene) in scenes.iter().enumerate() {
            // Extract frame at the middle of the scene
            let middle_time = (scene.start_time + scene.end_time) / 2.0;
            let output_path = format!("{}/{}_scene_{:03}.jpg", output_dir, video_name, i + 1);
            
            let result = Command::new("ffmpeg")
                .args(&[
                    "-ss", &middle_time.to_string(),
                    "-i", video_path.to_str().unwrap(),
                    "-vframes", "1",
                    "-q:v", "2",
                    &output_path,
                    "-y"
                ])
                .output();
            
            match result {
                Ok(output) if output.status.success() => {
                    debug!("Extracted keyframe for scene {} to {}", i + 1, output_path);
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("Failed to extract keyframe for scene {}: {}", i + 1, stderr);
                }
                Err(e) => {
                    warn!("Failed to run FFmpeg for keyframe extraction: {}", e);
                }
            }
        }
        
        Ok(())
    }

    /// Get video duration using FFprobe
    fn get_video_duration(&self, video_path: &Path) -> Result<f64> {
        let output = Command::new("ffprobe")
            .args(&[
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
                video_path.to_str().unwrap()
            ])
            .output()
            .context("Failed to get video duration")?;
        
        let duration_str = String::from_utf8_lossy(&output.stdout);
        duration_str.trim().parse::<f64>()
            .context("Failed to parse video duration")
    }

    /// Get video FPS using FFprobe
    fn get_video_fps(&self, video_path: &Path) -> Result<f64> {
        let output = Command::new("ffprobe")
            .args(&[
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "stream=r_frame_rate",
                "-of", "default=noprint_wrappers=1:nokey=1",
                video_path.to_str().unwrap()
            ])
            .output()
            .context("Failed to get video FPS")?;
        
        let fps_str = String::from_utf8_lossy(&output.stdout);
        let fps_parts: Vec<&str> = fps_str.trim().split('/').collect();
        
        if fps_parts.len() == 2 {
            let num = fps_parts[0].parse::<f64>()?;
            let den = fps_parts[1].parse::<f64>()?;
            Ok(num / den)
        } else {
            fps_str.trim().parse::<f64>()
                .context("Failed to parse video FPS")
        }
    }

    /// Parse FFmpeg scene detection output
    fn parse_ffmpeg_output(&self, output: &str, fps: f64) -> Result<Vec<Scene>> {
        let mut scenes = Vec::new();
        let mut last_time = 0.0;
        let mut scene_number = 0;
        
        for line in output.lines() {
            if line.contains("Parsed_showinfo") && line.contains("pts_time:") {
                // Extract timestamp from showinfo output
                if let Some(time_str) = line.split("pts_time:").nth(1) {
                    if let Some(time_str) = time_str.split_whitespace().next() {
                        if let Ok(time) = time_str.parse::<f64>() {
                            if time - last_time >= self.min_scene_length {
                                scene_number += 1;
                                scenes.push(Scene {
                                    scene_number,
                                    start_time: last_time,
                                    end_time: time,
                                    start_frame: (last_time * fps) as usize,
                                    end_frame: (time * fps) as usize,
                                    duration: time - last_time,
                                    keyframe_path: None,
                                    confidence: self.threshold,
                                });
                                last_time = time;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(scenes)
    }

    /// Parse PySceneDetect CSV output
    fn parse_pyscenedetect_output(&self, csv_path: &Path) -> Result<Vec<Scene>> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};
        
        let file = File::open(csv_path)?;
        let reader = BufReader::new(file);
        let mut scenes = Vec::new();
        let mut lines = reader.lines();
        
        // Skip header
        lines.next();
        
        for line in lines {
            let line = line?;
            let parts: Vec<&str> = line.split(',').collect();
            
            if parts.len() >= 7 {
                let scene_number = parts[0].parse::<usize>().unwrap_or(0);
                let start_frame = parts[1].parse::<usize>().unwrap_or(0);
                let start_time = parts[3].parse::<f64>().unwrap_or(0.0);
                let end_frame = parts[4].parse::<usize>().unwrap_or(0);
                let end_time = parts[6].parse::<f64>().unwrap_or(0.0);
                
                scenes.push(Scene {
                    scene_number,
                    start_time,
                    end_time,
                    start_frame,
                    end_frame,
                    duration: end_time - start_time,
                    keyframe_path: None,
                    confidence: self.threshold,
                });
            }
        }
        
        Ok(scenes)
    }

    /// Detect shot boundaries using color histogram analysis
    pub async fn detect_shot_boundaries(&self, video_path: &Path) -> Result<Vec<Scene>> {
        // This is a simplified version that uses FFmpeg's scene filter
        // A more sophisticated version would analyze color histograms directly
        self.detect_scenes_ffmpeg(video_path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scene_detection() {
        let detector = SceneDetector::new()
            .with_threshold(0.3)
            .with_min_scene_length(1.0);
        
        // Test with a sample video if available
        let test_video = Path::new("test_videos/sample.mp4");
        if test_video.exists() {
            let scenes = detector.detect_scenes_ffmpeg(test_video).await;
            assert!(scenes.is_ok());
        }
    }
}