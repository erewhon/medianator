use anyhow::Result;

use crate::models::{MediaFile, MediaGroup, MediaGroupWithItems, SmartAlbum};

impl super::Database {
    // Media Groups
    
    pub async fn get_all_media_groups(&self) -> Result<Vec<MediaGroup>> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                id, group_type, group_name, group_date, 
                latitude, longitude, location_name,
                media_count, total_size, cover_media_id,
                created_at, updated_at
            FROM media_groups
            ORDER BY updated_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let groups = rows.into_iter().map(|row| MediaGroup {
            id: row.id,
            group_type: row.group_type,
            group_name: row.group_name,
            group_date: row.group_date,
            latitude: row.latitude,
            longitude: row.longitude,
            location_name: row.location_name,
            media_count: row.media_count as i32,
            total_size: row.total_size,
            cover_media_id: row.cover_media_id,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }).collect();

        Ok(groups)
    }

    pub async fn get_media_group_with_items(&self, group_id: &str) -> Result<Option<MediaGroupWithItems>> {
        let group_row = sqlx::query!(
            r#"
            SELECT 
                id, group_type, group_name, group_date,
                latitude, longitude, location_name,
                media_count, total_size, cover_media_id,
                created_at, updated_at
            FROM media_groups
            WHERE id = ?1
            "#,
            group_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = group_row {
            let group = MediaGroup {
                id: row.id,
                group_type: row.group_type,
                group_name: row.group_name,
                group_date: row.group_date,
                latitude: row.latitude,
                longitude: row.longitude,
                location_name: row.location_name,
                media_count: row.media_count as i32,
                total_size: row.total_size,
                cover_media_id: row.cover_media_id,
                created_at: row.created_at.and_utc(),
                updated_at: row.updated_at.and_utc(),
            };

            // For media items, we'll use a simpler query
            let media_items = self.get_media_by_group(group_id).await?;

            Ok(Some(MediaGroupWithItems {
                group,
                media_items,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_media_by_group(&self, group_id: &str) -> Result<Vec<MediaFile>> {
        // Use a simpler approach - fetch IDs first, then use existing method
        let media_ids: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT media_id 
            FROM media_group_members 
            WHERE group_id = ?1
            "#,
            group_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut media_files = Vec::new();
        for media_id in media_ids {
            if let Some(media) = self.get_media_by_id(&media_id).await? {
                media_files.push(media);
            }
        }

        Ok(media_files)
    }

    pub async fn insert_media_group(&self, group: &MediaGroup) -> Result<()> {

        sqlx::query!(
            r#"
            INSERT INTO media_groups (
                id, group_type, group_name, group_date,
                latitude, longitude, location_name,
                media_count, total_size, cover_media_id,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            group.id,
            group.group_type,
            group.group_name,
            group.group_date,
            group.latitude,
            group.longitude,
            group.location_name,
            group.media_count,
            group.total_size,
            group.cover_media_id,
            group.created_at,
            group.updated_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_media_to_group(&self, media_id: &str, group_id: &str) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO media_group_members (media_id, group_id)
            VALUES (?1, ?2)
            ON CONFLICT DO NOTHING
            "#,
            media_id,
            group_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_all_media_with_dates(&self) -> Result<Vec<MediaFile>> {
        // Use existing list_media with a filter
        let all_media = self.list_media(None, 10000, 0).await?;
        Ok(all_media.into_iter()
            .filter(|m| m.date_taken.is_some() || m.file_created_at.is_some())
            .collect())
    }

    pub async fn get_all_media_with_location(&self) -> Result<Vec<MediaFile>> {
        // Use existing list_media with a filter
        let all_media = self.list_media(None, 10000, 0).await?;
        Ok(all_media.into_iter()
            .filter(|m| m.latitude.is_some() && m.longitude.is_some())
            .collect())
    }

    pub async fn get_all_media_with_dates_and_location(&self) -> Result<Vec<MediaFile>> {
        // Use existing list_media with a filter
        let all_media = self.list_media(None, 10000, 0).await?;
        Ok(all_media.into_iter()
            .filter(|m| {
                (m.date_taken.is_some() || m.file_created_at.is_some()) ||
                (m.latitude.is_some() && m.longitude.is_some())
            })
            .collect())
    }

    // Smart Albums
    
    pub async fn get_all_smart_albums(&self) -> Result<Vec<SmartAlbum>> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                id, album_name, description, filter_rules,
                sort_order, media_count, cover_media_id,
                is_public, created_at, updated_at, last_refreshed_at
            FROM smart_albums
            ORDER BY album_name
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let albums = rows.into_iter().map(|row| SmartAlbum {
            id: row.id,
            album_name: row.album_name,
            description: row.description,
            filter_rules: row.filter_rules,
            sort_order: row.sort_order,
            media_count: row.media_count as i32,
            cover_media_id: row.cover_media_id,
            is_public: row.is_public.unwrap_or(false),
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
            last_refreshed_at: row.last_refreshed_at.map(|dt| dt.and_utc()),
        }).collect();

        Ok(albums)
    }

    pub async fn get_smart_album(&self, album_id: &str) -> Result<Option<SmartAlbum>> {
        let row = sqlx::query!(
            r#"
            SELECT 
                id, album_name, description, filter_rules,
                sort_order, media_count, cover_media_id,
                is_public, created_at, updated_at, last_refreshed_at
            FROM smart_albums
            WHERE id = ?1
            "#,
            album_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| SmartAlbum {
            id: row.id,
            album_name: row.album_name,
            description: row.description,
            filter_rules: row.filter_rules,
            sort_order: row.sort_order,
            media_count: row.media_count as i32,
            cover_media_id: row.cover_media_id,
            is_public: row.is_public.unwrap_or(false),
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
            last_refreshed_at: row.last_refreshed_at.map(|dt| dt.and_utc()),
        }))
    }

    pub async fn insert_smart_album(&self, album: &SmartAlbum) -> Result<()> {
        let is_public = if album.is_public { 1 } else { 0 };

        sqlx::query!(
            r#"
            INSERT INTO smart_albums (
                id, album_name, description, filter_rules,
                sort_order, media_count, cover_media_id,
                is_public, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            album.id,
            album.album_name,
            album.description,
            album.filter_rules,
            album.sort_order,
            album.media_count,
            album.cover_media_id,
            is_public,
            album.created_at,
            album.updated_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_smart_album_media(&self, album_id: &str) -> Result<Vec<MediaFile>> {
        // Fetch media IDs from smart_album_members
        let media_ids: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT media_id 
            FROM smart_album_members 
            WHERE album_id = ?1
            ORDER BY match_score DESC
            "#,
            album_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut media_files = Vec::new();
        for media_id in media_ids {
            if let Some(media) = self.get_media_by_id(&media_id).await? {
                media_files.push(media);
            }
        }

        Ok(media_files)
    }

    pub async fn clear_smart_album_members(&self, album_id: &str) -> Result<()> {
        sqlx::query!(
            "DELETE FROM smart_album_members WHERE album_id = ?1",
            album_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_media_to_smart_album(&self, album_id: &str, media_id: &str, match_score: f64) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO smart_album_members (album_id, media_id, match_score)
            VALUES (?1, ?2, ?3)
            ON CONFLICT DO UPDATE SET match_score = ?3
            "#,
            album_id,
            media_id,
            match_score
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_smart_album_metadata(&self, album_id: &str, media_count: i32, cover_media_id: Option<String>) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE smart_albums
            SET media_count = ?1,
                cover_media_id = ?2,
                last_refreshed_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?3
            "#,
            media_count,
            cover_media_id,
            album_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_all_media(&self) -> Result<Vec<MediaFile>> {
        // Use existing list_media method
        self.list_media(None, 10000, 0).await
    }

    pub async fn get_face_count_for_media(&self, media_id: &str) -> Result<i32> {
        let result = sqlx::query!(
            "SELECT COUNT(*) as count FROM faces WHERE media_file_id = ?1",
            media_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(result.count as i32)
    }

    pub async fn get_media_statistics(&self) -> Result<serde_json::Value> {
        let total_files = sqlx::query!("SELECT COUNT(*) as count FROM media_files")
            .fetch_one(&self.pool)
            .await?
            .count;

        let total_size = sqlx::query!("SELECT SUM(file_size) as size FROM media_files")
            .fetch_one(&self.pool)
            .await?
            .size
            .unwrap_or(0);

        let by_type = sqlx::query!(
            r#"
            SELECT media_type, COUNT(*) as count
            FROM media_files
            GROUP BY media_type
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut type_counts = serde_json::Map::new();
        for row in by_type {
            type_counts.insert(row.media_type, serde_json::Value::Number(row.count.into()));
        }

        Ok(serde_json::json!({
            "total_files": total_files,
            "total_size": total_size,
            "by_type": type_counts
        }))
    }

    pub async fn get_unique_camera_makes(&self) -> Result<Vec<String>> {
        let makes = sqlx::query!(
            r#"
            SELECT DISTINCT camera_make
            FROM media_files
            WHERE camera_make IS NOT NULL
            ORDER BY camera_make
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(makes.into_iter()
            .filter_map(|r| r.camera_make)
            .collect())
    }

    pub async fn get_media_years(&self) -> Result<Vec<i32>> {
        let years = sqlx::query!(
            r#"
            SELECT DISTINCT strftime('%Y', COALESCE(date_taken, file_created_at)) as year
            FROM media_files
            WHERE date_taken IS NOT NULL OR file_created_at IS NOT NULL
            ORDER BY year DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(years.into_iter()
            .filter_map(|r| r.year.and_then(|y| y.parse().ok()))
            .collect())
    }
}