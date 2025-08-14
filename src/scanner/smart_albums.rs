use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json;
use uuid::Uuid;

use crate::db::Database;
use crate::models::{MediaFile, SmartAlbum, SmartAlbumFilter, DateRange, LocationRadius};

pub struct SmartAlbumManager {
    db: Database,
}

impl SmartAlbumManager {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Create a new smart album with specified filters
    pub async fn create_smart_album(
        &self,
        name: String,
        description: Option<String>,
        filter: SmartAlbumFilter,
    ) -> Result<SmartAlbum> {
        let filter_json = serde_json::to_string(&filter)?;
        
        let album = SmartAlbum {
            id: Uuid::new_v4().to_string(),
            album_name: name,
            description,
            filter_rules: filter_json,
            sort_order: Some("date_desc".to_string()),
            media_count: 0,
            cover_media_id: None,
            is_public: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_refreshed_at: None,
        };

        self.db.insert_smart_album(&album).await?;
        
        // Refresh the album to populate it with matching media
        self.refresh_smart_album(&album.id).await?;

        Ok(album)
    }

    /// Create predefined smart albums for common use cases
    pub async fn create_default_smart_albums(&self) -> Result<Vec<SmartAlbum>> {
        let mut albums = Vec::new();

        // Recent Photos (last 30 days)
        let recent_filter = SmartAlbumFilter {
            media_type: Some(vec!["image".to_string()]),
            date_range: Some(DateRange {
                start: Some(Utc::now() - chrono::Duration::days(30)),
                end: Some(Utc::now()),
            }),
            location_radius: None,
            camera_make: None,
            has_faces: None,
            min_resolution: None,
            tags: None,
        };
        albums.push(
            self.create_smart_album(
                "Recent Photos".to_string(),
                Some("Photos from the last 30 days".to_string()),
                recent_filter,
            ).await?
        );

        // Videos
        let videos_filter = SmartAlbumFilter {
            media_type: Some(vec!["video".to_string()]),
            date_range: None,
            location_radius: None,
            camera_make: None,
            has_faces: None,
            min_resolution: None,
            tags: None,
        };
        albums.push(
            self.create_smart_album(
                "All Videos".to_string(),
                Some("All video files in the library".to_string()),
                videos_filter,
            ).await?
        );

        // High Resolution Photos
        let hires_filter = SmartAlbumFilter {
            media_type: Some(vec!["image".to_string()]),
            date_range: None,
            location_radius: None,
            camera_make: None,
            has_faces: None,
            min_resolution: Some(3840), // 4K resolution
            tags: None,
        };
        albums.push(
            self.create_smart_album(
                "High Resolution".to_string(),
                Some("Photos with 4K or higher resolution".to_string()),
                hires_filter,
            ).await?
        );

        // Photos with Faces
        let faces_filter = SmartAlbumFilter {
            media_type: Some(vec!["image".to_string()]),
            date_range: None,
            location_radius: None,
            camera_make: None,
            has_faces: Some(true),
            min_resolution: None,
            tags: None,
        };
        albums.push(
            self.create_smart_album(
                "People".to_string(),
                Some("Photos containing detected faces".to_string()),
                faces_filter,
            ).await?
        );

        Ok(albums)
    }

    /// Refresh a smart album by re-evaluating its filters
    pub async fn refresh_smart_album(&self, album_id: &str) -> Result<()> {
        let album = self.db.get_smart_album(album_id).await?
            .ok_or_else(|| anyhow::anyhow!("Smart album not found"))?;

        let filter: SmartAlbumFilter = serde_json::from_str(&album.filter_rules)?;
        
        // Clear existing members
        self.db.clear_smart_album_members(album_id).await?;

        // Get all media files
        let all_media = self.db.list_all_media().await?;
        
        let mut matching_media = Vec::new();
        for media in all_media {
            if self.media_matches_filter(&media, &filter).await? {
                matching_media.push(media);
            }
        }

        // Sort the media based on sort_order
        self.sort_media(&mut matching_media, album.sort_order.as_deref());

        // Update album with new count and cover
        let media_count = matching_media.len() as i32;
        let cover_media_id = matching_media.first().map(|m| m.id.clone());

        // Add matching media to the album
        for media in &matching_media {
            self.db.add_media_to_smart_album(album_id, &media.id, 1.0).await?;
        }

        // Update album metadata
        self.db.update_smart_album_metadata(album_id, media_count, cover_media_id).await?;

        Ok(())
    }

    /// Check if a media file matches the smart album filter
    async fn media_matches_filter(&self, media: &MediaFile, filter: &SmartAlbumFilter) -> Result<bool> {
        // Check media type
        if let Some(ref types) = filter.media_type {
            if !types.contains(&media.media_type) {
                return Ok(false);
            }
        }

        // Check date range
        if let Some(ref date_range) = filter.date_range {
            let media_date = media.file_created_at.or(media.file_modified_at);
            
            if let Some(media_date) = media_date {
                if let Some(start) = date_range.start {
                    if media_date < start {
                        return Ok(false);
                    }
                }
                if let Some(end) = date_range.end {
                    if media_date > end {
                        return Ok(false);
                    }
                }
            } else {
                // No date information, exclude if date filter is set
                return Ok(false);
            }
        }

        // Check location radius
        if let Some(ref location) = filter.location_radius {
            // Extract media location (would need to be added to MediaFile or parsed from extra_metadata)
            if let (Some(lat), Some(lon)) = (media.latitude(), media.longitude()) {
                let distance = Self::distance_km(lat, lon, location.latitude, location.longitude);
                if distance > location.radius_km {
                    return Ok(false);
                }
            } else {
                // No location information, exclude if location filter is set
                return Ok(false);
            }
        }

        // Check camera make
        if let Some(ref camera_makes) = filter.camera_make {
            if let Some(ref camera_make) = media.camera_make {
                if !camera_makes.iter().any(|make| camera_make.contains(make)) {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        // Check has faces
        if let Some(has_faces) = filter.has_faces {
            let face_count = self.db.get_face_count_for_media(&media.id).await?;
            if has_faces && face_count == 0 {
                return Ok(false);
            }
            if !has_faces && face_count > 0 {
                return Ok(false);
            }
        }

        // Check minimum resolution
        if let Some(min_resolution) = filter.min_resolution {
            if let (Some(width), Some(height)) = (media.width, media.height) {
                let max_dimension = width.max(height) as u32;
                if max_dimension < min_resolution {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Sort media based on the specified order
    fn sort_media(&self, media: &mut Vec<MediaFile>, sort_order: Option<&str>) {
        match sort_order {
            Some("date_asc") => {
                media.sort_by(|a, b| {
                    let a_date = a.file_created_at.or(a.file_modified_at);
                    let b_date = b.file_created_at.or(b.file_modified_at);
                    a_date.cmp(&b_date)
                });
            }
            Some("date_desc") | None => {
                media.sort_by(|a, b| {
                    let a_date = a.file_created_at.or(a.file_modified_at);
                    let b_date = b.file_created_at.or(b.file_modified_at);
                    b_date.cmp(&a_date)
                });
            }
            Some("name_asc") => {
                media.sort_by(|a, b| a.file_name.cmp(&b.file_name));
            }
            Some("name_desc") => {
                media.sort_by(|a, b| b.file_name.cmp(&a.file_name));
            }
            Some("size_asc") => {
                media.sort_by_key(|m| m.file_size);
            }
            Some("size_desc") => {
                media.sort_by_key(|m| -m.file_size);
            }
            _ => {}
        }
    }

    /// Calculate distance between two coordinates in kilometers
    fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        const EARTH_RADIUS_KM: f64 = 6371.0;

        let dlat = (lat2 - lat1).to_radians();
        let dlon = (lon2 - lon1).to_radians();

        let lat1_rad = lat1.to_radians();
        let lat2_rad = lat2.to_radians();

        let a = (dlat / 2.0).sin().powi(2) + lat1_rad.cos() * lat2_rad.cos() * (dlon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        EARTH_RADIUS_KM * c
    }

    /// Get smart album suggestions based on user's media library
    pub async fn suggest_smart_albums(&self) -> Result<Vec<SmartAlbumFilter>> {
        let mut suggestions = Vec::new();

        // Analyze the media library to suggest relevant smart albums
        let stats = self.db.get_media_statistics().await?;

        // Suggest by camera makes if there are multiple
        let camera_makes = self.db.get_unique_camera_makes().await?;
        for camera_make in camera_makes {
            if !camera_make.is_empty() {
                suggestions.push(SmartAlbumFilter {
                    media_type: Some(vec!["image".to_string()]),
                    date_range: None,
                    location_radius: None,
                    camera_make: Some(vec![camera_make]),
                    has_faces: None,
                    min_resolution: None,
                    tags: None,
                });
            }
        }

        // Suggest by year if media spans multiple years
        let years = self.db.get_media_years().await?;
        for year in years {
            let start = DateTime::parse_from_rfc3339(&format!("{}-01-01T00:00:00Z", year))
                .unwrap()
                .with_timezone(&Utc);
            let end = DateTime::parse_from_rfc3339(&format!("{}-12-31T23:59:59Z", year))
                .unwrap()
                .with_timezone(&Utc);
            
            suggestions.push(SmartAlbumFilter {
                media_type: None,
                date_range: Some(DateRange {
                    start: Some(start),
                    end: Some(end),
                }),
                location_radius: None,
                camera_make: None,
                has_faces: None,
                min_resolution: None,
                tags: None,
            });
        }

        Ok(suggestions)
    }
}

