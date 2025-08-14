-- Add user-editable description and tags to media files
ALTER TABLE media_files ADD COLUMN user_description TEXT;
ALTER TABLE media_files ADD COLUMN user_tags TEXT; -- JSON array of tags

-- Create index for tag searching
CREATE INDEX IF NOT EXISTS idx_media_files_user_tags ON media_files(user_tags);