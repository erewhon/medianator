use anyhow::Result;
use chrono::{NaiveDate, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::Database;
use crate::models::{MediaFile, MediaGroup};

const LOCATION_CLUSTER_RADIUS_KM: f64 = 1.0; // 1km radius for location clustering

pub struct MediaGrouper {
    db: Database,
}

impl MediaGrouper {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Auto-group media files by date (same day)
    pub async fn group_by_date(&self) -> Result<Vec<MediaGroup>> {
        let media_files = self.db.get_all_media_with_dates().await?;
        let mut date_groups: HashMap<NaiveDate, Vec<MediaFile>> = HashMap::new();

        for file in media_files {
            if let Some(date_taken) = file.file_created_at {
                let date = date_taken.date_naive();
                date_groups.entry(date).or_insert_with(Vec::new).push(file);
            }
        }

        let mut groups = Vec::new();
        for (date, files) in date_groups {
            if files.is_empty() {
                continue;
            }

            let group_name = format!("{}", date.format("%B %d, %Y"));
            let total_size: i64 = files.iter().map(|f| f.file_size).sum();
            let cover_media_id = files.first().map(|f| f.id.clone());

            let group = MediaGroup {
                id: Uuid::new_v4().to_string(),
                group_type: "date".to_string(),
                group_name,
                group_date: Some(date),
                latitude: None,
                longitude: None,
                location_name: None,
                media_count: files.len() as i32,
                total_size,
                cover_media_id,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            // Insert group into database
            self.db.insert_media_group(&group).await?;

            // Add members to group
            for file in &files {
                self.db.add_media_to_group(&file.id, &group.id).await?;
            }

            groups.push(group);
        }

        Ok(groups)
    }

    /// Auto-group media files by location using clustering
    pub async fn group_by_location(&self) -> Result<Vec<MediaGroup>> {
        let media_files = self.db.get_all_media_with_location().await?;
        let mut location_clusters: Vec<LocationCluster> = Vec::new();

        for file in media_files {
            // Try to find an existing cluster within radius
            let mut added_to_cluster = false;
            
            // Extract location from the file (assuming we have latitude/longitude in the database)
            if let (Some(lat), Some(lon)) = (file.latitude, file.longitude) {
                for cluster in &mut location_clusters {
                    if Self::distance_km(lat, lon, cluster.center_lat, cluster.center_lon) <= LOCATION_CLUSTER_RADIUS_KM {
                        cluster.add_file(file.clone(), lat, lon);
                        added_to_cluster = true;
                        break;
                    }
                }

                if !added_to_cluster {
                    let mut cluster = LocationCluster::new(lat, lon);
                    cluster.add_file(file, lat, lon);
                    location_clusters.push(cluster);
                }
            }
        }

        let mut groups = Vec::new();
        for cluster in location_clusters {
            if cluster.files.is_empty() {
                continue;
            }

            let location_name = self.reverse_geocode(cluster.center_lat, cluster.center_lon).await.unwrap_or_else(|_| {
                format!("{:.4}°, {:.4}°", cluster.center_lat, cluster.center_lon)
            });

            let total_size: i64 = cluster.files.iter().map(|f| f.file_size).sum();
            let cover_media_id = cluster.files.first().map(|f| f.id.clone());

            let group = MediaGroup {
                id: Uuid::new_v4().to_string(),
                group_type: "location".to_string(),
                group_name: location_name.clone(),
                group_date: None,
                latitude: Some(cluster.center_lat),
                longitude: Some(cluster.center_lon),
                location_name: Some(location_name),
                media_count: cluster.files.len() as i32,
                total_size,
                cover_media_id,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            // Insert group into database
            self.db.insert_media_group(&group).await?;

            // Add members to group
            for file in &cluster.files {
                self.db.add_media_to_group(&file.id, &group.id).await?;
            }

            groups.push(group);
        }

        Ok(groups)
    }

    /// Auto-group media by events (combination of date and location proximity)
    pub async fn group_by_events(&self) -> Result<Vec<MediaGroup>> {
        let media_files = self.db.get_all_media_with_dates_and_location().await?;
        let mut event_clusters: Vec<EventCluster> = Vec::new();

        for file in media_files {
            let mut added_to_cluster = false;

            if let Some(date_taken) = file.file_created_at {
                let date = date_taken.date_naive();
                
                for cluster in &mut event_clusters {
                    // Check if file belongs to the same event (same day or adjacent days + nearby location)
                    let days_diff = (date - cluster.date).num_days().abs();
                    
                    if days_diff <= 1 {
                        // Check location if available
                        if let (Some(lat), Some(lon)) = (file.latitude, file.longitude) {
                            if let (Some(cluster_lat), Some(cluster_lon)) = (cluster.latitude, cluster.longitude) {
                                if Self::distance_km(lat, lon, cluster_lat, cluster_lon) <= LOCATION_CLUSTER_RADIUS_KM * 2.0 {
                                    cluster.add_file(file.clone(), Some(lat), Some(lon));
                                    added_to_cluster = true;
                                    break;
                                }
                            }
                        } else if cluster.latitude.is_none() {
                            // Both have no location, group by date only
                            cluster.add_file(file.clone(), None, None);
                            added_to_cluster = true;
                            break;
                        }
                    }
                }

                if !added_to_cluster {
                    let mut cluster = EventCluster::new(date, file.latitude, file.longitude);
                    cluster.add_file(file, None, None);
                    event_clusters.push(cluster);
                }
            }
        }

        let mut groups = Vec::new();
        for cluster in event_clusters {
            if cluster.files.len() < 3 {
                // Skip small clusters for events
                continue;
            }

            let event_name = self.generate_event_name(&cluster).await;
            let total_size: i64 = cluster.files.iter().map(|f| f.file_size).sum();
            let cover_media_id = cluster.files.first().map(|f| f.id.clone());

            let group = MediaGroup {
                id: Uuid::new_v4().to_string(),
                group_type: "event".to_string(),
                group_name: event_name,
                group_date: Some(cluster.date),
                latitude: cluster.latitude,
                longitude: cluster.longitude,
                location_name: cluster.location_name.clone(),
                media_count: cluster.files.len() as i32,
                total_size,
                cover_media_id,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            // Insert group into database
            self.db.insert_media_group(&group).await?;

            // Add members to group
            for file in &cluster.files {
                self.db.add_media_to_group(&file.id, &group.id).await?;
            }

            groups.push(group);
        }

        Ok(groups)
    }

    /// Calculate distance between two coordinates in kilometers using Haversine formula
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

    /// Reverse geocode coordinates to location name (placeholder - would use external service)
    async fn reverse_geocode(&self, lat: f64, lon: f64) -> Result<String> {
        // In a real implementation, this would call a geocoding API
        // For now, return a formatted coordinate string
        Ok(format!("Location at {:.2}°, {:.2}°", lat, lon))
    }

    /// Generate a descriptive name for an event cluster
    async fn generate_event_name(&self, cluster: &EventCluster) -> String {
        let date_str = cluster.date.format("%B %d, %Y");
        
        if let Some(location_name) = &cluster.location_name {
            format!("{} at {}", date_str, location_name)
        } else {
            format!("Event on {}", date_str)
        }
    }
}

struct LocationCluster {
    center_lat: f64,
    center_lon: f64,
    files: Vec<MediaFile>,
}

impl LocationCluster {
    fn new(lat: f64, lon: f64) -> Self {
        Self {
            center_lat: lat,
            center_lon: lon,
            files: Vec::new(),
        }
    }

    fn add_file(&mut self, file: MediaFile, lat: f64, lon: f64) {
        // Update center as weighted average
        let n = self.files.len() as f64;
        self.center_lat = (self.center_lat * n + lat) / (n + 1.0);
        self.center_lon = (self.center_lon * n + lon) / (n + 1.0);
        self.files.push(file);
    }
}

struct EventCluster {
    date: NaiveDate,
    latitude: Option<f64>,
    longitude: Option<f64>,
    location_name: Option<String>,
    files: Vec<MediaFile>,
}

impl EventCluster {
    fn new(date: NaiveDate, lat: Option<f64>, lon: Option<f64>) -> Self {
        Self {
            date,
            latitude: lat,
            longitude: lon,
            location_name: None,
            files: Vec::new(),
        }
    }

    fn add_file(&mut self, file: MediaFile, lat: Option<f64>, lon: Option<f64>) {
        if let (Some(new_lat), Some(new_lon)) = (lat, lon) {
            if let (Some(cur_lat), Some(cur_lon)) = (self.latitude, self.longitude) {
                // Update location as weighted average
                let n = self.files.len() as f64;
                self.latitude = Some((cur_lat * n + new_lat) / (n + 1.0));
                self.longitude = Some((cur_lon * n + new_lon) / (n + 1.0));
            } else {
                self.latitude = Some(new_lat);
                self.longitude = Some(new_lon);
            }
        }
        self.files.push(file);
    }
}

