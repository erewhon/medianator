-- Create stories table for grouping media files
CREATE TABLE IF NOT EXISTS stories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    cover_image_id TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (cover_image_id) REFERENCES media_files(id) ON DELETE SET NULL
);

-- Create story_items table for many-to-many relationship
CREATE TABLE IF NOT EXISTS story_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    story_id TEXT NOT NULL,
    media_file_id TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    caption TEXT,
    added_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE,
    UNIQUE(story_id, media_file_id)
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_story_items_story_id ON story_items(story_id);
CREATE INDEX IF NOT EXISTS idx_story_items_media_file_id ON story_items(media_file_id);
CREATE INDEX IF NOT EXISTS idx_story_items_position ON story_items(story_id, position);
CREATE INDEX IF NOT EXISTS idx_stories_created_at ON stories(created_at DESC);