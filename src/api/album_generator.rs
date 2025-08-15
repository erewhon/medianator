use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use tracing::{info, error};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlbumCriteria {
    pub min_media_count: usize,
    pub confidence_threshold: f32,
    pub album_types: Vec<String>, // category, scene, object, event
}

impl Default for AlbumCriteria {
    fn default() -> Self {
        Self {
            min_media_count: 3,
            confidence_threshold: 0.7,
            album_types: vec![
                "category".to_string(),
                "scene".to_string(),
                "object".to_string(),
            ],
        }
    }
}

pub struct AlbumGenerator {
    pool: SqlitePool,
    criteria: AlbumCriteria,
}

impl AlbumGenerator {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            criteria: AlbumCriteria::default(),
        }
    }

    pub fn with_criteria(mut self, criteria: AlbumCriteria) -> Self {
        self.criteria = criteria;
        self
    }

    pub async fn generate_albums(&self) -> Result<Vec<GeneratedAlbum>> {
        let mut albums = Vec::new();

        if self.criteria.album_types.contains(&"category".to_string()) {
            albums.extend(self.generate_category_albums().await?);
        }

        if self.criteria.album_types.contains(&"scene".to_string()) {
            albums.extend(self.generate_scene_albums().await?);
        }

        if self.criteria.album_types.contains(&"object".to_string()) {
            albums.extend(self.generate_object_albums().await?);
        }

        Ok(albums)
    }

    async fn generate_category_albums(&self) -> Result<Vec<GeneratedAlbum>> {
        let min_media_count = self.criteria.min_media_count as i64;
        let categories = sqlx::query!(
            r#"
            SELECT 
                primary_category,
                COUNT(*) as media_count,
                GROUP_CONCAT(media_file_id) as media_ids
            FROM photo_classifications
            GROUP BY primary_category
            HAVING COUNT(*) >= ?
            "#,
            min_media_count
        )
        .fetch_all(&self.pool)
        .await?;

        let mut albums = Vec::new();
        for cat in categories {
            let album_name = format!("{} Photos", capitalize_first(&cat.primary_category));
            let media_ids: Vec<String> = if cat.media_ids.is_empty() {
                vec![]
            } else {
                cat.media_ids
                    .split(',')
                    .map(|s| s.to_string())
                    .collect()
            };

            albums.push(GeneratedAlbum {
                name: album_name,
                album_type: "category".to_string(),
                criteria: serde_json::json!({
                    "primary_category": cat.primary_category
                }),
                media_count: cat.media_count as usize,
                media_ids: media_ids.clone(),
                cover_media_id: media_ids.first().cloned(),
            });
        }

        Ok(albums)
    }

    async fn generate_scene_albums(&self) -> Result<Vec<GeneratedAlbum>> {
        // Group by scene types from photo classifications
        let min_media_count = self.criteria.min_media_count as i64;
        let scenes = sqlx::query!(
            r#"
            SELECT 
                scene_type,
                COUNT(*) as media_count,
                GROUP_CONCAT(media_file_id) as media_ids
            FROM photo_classifications
            WHERE scene_type IS NOT NULL
            GROUP BY scene_type
            HAVING COUNT(*) >= ?
            "#,
            min_media_count
        )
        .fetch_all(&self.pool)
        .await?;

        let mut albums = Vec::new();
        for scene in scenes {
            if let Some(scene_type) = scene.scene_type {
                let album_name = format!("{} Scenes", capitalize_first(&scene_type));
                let media_ids: Vec<String> = if scene.media_ids.is_empty() {
                    vec![]
                } else {
                    scene.media_ids
                        .split(',')
                        .map(|s| s.to_string())
                        .collect()
                };

                albums.push(GeneratedAlbum {
                    name: album_name,
                    album_type: "scene".to_string(),
                    criteria: serde_json::json!({
                        "scene_type": scene_type
                    }),
                    media_count: scene.media_count as usize,
                    media_ids: media_ids.clone(),
                    cover_media_id: media_ids.first().cloned(),
                });
            }
        }

        Ok(albums)
    }

    async fn generate_object_albums(&self) -> Result<Vec<GeneratedAlbum>> {
        // Group by frequently detected objects
        let confidence_threshold = self.criteria.confidence_threshold;
        let min_media_count = self.criteria.min_media_count as i64;
        let objects = sqlx::query!(
            r#"
            SELECT 
                class_name,
                COUNT(DISTINCT media_file_id) as media_count,
                GROUP_CONCAT(DISTINCT media_file_id) as media_ids,
                AVG(confidence) as avg_confidence
            FROM detected_objects
            WHERE confidence >= ?
            GROUP BY class_name
            HAVING COUNT(DISTINCT media_file_id) >= ?
            ORDER BY media_count DESC
            "#,
            confidence_threshold,
            min_media_count
        )
        .fetch_all(&self.pool)
        .await?;

        let mut albums = Vec::new();
        for obj in objects {
            // Filter out common objects that don't make good albums
            if should_create_object_album(&obj.class_name) {
                let album_name = format!("{} Collection", capitalize_first(&obj.class_name));
                let media_ids: Vec<String> = match obj.media_ids {
                    Some(ids) if !ids.is_empty() => {
                        ids.split(',')
                            .map(|s| s.to_string())
                            .collect()
                    }
                    _ => vec![],
                };

                albums.push(GeneratedAlbum {
                    name: album_name,
                    album_type: "object".to_string(),
                    criteria: serde_json::json!({
                        "detected_object": obj.class_name,
                        "min_confidence": self.criteria.confidence_threshold
                    }),
                    media_count: obj.media_count as usize,
                    media_ids: media_ids.clone(),
                    cover_media_id: media_ids.first().cloned(),
                });
            }
        }

        Ok(albums)
    }

    pub async fn save_albums(&self, albums: &[GeneratedAlbum]) -> Result<()> {
        for album in albums {
            // Check if album already exists
            let existing = sqlx::query!(
                "SELECT id FROM auto_albums WHERE album_name = ?",
                album.name
            )
            .fetch_optional(&self.pool)
            .await?;

            let album_id = if let Some(existing) = existing {
                let existing_id = existing.id.clone().unwrap_or_else(|| String::new());
                let media_count = album.media_count as i64;
                let cover_media_id = album.cover_media_id.clone();
                let id_for_update = existing_id.clone();
                let id_for_delete = existing_id.clone();
                
                // Update existing album
                sqlx::query!(
                    r#"
                    UPDATE auto_albums 
                    SET media_count = ?, 
                        cover_media_id = ?,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE id = ?
                    "#,
                    media_count,
                    cover_media_id,
                    id_for_update
                )
                .execute(&self.pool)
                .await?;

                // Clear existing media associations
                sqlx::query!("DELETE FROM auto_album_media WHERE album_id = ?", id_for_delete)
                    .execute(&self.pool)
                    .await?;

                existing_id
            } else {
                // Create new album
                let id = uuid::Uuid::new_v4().to_string();
                let id_for_insert = id.clone();
                let album_name = album.name.clone();
                let album_type = album.album_type.clone();
                let criteria_str = album.criteria.to_string();
                let media_count = album.media_count as i64;
                let cover_media_id = album.cover_media_id.clone();
                
                sqlx::query!(
                    r#"
                    INSERT INTO auto_albums (id, album_name, album_type, criteria, media_count, cover_media_id)
                    VALUES (?, ?, ?, ?, ?, ?)
                    "#,
                    id_for_insert,
                    album_name,
                    album_type,
                    criteria_str,
                    media_count,
                    cover_media_id
                )
                .execute(&self.pool)
                .await?;
                id
            };

            // Add media associations
            for media_id in &album.media_ids {
                sqlx::query!(
                    r#"
                    INSERT INTO auto_album_media (album_id, media_file_id, confidence)
                    VALUES (?, ?, 1.0)
                    "#,
                    album_id,
                    media_id
                )
                .execute(&self.pool)
                .await?;
            }
        }

        info!("Saved {} auto-generated albums", albums.len());
        Ok(())
    }

    pub async fn analyze_and_create_smart_albums(&self) -> Result<AlbumGenerationReport> {
        info!("Starting smart album generation...");
        
        // Generate albums based on criteria
        let albums = self.generate_albums().await?;
        
        // Save albums to database
        self.save_albums(&albums).await?;
        
        // Create report
        let mut albums_by_type = HashMap::new();
        for album in &albums {
            albums_by_type
                .entry(album.album_type.clone())
                .or_insert_with(Vec::new)
                .push(album.name.clone());
        }
        
        let report = AlbumGenerationReport {
            total_albums_created: albums.len(),
            albums_by_type,
            albums: albums.into_iter().map(|a| AlbumSummary {
                name: a.name,
                album_type: a.album_type,
                media_count: a.media_count,
            }).collect(),
        };
        
        Ok(report)
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct GeneratedAlbum {
    pub name: String,
    pub album_type: String,
    pub criteria: serde_json::Value,
    pub media_count: usize,
    pub media_ids: Vec<String>,
    pub cover_media_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AlbumGenerationReport {
    pub total_albums_created: usize,
    pub albums_by_type: HashMap<String, Vec<String>>,
    pub albums: Vec<AlbumSummary>,
}

#[derive(Debug, Serialize)]
pub struct AlbumSummary {
    pub name: String,
    pub album_type: String,
    pub media_count: usize,
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

fn should_create_object_album(class_name: &str) -> bool {
    // Filter out generic objects that don't make interesting albums
    let interesting_objects = [
        "person", "dog", "cat", "bird", "car", "bicycle", "motorcycle",
        "airplane", "boat", "train", "horse", "elephant", "bear", "zebra",
        "giraffe", "flower", "plant", "cake", "pizza", "sandwich"
    ];
    
    interesting_objects.contains(&class_name.to_lowercase().as_str())
}