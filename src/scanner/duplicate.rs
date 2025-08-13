use anyhow::Result;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use tracing::{debug, info};

use crate::models::{DuplicateGroup, DuplicateFile, MediaFile};

pub struct DuplicateDetector {
    pool: Pool<Sqlite>,
}

impl DuplicateDetector {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub async fn find_all_duplicates(&self) -> Result<Vec<DuplicateGroup>> {
        // Query for files with duplicate hashes
        let query = r#"
            SELECT 
                file_hash,
                GROUP_CONCAT(id, '||') as ids,
                GROUP_CONCAT(file_path, '||') as paths,
                GROUP_CONCAT(file_size, '||') as sizes,
                GROUP_CONCAT(file_modified_at, '||') as modified_dates,
                COUNT(*) as count,
                SUM(file_size) as total_size
            FROM media_files
            GROUP BY file_hash
            HAVING COUNT(*) > 1
            ORDER BY total_size DESC
        "#;

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await?;

        let mut duplicate_groups = Vec::new();

        for row in rows {
            use sqlx::Row;
            let file_hash: String = row.get("file_hash");
            let ids: Option<String> = row.get("ids");
            let paths: Option<String> = row.get("paths");
            let sizes: Option<String> = row.get("sizes");
            let modified_dates: Option<String> = row.get("modified_dates");
            let count: i64 = row.get("count");
            let total_size: Option<i64> = row.get("total_size");
            
            let ids: Vec<&str> = ids.as_ref().map_or(vec![], |s| s.split("||").collect());
            let paths: Vec<&str> = paths.as_ref().map_or(vec![], |s| s.split("||").collect());
            let sizes: Vec<&str> = sizes.as_ref().map_or(vec![], |s| s.split("||").collect());
            let modified: Vec<&str> = modified_dates.as_ref().map_or(vec![], |s| s.split("||").collect());

            let mut files = Vec::new();
            for i in 0..ids.len() {
                files.push(DuplicateFile {
                    id: ids.get(i).unwrap_or(&"").to_string(),
                    path: paths.get(i).unwrap_or(&"").to_string(),
                    size: sizes.get(i).and_then(|s| s.parse().ok()).unwrap_or(0),
                    modified_at: modified.get(i)
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc)),
                });
            }

            duplicate_groups.push(DuplicateGroup {
                hash: file_hash,
                files,
                total_size: total_size.unwrap_or(0),
                count: count as usize,
            });
        }

        info!("Found {} groups of duplicate files", duplicate_groups.len());
        Ok(duplicate_groups)
    }

    pub async fn find_duplicates_for_hash(&self, file_hash: &str) -> Result<Vec<MediaFile>> {
        let query = r#"
            SELECT * FROM media_files
            WHERE file_hash = ?
            ORDER BY file_path
        "#;

        let files = sqlx::query_as::<_, MediaFile>(query)
            .bind(file_hash)
            .fetch_all(&self.pool)
            .await?;

        Ok(files)
    }

    pub async fn update_duplicates_table(&self) -> Result<()> {
        // Clear existing duplicates table
        sqlx::query("DELETE FROM duplicates")
            .execute(&self.pool)
            .await?;

        // Insert new duplicate records
        let query = r#"
            INSERT INTO duplicates (file_hash, file_paths, file_count, total_size)
            SELECT 
                file_hash,
                json_group_array(file_path) as file_paths,
                COUNT(*) as file_count,
                SUM(file_size) as total_size
            FROM media_files
            GROUP BY file_hash
            HAVING COUNT(*) > 1
        "#;

        let result = sqlx::query(query)
            .execute(&self.pool)
            .await?;

        info!("Updated duplicates table with {} entries", result.rows_affected());
        Ok(())
    }

    pub async fn get_duplicate_stats(&self) -> Result<DuplicateStats> {
        let query = r#"
            SELECT 
                COUNT(DISTINCT file_hash) as duplicate_groups,
                SUM(file_count - 1) as redundant_files,
                SUM((file_count - 1) * (total_size / file_count)) as wasted_space
            FROM duplicates
        "#;

        let row = sqlx::query(query)
            .fetch_one(&self.pool)
            .await?;

        use sqlx::Row;
        let duplicate_groups: Option<i64> = row.get("duplicate_groups");
        let redundant_files: Option<i64> = row.get("redundant_files");
        let wasted_space: Option<i64> = row.get("wasted_space");
        
        Ok(DuplicateStats {
            duplicate_groups: duplicate_groups.unwrap_or(0) as usize,
            redundant_files: redundant_files.unwrap_or(0) as usize,
            wasted_space: wasted_space.unwrap_or(0),
        })
    }

    pub async fn suggest_files_to_remove(&self, keep_newest: bool) -> Result<Vec<String>> {
        let duplicate_groups = self.find_all_duplicates().await?;
        let mut files_to_remove = Vec::new();

        for group in duplicate_groups {
            if group.files.len() < 2 {
                continue;
            }

            // Sort files by modification date
            let mut sorted_files = group.files.clone();
            sorted_files.sort_by(|a, b| {
                let a_time = a.modified_at.as_ref();
                let b_time = b.modified_at.as_ref();
                
                match (a_time, b_time) {
                    (Some(a), Some(b)) => {
                        if keep_newest {
                            b.cmp(a) // Newest first
                        } else {
                            a.cmp(b) // Oldest first
                        }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });

            // Keep the first file, suggest removing the rest
            for file in sorted_files.iter().skip(1) {
                files_to_remove.push(file.path.clone());
            }
        }

        info!("Suggested {} files for removal", files_to_remove.len());
        Ok(files_to_remove)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateStats {
    pub duplicate_groups: usize,
    pub redundant_files: usize,
    pub wasted_space: i64,
}

impl DuplicateStats {
    pub fn wasted_space_human_readable(&self) -> String {
        let bytes = self.wasted_space;
        let units = ["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < units.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.2} {}", size, units[unit_index])
    }
}