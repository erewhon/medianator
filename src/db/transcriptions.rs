use crate::models::{Transcription, TranscriptionSegment};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn create_transcription(
    pool: &SqlitePool,
    media_file_id: &str,
    text: &str,
    segments: &[TranscriptionSegment],
    language: Option<&str>,
    duration: Option<f64>,
    model: &str,
) -> Result<Transcription> {
    let id = Uuid::new_v4().to_string();
    let segments_json = serde_json::to_string(segments)?;
    let now = Utc::now();
    
    sqlx::query!(
        r#"
        INSERT INTO transcriptions (
            id, media_file_id, transcription_text, transcription_segments,
            language, duration_seconds, model_used, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
        id,
        media_file_id,
        text,
        segments_json,
        language,
        duration,
        model,
        now,
        now
    )
    .execute(pool)
    .await?;
    
    get_transcription(pool, &id).await
}

pub async fn get_transcription(pool: &SqlitePool, id: &str) -> Result<Transcription> {
    let row = sqlx::query!(
        r#"
        SELECT 
            id, media_file_id, transcription_text, transcription_segments,
            language, duration_seconds, model_used, created_at, updated_at
        FROM transcriptions
        WHERE id = ?1
        "#,
        id
    )
    .fetch_one(pool)
    .await?;
    
    Ok(Transcription {
        id: row.id.expect("transcription id should not be null"),
        media_file_id: row.media_file_id,
        transcription_text: row.transcription_text,
        transcription_segments: row.transcription_segments,
        language: row.language,
        duration_seconds: row.duration_seconds,
        model_used: row.model_used,
        created_at: row.created_at.map(|dt| dt.and_utc()).unwrap_or_else(Utc::now),
        updated_at: row.updated_at.map(|dt| dt.and_utc()).unwrap_or_else(Utc::now),
    })
}

pub async fn get_transcription_by_media(
    pool: &SqlitePool,
    media_file_id: &str,
) -> Result<Option<Transcription>> {
    let row = sqlx::query!(
        r#"
        SELECT 
            id, media_file_id, transcription_text, transcription_segments,
            language, duration_seconds, model_used, created_at, updated_at
        FROM transcriptions
        WHERE media_file_id = ?1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        media_file_id
    )
    .fetch_optional(pool)
    .await?;
    
    Ok(row.map(|r| Transcription {
        id: r.id.expect("transcription id should not be null"),
        media_file_id: r.media_file_id,
        transcription_text: r.transcription_text,
        transcription_segments: r.transcription_segments,
        language: r.language,
        duration_seconds: r.duration_seconds,
        model_used: r.model_used,
        created_at: r.created_at.map(|dt| dt.and_utc()).unwrap_or_else(Utc::now),
        updated_at: r.updated_at.map(|dt| dt.and_utc()).unwrap_or_else(Utc::now),
    }))
}

pub async fn update_transcription(
    pool: &SqlitePool,
    id: &str,
    text: &str,
    segments: &[TranscriptionSegment],
) -> Result<()> {
    let segments_json = serde_json::to_string(segments)?;
    let now = Utc::now();
    
    sqlx::query!(
        r#"
        UPDATE transcriptions
        SET transcription_text = ?2,
            transcription_segments = ?3,
            updated_at = ?4
        WHERE id = ?1
        "#,
        id,
        text,
        segments_json,
        now
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

pub async fn delete_transcription(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM transcriptions
        WHERE id = ?1
        "#,
        id
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

pub async fn search_transcriptions(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<(Transcription, String)>> {
    // Search using FTS and return transcriptions with matched media file IDs
    let results = sqlx::query!(
        r#"
        SELECT 
            t.id, t.media_file_id, t.transcription_text, t.transcription_segments,
            t.language, t.duration_seconds, t.model_used, t.created_at, t.updated_at,
            m.file_path
        FROM transcriptions_fts
        INNER JOIN transcriptions t ON transcriptions_fts.id = t.id
        INNER JOIN media_files m ON t.media_file_id = m.id
        WHERE transcriptions_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2 OFFSET ?3
        "#,
        query,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;
    
    let transcriptions = results
        .into_iter()
        .map(|r| {
            // Unwrap required fields safely
            let id = r.id.expect("transcription id should not be null");
            let media_file_id = r.media_file_id;
            let file_path = r.file_path;
            
            (
                Transcription {
                    id,
                    media_file_id,
                    transcription_text: r.transcription_text,
                    transcription_segments: r.transcription_segments,
                    language: r.language,
                    duration_seconds: r.duration_seconds,
                    model_used: r.model_used,
                    created_at: r.created_at.map(|dt| dt.and_utc()).unwrap_or_else(Utc::now),
                    updated_at: r.updated_at.map(|dt| dt.and_utc()).unwrap_or_else(Utc::now),
                },
                file_path,
            )
        })
        .collect();
    
    Ok(transcriptions)
}