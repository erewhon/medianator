-- Create media files table
CREATE TABLE IF NOT EXISTS media_files (
    id TEXT PRIMARY KEY NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    file_hash TEXT NOT NULL,
    media_type TEXT NOT NULL CHECK(media_type IN ('image', 'video', 'audio')),
    mime_type TEXT NOT NULL,
    
    -- Common metadata
    width INTEGER,
    height INTEGER,
    duration_seconds REAL,
    bit_rate INTEGER,
    
    -- Image specific
    camera_make TEXT,
    camera_model TEXT,
    lens_model TEXT,
    focal_length REAL,
    aperture REAL,
    iso INTEGER,
    shutter_speed TEXT,
    orientation INTEGER,
    
    -- Video/Audio specific
    codec TEXT,
    frame_rate REAL,
    audio_channels INTEGER,
    audio_sample_rate INTEGER,
    
    -- Timestamps
    file_created_at DATETIME,
    file_modified_at DATETIME,
    indexed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_scanned_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- Additional metadata as JSON
    extra_metadata TEXT
);

-- Create indexes for faster queries
CREATE INDEX idx_media_files_type ON media_files(media_type);
CREATE INDEX idx_media_files_mime ON media_files(mime_type);
CREATE INDEX idx_media_files_hash ON media_files(file_hash);
CREATE INDEX idx_media_files_path ON media_files(file_path);
CREATE INDEX idx_media_files_indexed ON media_files(indexed_at);

-- Create scan history table
CREATE TABLE IF NOT EXISTS scan_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_path TEXT NOT NULL,
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    files_scanned INTEGER DEFAULT 0,
    files_added INTEGER DEFAULT 0,
    files_updated INTEGER DEFAULT 0,
    files_removed INTEGER DEFAULT 0,
    error_count INTEGER DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'running' CHECK(status IN ('running', 'completed', 'failed'))
);