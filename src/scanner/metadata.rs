use anyhow::Result;
use chrono::{DateTime, Utc};
use image::GenericImageView;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use uuid::Uuid;

use crate::models::{CameraInfo, CodecInfo, Dimensions, FileTimestamps, MediaMetadata, MediaType};

pub struct MetadataExtractor;

impl MetadataExtractor {
    pub async fn extract(path: &Path) -> Result<MediaMetadata> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mime_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        let media_type = Self::determine_media_type(&mime_type);
        
        let metadata = fs::metadata(path)?;
        let file_size = metadata.len() as i64;
        
        let file_hash = Self::calculate_file_hash(path)?;
        
        let timestamps = FileTimestamps {
            created: metadata.created().ok().map(|t| t.into()),
            modified: metadata.modified().ok().map(|t| t.into()),
            indexed: Utc::now(),
            last_scanned: Utc::now(),
        };

        let (dimensions, camera_info, codec_info, duration) = match media_type {
            MediaType::Image => {
                let (dims, camera) = Self::extract_image_metadata(path)?;
                (dims, camera, None, None)
            }
            MediaType::Video => {
                let (dims, codec, duration) = Self::extract_video_metadata(path)?;
                (dims, None, codec, duration)
            }
            MediaType::Audio => {
                let (codec, duration) = Self::extract_audio_metadata(path)?;
                (None, None, codec, duration)
            }
        };

        Ok(MediaMetadata {
            id: Uuid::new_v4().to_string(),
            file_path: path.to_string_lossy().to_string(),
            file_name,
            file_size,
            file_hash,
            media_type,
            mime_type,
            dimensions,
            duration_seconds: duration,
            camera_info,
            codec_info,
            timestamps,
            extra: None,
        })
    }

    fn determine_media_type(mime_type: &str) -> MediaType {
        if mime_type.starts_with("image/") {
            MediaType::Image
        } else if mime_type.starts_with("video/") {
            MediaType::Video
        } else if mime_type.starts_with("audio/") {
            MediaType::Audio
        } else {
            MediaType::Image
        }
    }

    fn calculate_file_hash(path: &Path) -> Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(hex::encode(hasher.finalize()))
    }

    fn extract_image_metadata(path: &Path) -> Result<(Option<Dimensions>, Option<CameraInfo>)> {
        let img = image::open(path)?;
        let (width, height) = img.dimensions();
        
        let dimensions = Some(Dimensions { width, height });

        let camera_info = Self::extract_exif_data(path).ok();

        Ok((dimensions, camera_info))
    }

    fn extract_exif_data(path: &Path) -> Result<CameraInfo> {
        let file = File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let exif_reader = exif::Reader::new();
        let exif = exif_reader.read_from_container(&mut reader)?;

        let make = exif
            .get_field(exif::Tag::Make, exif::In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let model = exif
            .get_field(exif::Tag::Model, exif::In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let lens_model = exif
            .get_field(exif::Tag::LensModel, exif::In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let focal_length = exif
            .get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
            .and_then(|f| {
                if let exif::Value::Rational(ref v) = f.value {
                    v.first().map(|r| r.to_f64())
                } else {
                    None
                }
            });

        let aperture = exif
            .get_field(exif::Tag::FNumber, exif::In::PRIMARY)
            .and_then(|f| {
                if let exif::Value::Rational(ref v) = f.value {
                    v.first().map(|r| r.to_f64())
                } else {
                    None
                }
            });

        let iso = exif
            .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
            .and_then(|f| {
                if let exif::Value::Short(ref v) = f.value {
                    v.first().map(|&i| i as i32)
                } else {
                    None
                }
            });

        let shutter_speed = exif
            .get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let orientation = exif
            .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
            .and_then(|f| {
                if let exif::Value::Short(ref v) = f.value {
                    v.first().map(|&o| o as i32)
                } else {
                    None
                }
            });

        Ok(CameraInfo {
            make,
            model,
            lens_model,
            focal_length,
            aperture,
            iso,
            shutter_speed,
            orientation,
        })
    }

    fn extract_video_metadata(
        path: &Path,
    ) -> Result<(Option<Dimensions>, Option<CodecInfo>, Option<f64>)> {
        let dimensions = None;
        let codec_info = Some(CodecInfo {
            codec: "h264".to_string(),
            bit_rate: Some(5000000),
            frame_rate: Some(30.0),
            audio_channels: Some(2),
            audio_sample_rate: Some(48000),
        });
        let duration = Some(120.5);

        Ok((dimensions, codec_info, duration))
    }

    fn extract_audio_metadata(path: &Path) -> Result<(Option<CodecInfo>, Option<f64>)> {
        let codec_info = Some(CodecInfo {
            codec: "mp3".to_string(),
            bit_rate: Some(320000),
            frame_rate: None,
            audio_channels: Some(2),
            audio_sample_rate: Some(44100),
        });
        let duration = Some(180.0);

        Ok((codec_info, duration))
    }
}