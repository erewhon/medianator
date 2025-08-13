use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Image,
    Video,
    Audio,
}

impl From<MediaType> for String {
    fn from(media_type: MediaType) -> Self {
        match media_type {
            MediaType::Image => "image".to_string(),
            MediaType::Video => "video".to_string(),
            MediaType::Audio => "audio".to_string(),
        }
    }
}

impl TryFrom<String> for MediaType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "image" => Ok(MediaType::Image),
            "video" => Ok(MediaType::Video),
            "audio" => Ok(MediaType::Audio),
            _ => Err(format!("Invalid media type: {}", value)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MediaFile {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub file_hash: String,
    pub media_type: String,
    pub mime_type: String,
    
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<f64>,
    pub bit_rate: Option<i32>,
    
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub iso: Option<i32>,
    pub shutter_speed: Option<String>,
    pub orientation: Option<i32>,
    
    pub codec: Option<String>,
    pub frame_rate: Option<f64>,
    pub audio_channels: Option<i32>,
    pub audio_sample_rate: Option<i32>,
    
    pub file_created_at: Option<DateTime<Utc>>,
    pub file_modified_at: Option<DateTime<Utc>>,
    pub indexed_at: DateTime<Utc>,
    pub last_scanned_at: DateTime<Utc>,
    
    pub thumbnail_path: Option<String>,
    pub thumbnail_generated_at: Option<DateTime<Utc>>,
    
    pub extra_metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub file_hash: String,
    pub media_type: MediaType,
    pub mime_type: String,
    pub dimensions: Option<Dimensions>,
    pub duration_seconds: Option<f64>,
    pub camera_info: Option<CameraInfo>,
    pub codec_info: Option<CodecInfo>,
    pub timestamps: FileTimestamps,
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraInfo {
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub iso: Option<i32>,
    pub shutter_speed: Option<String>,
    pub orientation: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecInfo {
    pub codec: String,
    pub bit_rate: Option<i32>,
    pub frame_rate: Option<f64>,
    pub audio_channels: Option<i32>,
    pub audio_sample_rate: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTimestamps {
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub indexed: DateTime<Utc>,
    pub last_scanned: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScanHistory {
    pub id: i32,
    pub scan_path: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub files_scanned: i32,
    pub files_added: i32,
    pub files_updated: i32,
    pub files_removed: i32,
    pub error_count: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub scan_id: i32,
    pub current_path: String,
    pub files_processed: usize,
    pub files_pending: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Duplicate {
    pub id: i32,
    pub file_hash: String,
    pub file_paths: String, // JSON array
    pub file_count: i32,
    pub total_size: i64,
    pub first_seen_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub files: Vec<DuplicateFile>,
    pub total_size: i64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateFile {
    pub id: String,
    pub path: String,
    pub size: i64,
    pub modified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Face {
    pub id: String,
    pub media_file_id: String,
    pub face_embedding: String, // Base64 encoded embedding
    pub face_bbox: String, // JSON
    pub confidence: f32,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceBoundingBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FaceGroup {
    pub id: String,
    pub group_name: Option<String>,
    pub face_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceGroupMember {
    pub face_id: String,
    pub group_id: String,
    pub similarity_score: f32,
    pub media_file_path: String,
    pub face_bbox: FaceBoundingBox,
}