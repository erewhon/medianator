use anyhow::Result;
use sqlx::SqlitePool;
use uuid::Uuid;
use chrono::Utc;

use crate::models::{Story, StoryItem, StoryWithItems, MediaFile};

pub struct StoryDatabase {
    pool: SqlitePool,
}

impl StoryDatabase {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_story(&self, name: &str, description: Option<&str>) -> Result<Story> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT INTO stories (id, name, description, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#
        )
        .bind(&id)
        .bind(name)
        .bind(description)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        
        Ok(Story {
            id: id.clone(),
            name: name.to_string(),
            description: description.map(String::from),
            cover_image_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_story(&self, story_id: &str) -> Result<Option<Story>> {
        let story = sqlx::query_as::<_, Story>(
            r#"
            SELECT * FROM stories WHERE id = ?1
            "#
        )
        .bind(story_id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(story)
    }

    pub async fn get_all_stories(&self) -> Result<Vec<Story>> {
        let stories = sqlx::query_as::<_, Story>(
            r#"
            SELECT * FROM stories
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(stories)
    }

    pub async fn get_story_with_items(&self, story_id: &str) -> Result<Option<StoryWithItems>> {
        let story = self.get_story(story_id).await?;
        
        if let Some(story) = story {
            let items = sqlx::query_as::<_, MediaFile>(
                r#"
                SELECT m.* FROM media_files m
                JOIN story_items si ON m.id = si.media_file_id
                WHERE si.story_id = ?1
                ORDER BY si.position, si.added_at
                "#
            )
            .bind(story_id)
            .fetch_all(&self.pool)
            .await?;
            
            Ok(Some(StoryWithItems {
                item_count: items.len(),
                story,
                items,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn add_item_to_story(&self, story_id: &str, media_file_id: &str, caption: Option<&str>) -> Result<()> {
        // Get the next position
        let position: i32 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(position), -1) + 1
            FROM story_items
            WHERE story_id = ?1
            "#
        )
        .bind(story_id)
        .fetch_one(&self.pool)
        .await?;
        
        sqlx::query(
            r#"
            INSERT INTO story_items (story_id, media_file_id, position, caption, added_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(story_id, media_file_id) DO UPDATE SET
                caption = excluded.caption,
                position = excluded.position
            "#
        )
        .bind(story_id)
        .bind(media_file_id)
        .bind(position)
        .bind(caption)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        
        // Update story's updated_at
        sqlx::query(
            r#"
            UPDATE stories SET updated_at = ?1 WHERE id = ?2
            "#
        )
        .bind(Utc::now())
        .bind(story_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn remove_item_from_story(&self, story_id: &str, media_file_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM story_items
            WHERE story_id = ?1 AND media_file_id = ?2
            "#
        )
        .bind(story_id)
        .bind(media_file_id)
        .execute(&self.pool)
        .await?;
        
        // Update story's updated_at
        sqlx::query(
            r#"
            UPDATE stories SET updated_at = ?1 WHERE id = ?2
            "#
        )
        .bind(Utc::now())
        .bind(story_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn delete_story(&self, story_id: &str) -> Result<()> {
        // Items will be deleted automatically due to CASCADE
        sqlx::query(
            r#"
            DELETE FROM stories WHERE id = ?1
            "#
        )
        .bind(story_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn update_story(&self, story_id: &str, name: &str, description: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE stories 
            SET name = ?1, description = ?2, updated_at = ?3
            WHERE id = ?4
            "#
        )
        .bind(name)
        .bind(description)
        .bind(Utc::now())
        .bind(story_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}