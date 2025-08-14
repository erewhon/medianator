-- Add parent_id column to media_files for parent-child relationships
ALTER TABLE media_files ADD COLUMN parent_id TEXT;
ALTER TABLE media_files ADD COLUMN is_sub_image BOOLEAN DEFAULT FALSE;
ALTER TABLE media_files ADD COLUMN sub_image_index INTEGER;
ALTER TABLE media_files ADD COLUMN extraction_metadata TEXT; -- JSON with extraction details (e.g., region coordinates)

-- Add foreign key constraint
CREATE INDEX idx_media_files_parent_id ON media_files(parent_id);

-- Create a view for easy querying of sub-images
CREATE VIEW sub_images_view AS
SELECT 
    child.id AS sub_image_id,
    child.file_path AS sub_image_path,
    child.sub_image_index,
    child.extraction_metadata,
    parent.id AS parent_id,
    parent.file_path AS parent_path,
    parent.file_name AS parent_name
FROM media_files child
JOIN media_files parent ON child.parent_id = parent.id
WHERE child.is_sub_image = TRUE;