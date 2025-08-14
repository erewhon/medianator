use anyhow::Result;
use tracing::{info, debug};
use crate::db::Database;
use crate::scanner::face_recognition::{base64_decode, calculate_face_similarity};

impl Database {
    /// Automatically group faces based on similarity
    pub async fn auto_group_faces(&self) -> Result<()> {
        info!("Starting automatic face grouping");
        
        // Get all faces that aren't in any group yet
        let ungrouped_faces = sqlx::query!(
            r#"
            SELECT f.id, f.face_embedding
            FROM faces f
            WHERE NOT EXISTS (
                SELECT 1 FROM face_group_members fgm 
                WHERE fgm.face_id = f.id
            )
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        info!("Found {} ungrouped faces", ungrouped_faces.len());
        
        if ungrouped_faces.is_empty() {
            return Ok(());
        }
        
        // Get existing face groups with their representative face
        let existing_groups = sqlx::query!(
            r#"
            SELECT 
                fg.id as group_id,
                f.face_embedding as representative_embedding
            FROM face_groups fg
            JOIN face_group_members fgm ON fgm.group_id = fg.id
            JOIN faces f ON f.id = fgm.face_id
            WHERE fgm.similarity_score = (
                SELECT MAX(similarity_score) 
                FROM face_group_members 
                WHERE group_id = fg.id
            )
            GROUP BY fg.id
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        debug!("Found {} existing face groups", existing_groups.len());
        
        const SIMILARITY_THRESHOLD: f32 = 0.7;
        
        // Process each ungrouped face
        for face in ungrouped_faces {
            let face_embedding = base64_decode(&face.face_embedding)?;
            let mut best_group = None;
            let mut best_similarity = 0.0f32;
            
            // Check similarity with existing groups
            for group in &existing_groups {
                let group_embedding = base64_decode(&group.representative_embedding)?;
                let similarity = calculate_face_similarity(&face_embedding, &group_embedding);
                
                if similarity > best_similarity && similarity >= SIMILARITY_THRESHOLD {
                    best_similarity = similarity;
                    best_group = Some(group.group_id.clone());
                }
            }
            
            // If a similar group was found, add face to it
            if let Some(group_id) = best_group {
                debug!("Adding face {} to existing group {} with similarity {}", 
                    face.id, group_id, best_similarity);
                
                self.add_face_to_group(&face.id, &group_id, best_similarity).await?;
            } else {
                // Create a new group for this face
                debug!("Creating new group for face {}", face.id);
                
                let group_id = self.create_face_group(None).await?;
                self.add_face_to_group(&face.id, &group_id, 1.0).await?;
            }
        }
        
        // Now check if any faces within groups are similar to faces in other groups
        // This could merge groups that should be together
        self.merge_similar_groups(SIMILARITY_THRESHOLD).await?;
        
        info!("Automatic face grouping completed");
        Ok(())
    }
    
    /// Merge groups that have similar faces
    async fn merge_similar_groups(&self, threshold: f32) -> Result<()> {
        let groups = sqlx::query!(
            r#"
            SELECT DISTINCT
                fg1.id as group1_id,
                fg2.id as group2_id,
                f1.face_embedding as embedding1,
                f2.face_embedding as embedding2
            FROM face_groups fg1
            JOIN face_group_members fgm1 ON fgm1.group_id = fg1.id
            JOIN faces f1 ON f1.id = fgm1.face_id
            CROSS JOIN face_groups fg2
            JOIN face_group_members fgm2 ON fgm2.group_id = fg2.id
            JOIN faces f2 ON f2.id = fgm2.face_id
            WHERE fg1.id < fg2.id  -- Avoid duplicate comparisons
            LIMIT 1000  -- Limit to prevent excessive comparisons
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut groups_to_merge = Vec::new();
        
        for group_pair in groups {
            let embedding1 = base64_decode(&group_pair.embedding1)?;
            let embedding2 = base64_decode(&group_pair.embedding2)?;
            let similarity = calculate_face_similarity(&embedding1, &embedding2);
            
            if similarity >= threshold {
                groups_to_merge.push((group_pair.group1_id, group_pair.group2_id, similarity));
            }
        }
        
        // Merge similar groups
        for (group1_id, group2_id, similarity) in groups_to_merge {
            debug!("Merging groups {} and {} with similarity {}", 
                group1_id, group2_id, similarity);
            
            // Move all faces from group2 to group1
            sqlx::query!(
                r#"
                UPDATE face_group_members 
                SET group_id = ?1
                WHERE group_id = ?2
                "#,
                group1_id,
                group2_id
            )
            .execute(&self.pool)
            .await?;
            
            // Delete the empty group
            sqlx::query!(
                "DELETE FROM face_groups WHERE id = ?",
                group2_id
            )
            .execute(&self.pool)
            .await?;
            
            // Update face count for the merged group
            sqlx::query!(
                r#"
                UPDATE face_groups 
                SET face_count = (SELECT COUNT(*) FROM face_group_members WHERE group_id = ?1),
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?1
                "#,
                group1_id
            )
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }
}