-- Add transcription support for audio and video files
CREATE TABLE IF NOT EXISTS transcriptions (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    media_file_id TEXT NOT NULL,
    transcription_text TEXT,
    transcription_segments TEXT, -- JSON array of segments with timestamps and speaker tags
    language TEXT,
    duration_seconds REAL,
    model_used TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE
);

-- Add index for media file lookups
CREATE INDEX IF NOT EXISTS idx_transcriptions_media_file ON transcriptions(media_file_id);

-- Add full-text search for transcriptions
CREATE VIRTUAL TABLE IF NOT EXISTS transcriptions_fts USING fts5(
    id UNINDEXED,
    media_file_id UNINDEXED,
    transcription_text,
    content=transcriptions,
    content_rowid=rowid
);

-- Triggers to keep FTS index in sync
CREATE TRIGGER IF NOT EXISTS transcriptions_ai AFTER INSERT ON transcriptions 
BEGIN
    INSERT INTO transcriptions_fts(id, media_file_id, transcription_text)
    VALUES (new.id, new.media_file_id, new.transcription_text);
END;

CREATE TRIGGER IF NOT EXISTS transcriptions_au AFTER UPDATE ON transcriptions 
BEGIN
    UPDATE transcriptions_fts 
    SET transcription_text = new.transcription_text
    WHERE id = new.id;
END;

CREATE TRIGGER IF NOT EXISTS transcriptions_ad AFTER DELETE ON transcriptions 
BEGIN
    DELETE FROM transcriptions_fts WHERE id = old.id;
END;