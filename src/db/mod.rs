mod face_grouping;
mod groups_albums;

use anyhow::Result;
use sqlx::{migrate::MigrateDatabase, Pool, Sqlite, SqlitePool};
use tracing::info;

use crate::models::{MediaFile, MediaMetadata, ScanHistory, Face, FaceGroup};

#[derive(Clone)]
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

        sqlx::query(
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
            "#)
        .bind(&media.id)
        .bind(&media.file_path)
        .bind(&media.file_name)
        .bind(media.file_size)
        .bind(&media.file_hash)
        .bind(&media_type_str)
        .bind(&media.mime_type)
        .bind(media.dimensions.as_ref().map(|d| d.width as i32))
        .bind(media.dimensions.as_ref().map(|d| d.height as i32))
        .bind(media.duration_seconds)
        .bind(media.codec_info.as_ref().and_then(|c| c.bit_rate))
        .bind(media.camera_info.as_ref().and_then(|c| c.make.clone()))
        .bind(media.camera_info.as_ref().and_then(|c| c.model.clone()))
        .bind(media.camera_info.as_ref().and_then(|c| c.lens_model.clone()))
        .bind(media.camera_info.as_ref().and_then(|c| c.focal_length))
        .bind(media.camera_info.as_ref().and_then(|c| c.aperture))
        .bind(media.camera_info.as_ref().and_then(|c| c.iso))
        .bind(media.camera_info.as_ref().and_then(|c| c.shutter_speed.clone()))
        .bind(media.camera_info.as_ref().and_then(|c| c.orientation))
        .bind(media.codec_info.as_ref().map(|c| c.codec.clone()))
        .bind(media.codec_info.as_ref().and_then(|c| c.frame_rate))
        .bind(media.codec_info.as_ref().and_then(|c| c.audio_channels))
        .bind(media.codec_info.as_ref().and_then(|c| c.audio_sample_rate))
        .bind(media.timestamps.created)
        .bind(media.timestamps.modified)
        .bind(media.timestamps.indexed)
        .bind(media.timestamps.last_scanned)
        .bind(extra_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_sub_image(&self, media: &MediaMetadata, parent_id: &str, extraction_metadata: Option<String>) -> Result<()> {
        let media_type_str: String = media.media_type.clone().into();
        let extra_json = media.extra.as_ref().map(|v| v.to_string());
        
        // Generate a unique index for this sub-image
        let sub_image_index = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT COALESCE(MAX(sub_image_index), -1) + 1
            FROM media_files
            WHERE parent_id = ?1
            "#)
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO media_files (
                id, file_path, file_name, file_size, file_hash,
                media_type, mime_type, width, height, duration_seconds,
                bit_rate, camera_make, camera_model, lens_model,
                focal_length, aperture, iso, shutter_speed, orientation,
                codec, frame_rate, audio_channels, audio_sample_rate,
                file_created_at, file_modified_at, indexed_at, last_scanned_at,
                extra_metadata, parent_id, is_sub_image, sub_image_index, extraction_metadata
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                ?29, ?30, ?31, ?32
            )
            "#)
        .bind(&media.id)
        .bind(&media.file_path)
        .bind(&media.file_name)
        .bind(media.file_size)
        .bind(&media.file_hash)
        .bind(&media_type_str)
        .bind(&media.mime_type)
        .bind(media.dimensions.as_ref().map(|d| d.width as i32))
        .bind(media.dimensions.as_ref().map(|d| d.height as i32))
        .bind(media.duration_seconds)
        .bind(media.codec_info.as_ref().and_then(|c| c.bit_rate))
        .bind(media.camera_info.as_ref().and_then(|c| c.make.clone()))
        .bind(media.camera_info.as_ref().and_then(|c| c.model.clone()))
        .bind(media.camera_info.as_ref().and_then(|c| c.lens_model.clone()))
        .bind(media.camera_info.as_ref().and_then(|c| c.focal_length))
        .bind(media.camera_info.as_ref().and_then(|c| c.aperture))
        .bind(media.camera_info.as_ref().and_then(|c| c.iso))
        .bind(media.camera_info.as_ref().and_then(|c| c.shutter_speed.clone()))
        .bind(media.camera_info.as_ref().and_then(|c| c.orientation))
        .bind(media.codec_info.as_ref().map(|c| c.codec.clone()))
        .bind(media.codec_info.as_ref().and_then(|c| c.frame_rate))
        .bind(media.codec_info.as_ref().and_then(|c| c.audio_channels))
        .bind(media.codec_info.as_ref().and_then(|c| c.audio_sample_rate))
        .bind(media.timestamps.created)
        .bind(media.timestamps.modified)
        .bind(media.timestamps.indexed)
        .bind(media.timestamps.last_scanned)
        .bind(extra_json)
        .bind(parent_id)
        .bind(true) // is_sub_image
        .bind(sub_image_index)
        .bind(extraction_metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_media_by_id(&self, id: &str) -> Result<Option<MediaFile>> {
        let media = sqlx::query_as::<_, MediaFile>(
            r#"
            SELECT * FROM media_files WHERE id = ?1
            "#)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(media)
    }
    
    pub async fn delete_media_file(&self, id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM media_files WHERE id = ?1
            "#)
        .bind(id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn get_media_by_path(&self, path: &str) -> Result<Option<MediaFile>> {
        let media = sqlx::query_as::<_, MediaFile>(
            r#"
            SELECT * FROM media_files WHERE file_path = ?1
            "#)
        .bind(path)
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
            sqlx::query_as::<_, MediaFile>(
                r#"
                SELECT * FROM media_files
                WHERE media_type = ?1
                ORDER BY indexed_at DESC
                LIMIT ?2 OFFSET ?3
                "#)
            .bind(mt)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, MediaFile>(
                r#"
                SELECT * FROM media_files
                ORDER BY indexed_at DESC
                LIMIT ?1 OFFSET ?2
                "#)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(media)
    }

    pub async fn search_media(&self, query: &str) -> Result<Vec<MediaFile>> {
        let search_pattern = format!("%{}%", query);
        let media = sqlx::query_as::<_, MediaFile>(
            r#"
            SELECT * FROM media_files
            WHERE file_name LIKE ?1 OR file_path LIKE ?1
            ORDER BY indexed_at DESC
            LIMIT 100
            "#)
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await?;

        Ok(media)
    }

    pub async fn create_scan_history(&self, scan_path: &str) -> Result<i32> {
        let result = sqlx::query(
            r#"
            INSERT INTO scan_history (scan_path, status)
            VALUES (?1, 'running')
            "#)
        .bind(scan_path)
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
        sqlx::query(
            r#"
            UPDATE scan_history
            SET files_scanned = ?2,
                files_added = ?3,
                files_updated = ?4,
                error_count = ?5
            WHERE id = ?1
            "#)
        .bind(scan_id)
        .bind(files_scanned)
        .bind(files_added)
        .bind(files_updated)
        .bind(error_count)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn complete_scan(&self, scan_id: i32, status: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE scan_history
            SET completed_at = CURRENT_TIMESTAMP,
                status = ?2
            WHERE id = ?1
            "#)
        .bind(scan_id)
        .bind(status)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_scan_history(&self, limit: i32) -> Result<Vec<ScanHistory>> {
        let history = sqlx::query_as::<_, ScanHistory>(
            r#"
            SELECT * FROM scan_history
            ORDER BY started_at DESC
            LIMIT ?1
            "#)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    pub async fn get_stats(&self) -> Result<serde_json::Value> {
        let total_count: i32 = sqlx::query_scalar(
            r#"SELECT COUNT(*) as count FROM media_files"#
        )
        .fetch_one(&self.pool)
        .await?;

        let image_count: i32 = sqlx::query_scalar(
            r#"SELECT COUNT(*) as count FROM media_files WHERE media_type = 'image'"#
        )
        .fetch_one(&self.pool)
        .await?;

        let video_count: i32 = sqlx::query_scalar(
            r#"SELECT COUNT(*) as count FROM media_files WHERE media_type = 'video'"#
        )
        .fetch_one(&self.pool)
        .await?;

        let audio_count: i32 = sqlx::query_scalar(
            r#"SELECT COUNT(*) as count FROM media_files WHERE media_type = 'audio'"#
        )
        .fetch_one(&self.pool)
        .await?;

        let total_size: Option<i64> = sqlx::query_scalar(
            r#"SELECT SUM(file_size) as size FROM media_files"#
        )
        .fetch_one(&self.pool)
        .await?;
        let total_size = total_size.unwrap_or(0);

        Ok(serde_json::json!({
            "total_files": total_count,
            "image_files": image_count,
            "video_files": video_count,
            "audio_files": audio_count,
            "total_size_bytes": total_size,
        }))
    }

    pub async fn insert_face(&self, face: &Face) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO faces (id, media_file_id, face_embedding, face_bbox, confidence, detected_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#)
        .bind(&face.id)
        .bind(&face.media_file_id)
        .bind(&face.face_embedding)
        .bind(&face.face_bbox)
        .bind(face.confidence)
        .bind(face.detected_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_sub_images(&self, parent_id: &str) -> Result<Vec<MediaFile>> {
        let sub_images = sqlx::query_as::<_, MediaFile>(
            r#"
            SELECT * FROM media_files 
            WHERE parent_id = ?1 AND is_sub_image = TRUE
            ORDER BY sub_image_index
            "#)
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(sub_images)
    }

    pub async fn get_parent_image(&self, sub_image_id: &str) -> Result<Option<MediaFile>> {
        let parent = sqlx::query_as::<_, MediaFile>(
            r#"
            SELECT parent.* 
            FROM media_files parent
            JOIN media_files child ON parent.id = child.parent_id
            WHERE child.id = ?1 AND child.is_sub_image = TRUE
            "#)
        .bind(sub_image_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(parent)
    }

    pub async fn delete_sub_images(&self, parent_id: &str) -> Result<()> {
        // First get all sub-images to delete their files
        let sub_images = self.get_sub_images(parent_id).await?;
        
        // Delete files from filesystem
        for sub_image in &sub_images {
            let path = std::path::Path::new(&sub_image.file_path);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    tracing::warn!("Failed to delete sub-image file {}: {}", sub_image.file_path, e);
                }
            }
        }
        
        // Delete from database
        sqlx::query(
            r#"
            DELETE FROM media_files 
            WHERE parent_id = ?1 AND is_sub_image = TRUE
            "#)
        .bind(parent_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_faces_for_media(&self, media_id: &str) -> Result<Vec<Face>> {
        let faces = sqlx::query_as::<_, Face>(
            r#"
            SELECT * FROM faces WHERE media_file_id = ?1
            "#)
        .bind(media_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(faces)
    }
    
    pub async fn get_face_by_id(&self, face_id: &str) -> Result<Option<Face>> {
        let face = sqlx::query_as::<_, Face>(
            r#"
            SELECT * FROM faces WHERE id = ?1
            "#)
        .bind(face_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(face)
    }

    pub async fn create_face_group(&self, group_name: Option<String>) -> Result<String> {
        let group_id = uuid::Uuid::new_v4().to_string();
        
        sqlx::query(
            r#"
            INSERT INTO face_groups (id, group_name, face_count, created_at, updated_at)
            VALUES (?1, ?2, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#)
        .bind(&group_id)
        .bind(group_name)
        .execute(&self.pool)
        .await?;

        Ok(group_id)
    }

    pub async fn add_face_to_group(&self, face_id: &str, group_id: &str, similarity_score: f32) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO face_group_members (face_id, group_id, similarity_score)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(face_id, group_id) DO UPDATE SET
                similarity_score = excluded.similarity_score
            "#)
        .bind(face_id)
        .bind(group_id)
        .bind(similarity_score)
        .execute(&self.pool)
        .await?;

        // Update face count in group
        sqlx::query(
            r#"
            UPDATE face_groups 
            SET face_count = (SELECT COUNT(*) FROM face_group_members WHERE group_id = ?1),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#)
        .bind(group_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_face_groups(&self) -> Result<Vec<FaceGroup>> {
        let groups = sqlx::query_as::<_, FaceGroup>(
            r#"
            SELECT * FROM face_groups
            ORDER BY face_count DESC
            "#)
        .fetch_all(&self.pool)
        .await?;

        Ok(groups)
    }

    pub async fn get_duplicates_by_hash(&self, file_hash: &str) -> Result<Vec<MediaFile>> {
        let files = sqlx::query_as::<_, MediaFile>(
            r#"
            SELECT * FROM media_files
            WHERE file_hash = ?1
            ORDER BY file_path
            "#)
        .bind(file_hash)
        .fetch_all(&self.pool)
        .await?;

        Ok(files)
    }

    pub async fn get_all_duplicate_hashes(&self) -> Result<Vec<String>> {
        let hashes: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT file_hash 
            FROM media_files
            GROUP BY file_hash
            HAVING COUNT(*) > 1
            "#)
        .fetch_all(&self.pool)
        .await?;

        Ok(hashes)
    }

    pub async fn update_thumbnail_path(&self, media_id: &str, thumbnail_path: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE media_files
            SET thumbnail_path = ?2,
                thumbnail_generated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#)
        .bind(media_id)
        .bind(thumbnail_path)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_faces_for_media(&self, media_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM faces WHERE media_file_id = ?1
            "#)
        .bind(media_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_face_groups_with_members(&self) -> Result<Vec<serde_json::Value>> {
        // First get all face groups
        let groups = self.get_face_groups().await?;
        
        let mut result = Vec::new();
        
        for group in groups {
            // Get faces in this group
            let faces_query = sqlx::query(
                r#"
                SELECT 
                    f.id as face_id,
                    f.media_file_id,
                    f.face_bbox,
                    f.confidence,
                    fgm.similarity_score,
                    mf.file_path,
                    mf.file_name
                FROM face_group_members fgm
                JOIN faces f ON f.id = fgm.face_id
                JOIN media_files mf ON mf.id = f.media_file_id
                WHERE fgm.group_id = ?1
                ORDER BY fgm.similarity_score DESC
                "#)
            .bind(&group.id)
            .fetch_all(&self.pool)
            .await?;
            
            let mut faces = Vec::new();
            for row in faces_query {
                use sqlx::Row;
                faces.push(serde_json::json!({
                    "face_id": row.get::<String, _>("face_id"),
                    "media_file_id": row.get::<String, _>("media_file_id"),
                    "face_bbox": row.get::<String, _>("face_bbox"),
                    "confidence": row.get::<f32, _>("confidence"),
                    "similarity_score": row.get::<Option<f32>, _>("similarity_score"),
                    "file_path": row.get::<String, _>("file_path"),
                    "file_name": row.get::<String, _>("file_name"),
                }));
            }
            
            result.push(serde_json::json!({
                "group_id": group.id,
                "group_name": group.group_name,
                "face_count": group.face_count,
                "created_at": group.created_at,
                "updated_at": group.updated_at,
                "faces": faces,
            }));
        }
        
        Ok(result)
    }

    pub fn get_pool(&self) -> Pool<Sqlite> {
        self.pool.clone()
    }
}