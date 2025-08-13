use anyhow::Result;
use sqlx::{migrate::MigrateDatabase, Pool, Sqlite, SqlitePool};
use std::path::Path;
use tracing::info;

use crate::models::{MediaFile, MediaMetadata, ScanHistory};

pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        if !Sqlite::database_exists(database_url).await? {
            info!("Creating database: {}", database_url);
            Sqlite::create_database(database_url).await?;
        }

        let pool = SqlitePool::connect(database_url).await?;
        
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;

        Ok(Self { pool })
    }

    pub async fn insert_media_file(&self, media: &MediaMetadata) -> Result<()> {
        let media_type_str: String = media.media_type.clone().into();
        let extra_json = media.extra.as_ref().map(|v| v.to_string());

        sqlx::query!(
            r#"
            INSERT INTO media_files (
                id, file_path, file_name, file_size, file_hash,
                media_type, mime_type, width, height, duration_seconds,
                bit_rate, camera_make, camera_model, lens_model,
                focal_length, aperture, iso, shutter_speed, orientation,
                codec, frame_rate, audio_channels, audio_sample_rate,
                file_created_at, file_modified_at, indexed_at, last_scanned_at,
                extra_metadata
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28
            )
            ON CONFLICT(file_path) DO UPDATE SET
                file_size = excluded.file_size,
                file_hash = excluded.file_hash,
                mime_type = excluded.mime_type,
                width = excluded.width,
                height = excluded.height,
                duration_seconds = excluded.duration_seconds,
                last_scanned_at = excluded.last_scanned_at,
                extra_metadata = excluded.extra_metadata
            "#,
            media.id,
            media.file_path,
            media.file_name,
            media.file_size,
            media.file_hash,
            media_type_str,
            media.mime_type,
            media.dimensions.as_ref().map(|d| d.width as i32),
            media.dimensions.as_ref().map(|d| d.height as i32),
            media.duration_seconds,
            media.codec_info.as_ref().and_then(|c| c.bit_rate),
            media.camera_info.as_ref().and_then(|c| c.make.clone()),
            media.camera_info.as_ref().and_then(|c| c.model.clone()),
            media.camera_info.as_ref().and_then(|c| c.lens_model.clone()),
            media.camera_info.as_ref().and_then(|c| c.focal_length),
            media.camera_info.as_ref().and_then(|c| c.aperture),
            media.camera_info.as_ref().and_then(|c| c.iso),
            media.camera_info.as_ref().and_then(|c| c.shutter_speed.clone()),
            media.camera_info.as_ref().and_then(|c| c.orientation),
            media.codec_info.as_ref().map(|c| c.codec.clone()),
            media.codec_info.as_ref().and_then(|c| c.frame_rate),
            media.codec_info.as_ref().and_then(|c| c.audio_channels),
            media.codec_info.as_ref().and_then(|c| c.audio_sample_rate),
            media.timestamps.created,
            media.timestamps.modified,
            media.timestamps.indexed,
            media.timestamps.last_scanned,
            extra_json
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_media_by_id(&self, id: &str) -> Result<Option<MediaFile>> {
        let media = sqlx::query_as!(
            MediaFile,
            r#"
            SELECT * FROM media_files WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(media)
    }

    pub async fn get_media_by_path(&self, path: &str) -> Result<Option<MediaFile>> {
        let media = sqlx::query_as!(
            MediaFile,
            r#"
            SELECT * FROM media_files WHERE file_path = ?1
            "#,
            path
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(media)
    }

    pub async fn list_media(
        &self,
        media_type: Option<String>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<MediaFile>> {
        let media = if let Some(mt) = media_type {
            sqlx::query_as!(
                MediaFile,
                r#"
                SELECT * FROM media_files
                WHERE media_type = ?1
                ORDER BY indexed_at DESC
                LIMIT ?2 OFFSET ?3
                "#,
                mt,
                limit,
                offset
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as!(
                MediaFile,
                r#"
                SELECT * FROM media_files
                ORDER BY indexed_at DESC
                LIMIT ?1 OFFSET ?2
                "#,
                limit,
                offset
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(media)
    }

    pub async fn search_media(&self, query: &str) -> Result<Vec<MediaFile>> {
        let search_pattern = format!("%{}%", query);
        let media = sqlx::query_as!(
            MediaFile,
            r#"
            SELECT * FROM media_files
            WHERE file_name LIKE ?1 OR file_path LIKE ?1
            ORDER BY indexed_at DESC
            LIMIT 100
            "#,
            search_pattern
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(media)
    }

    pub async fn create_scan_history(&self, scan_path: &str) -> Result<i32> {
        let result = sqlx::query!(
            r#"
            INSERT INTO scan_history (scan_path, status)
            VALUES (?1, 'running')
            "#,
            scan_path
        )
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid() as i32)
    }

    pub async fn update_scan_progress(
        &self,
        scan_id: i32,
        files_scanned: i32,
        files_added: i32,
        files_updated: i32,
        error_count: i32,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE scan_history
            SET files_scanned = ?2,
                files_added = ?3,
                files_updated = ?4,
                error_count = ?5
            WHERE id = ?1
            "#,
            scan_id,
            files_scanned,
            files_added,
            files_updated,
            error_count
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn complete_scan(&self, scan_id: i32, status: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE scan_history
            SET completed_at = CURRENT_TIMESTAMP,
                status = ?2
            WHERE id = ?1
            "#,
            scan_id,
            status
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_scan_history(&self, limit: i32) -> Result<Vec<ScanHistory>> {
        let history = sqlx::query_as!(
            ScanHistory,
            r#"
            SELECT * FROM scan_history
            ORDER BY started_at DESC
            LIMIT ?1
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    pub async fn get_stats(&self) -> Result<serde_json::Value> {
        let total_count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as count FROM media_files"#
        )
        .fetch_one(&self.pool)
        .await?;

        let image_count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as count FROM media_files WHERE media_type = 'image'"#
        )
        .fetch_one(&self.pool)
        .await?;

        let video_count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as count FROM media_files WHERE media_type = 'video'"#
        )
        .fetch_one(&self.pool)
        .await?;

        let audio_count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as count FROM media_files WHERE media_type = 'audio'"#
        )
        .fetch_one(&self.pool)
        .await?;

        let total_size = sqlx::query_scalar!(
            r#"SELECT SUM(file_size) as size FROM media_files"#
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        Ok(serde_json::json!({
            "total_files": total_count,
            "image_files": image_count,
            "video_files": video_count,
            "audio_files": audio_count,
            "total_size_bytes": total_size,
        }))
    }
}