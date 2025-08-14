-- Add GPS location fields to media_files
ALTER TABLE media_files ADD COLUMN latitude REAL;
ALTER TABLE media_files ADD COLUMN longitude REAL;
ALTER TABLE media_files ADD COLUMN altitude REAL;
ALTER TABLE media_files ADD COLUMN location_name TEXT;
ALTER TABLE media_files ADD COLUMN date_taken DATETIME;

-- Create indexes for location and date grouping
CREATE INDEX idx_media_files_latitude ON media_files(latitude);
CREATE INDEX idx_media_files_longitude ON media_files(longitude);
CREATE INDEX idx_media_files_date_taken ON media_files(date_taken);

-- Create media groups table for date/location grouping
CREATE TABLE IF NOT EXISTS media_groups (
    id TEXT PRIMARY KEY NOT NULL,
    group_type TEXT NOT NULL CHECK(group_type IN ('date', 'location', 'event')),
    group_name TEXT NOT NULL,
    group_date DATE,
    latitude REAL,
    longitude REAL,
    location_name TEXT,
    media_count INTEGER NOT NULL DEFAULT 0,
    total_size INTEGER NOT NULL DEFAULT 0,
    cover_media_id TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (cover_media_id) REFERENCES media_files(id) ON DELETE SET NULL
);

CREATE INDEX idx_media_groups_type ON media_groups(group_type);
CREATE INDEX idx_media_groups_date ON media_groups(group_date);
CREATE INDEX idx_media_groups_location ON media_groups(latitude, longitude);

-- Create media group members table
CREATE TABLE IF NOT EXISTS media_group_members (
    media_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    added_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (media_id, group_id),
    FOREIGN KEY (media_id) REFERENCES media_files(id) ON DELETE CASCADE,
    FOREIGN KEY (group_id) REFERENCES media_groups(id) ON DELETE CASCADE
);

CREATE INDEX idx_media_group_members_group ON media_group_members(group_id);
CREATE INDEX idx_media_group_members_media ON media_group_members(media_id);

-- Create smart albums table
CREATE TABLE IF NOT EXISTS smart_albums (
    id TEXT PRIMARY KEY NOT NULL,
    album_name TEXT NOT NULL,
    description TEXT,
    filter_rules TEXT NOT NULL, -- JSON with filter criteria
    sort_order TEXT DEFAULT 'date_desc',
    media_count INTEGER NOT NULL DEFAULT 0,
    cover_media_id TEXT,
    is_public BOOLEAN DEFAULT FALSE,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_refreshed_at DATETIME,
    FOREIGN KEY (cover_media_id) REFERENCES media_files(id) ON DELETE SET NULL
);

CREATE INDEX idx_smart_albums_name ON smart_albums(album_name);
CREATE INDEX idx_smart_albums_public ON smart_albums(is_public);

-- Create smart album members table (cached results)
CREATE TABLE IF NOT EXISTS smart_album_members (
    album_id TEXT NOT NULL,
    media_id TEXT NOT NULL,
    match_score REAL DEFAULT 1.0,
    added_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (album_id, media_id),
    FOREIGN KEY (album_id) REFERENCES smart_albums(id) ON DELETE CASCADE,
    FOREIGN KEY (media_id) REFERENCES media_files(id) ON DELETE CASCADE
);

CREATE INDEX idx_smart_album_members_album ON smart_album_members(album_id);
CREATE INDEX idx_smart_album_members_media ON smart_album_members(media_id);