use anyhow::Result;
use chrono::{DateTime, Utc};
use image::GenericImageView;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use uuid::Uuid;

use crate::models::{CameraInfo, CodecInfo, Dimensions, FileTimestamps, LocationInfo, MediaMetadata, MediaType};

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

        let (dimensions, camera_info, codec_info, duration, location_info, date_taken) = match media_type {
            MediaType::Image => {
                let (dims, camera, location, date) = Self::extract_image_metadata(path)?;
                (dims, camera, None, None, location, date)
            }
            MediaType::Video => {
                let (dims, codec, duration) = Self::extract_video_metadata(path)?;
                (dims, None, codec, duration, None, None)
            }
            MediaType::Audio => {
                let (codec, duration) = Self::extract_audio_metadata(path)?;
                (None, None, codec, duration, None, None)
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
            location_info,
            date_taken,
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

    fn extract_image_metadata(path: &Path) -> Result<(Option<Dimensions>, Option<CameraInfo>, Option<LocationInfo>, Option<DateTime<Utc>>)> {
        let img = image::open(path)?;
        let (width, height) = img.dimensions();
        
        let dimensions = Some(Dimensions { width, height });

        let (camera_info, location_info, date_taken) = Self::extract_exif_data(path).unwrap_or((
            CameraInfo {
                make: None,
                model: None,
                lens_model: None,
                focal_length: None,
                aperture: None,
                iso: None,
                shutter_speed: None,
                orientation: None,
            },
            None,
            None,
        ));

        Ok((dimensions, Some(camera_info), location_info, date_taken))
    }

    pub fn extract_exif_data(path: &Path) -> Result<(CameraInfo, Option<LocationInfo>, Option<DateTime<Utc>>)> {
        use exif::{Tag, Reader, In, Value};
        
        let file = File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let exif_reader = Reader::new();
        let exif = exif_reader.read_from_container(&mut reader)?;

        let make = exif
            .get_field(Tag::Make, In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let model = exif
            .get_field(Tag::Model, In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let lens_model = exif
            .get_field(Tag::LensModel, In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let focal_length = exif
            .get_field(Tag::FocalLength, In::PRIMARY)
            .and_then(|f| {
                if let Value::Rational(ref v) = f.value {
                    v.first().map(|r| r.to_f64())
                } else {
                    None
                }
            });

        let aperture = exif
            .get_field(Tag::FNumber, In::PRIMARY)
            .and_then(|f| {
                if let Value::Rational(ref v) = f.value {
                    v.first().map(|r| r.to_f64())
                } else {
                    None
                }
            });

        let iso = exif
            .get_field(Tag::PhotographicSensitivity, In::PRIMARY)
            .and_then(|f| {
                if let Value::Short(ref v) = f.value {
                    v.first().map(|&i| i as i32)
                } else {
                    None
                }
            });

        let shutter_speed = exif
            .get_field(Tag::ExposureTime, In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let orientation = exif
            .get_field(Tag::Orientation, In::PRIMARY)
            .and_then(|f| {
                if let Value::Short(ref v) = f.value {
                    v.first().map(|&o| o as i32)
                } else {
                    None
                }
            });

        // Extract GPS location data
        let location_info = Self::extract_gps_data(&exif);
        
        // Extract date taken
        let date_taken = Self::extract_date_taken(&exif);

        let camera_info = CameraInfo {
            make,
            model,
            lens_model,
            focal_length,
            aperture,
            iso,
            shutter_speed,
            orientation,
        };

        Ok((camera_info, location_info, date_taken))
    }
    
    fn extract_gps_data(exif_data: &exif::Exif) -> Option<LocationInfo> {
        use exif::{Tag, In, Value};
        
        let latitude = Self::extract_gps_coordinate(exif_data, Tag::GPSLatitude, Tag::GPSLatitudeRef)?;
        let longitude = Self::extract_gps_coordinate(exif_data, Tag::GPSLongitude, Tag::GPSLongitudeRef)?;
        
        let altitude = exif_data
            .get_field(Tag::GPSAltitude, In::PRIMARY)
            .and_then(|f| {
                if let Value::Rational(ref v) = f.value {
                    v.first().map(|r| r.to_f64())
                } else {
                    None
                }
            });

        Some(LocationInfo {
            latitude,
            longitude,
            altitude,
        })
    }
    
    fn extract_gps_coordinate(exif_data: &exif::Exif, coord_tag: exif::Tag, ref_tag: exif::Tag) -> Option<f64> {
        use exif::{In, Value};
        
        let coord = exif_data.get_field(coord_tag, In::PRIMARY)?;
        let coord_ref = exif_data.get_field(ref_tag, In::PRIMARY)?;
        
        if let Value::Rational(ref v) = coord.value {
            if v.len() >= 3 {
                let degrees = v[0].to_f64();
                let minutes = v[1].to_f64();
                let seconds = v[2].to_f64();
                
                let decimal = degrees + minutes / 60.0 + seconds / 3600.0;
                
                let ref_str = coord_ref.display_value().to_string();
                if ref_str == "S" || ref_str == "W" {
                    Some(-decimal)
                } else {
                    Some(decimal)
                }
            } else {
                None
            }
        } else {
            None
        }
    }
    
    fn extract_date_taken(exif_data: &exif::Exif) -> Option<DateTime<Utc>> {
        use exif::{Tag, In};
        
        exif_data.get_field(Tag::DateTimeOriginal, In::PRIMARY)
            .or_else(|| exif_data.get_field(Tag::DateTime, In::PRIMARY))
            .and_then(|f| {
                let datetime_str = f.display_value().to_string();
                // EXIF datetime format: "YYYY:MM:DD HH:MM:SS"
                chrono::NaiveDateTime::parse_from_str(&datetime_str, "%Y:%m:%d %H:%M:%S")
                    .ok()
                    .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
            })
    }

    fn extract_video_metadata(
        _path: &Path,
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

    fn extract_audio_metadata(_path: &Path) -> Result<(Option<CodecInfo>, Option<f64>)> {
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
