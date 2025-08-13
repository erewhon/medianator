-- Add thumbnail support
ALTER TABLE media_files ADD COLUMN thumbnail_path TEXT;
ALTER TABLE media_files ADD COLUMN thumbnail_generated_at DATETIME;

-- Create duplicates table for tracking duplicate files
CREATE TABLE IF NOT EXISTS duplicates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_hash TEXT NOT NULL,
    file_paths TEXT NOT NULL, -- JSON array of file paths
    file_count INTEGER NOT NULL,
    total_size INTEGER NOT NULL,
    first_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_duplicates_hash ON duplicates(file_hash);
CREATE INDEX idx_duplicates_count ON duplicates(file_count);

-- Create faces table for face recognition
CREATE TABLE IF NOT EXISTS faces (
    id TEXT PRIMARY KEY NOT NULL,
    media_file_id TEXT NOT NULL,
    face_embedding TEXT NOT NULL, -- Serialized face embedding vector
    face_bbox TEXT NOT NULL, -- JSON: {x, y, width, height}
    confidence REAL NOT NULL,
    detected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE
);

CREATE INDEX idx_faces_media_file ON faces(media_file_id);

-- Create face groups table for grouping similar faces
CREATE TABLE IF NOT EXISTS face_groups (
    id TEXT PRIMARY KEY NOT NULL,
    group_name TEXT,
    face_count INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create face group members table
CREATE TABLE IF NOT EXISTS face_group_members (
    face_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    similarity_score REAL NOT NULL,
    added_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (face_id, group_id),
    FOREIGN KEY (face_id) REFERENCES faces(id) ON DELETE CASCADE,
    FOREIGN KEY (group_id) REFERENCES face_groups(id) ON DELETE CASCADE
);

CREATE INDEX idx_face_group_members_group ON face_group_members(group_id);