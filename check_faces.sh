#!/bin/bash

# Check face detection status in the database

echo "Checking face detection status..."
echo ""

# Check if database exists
if [ ! -f "medianator.db" ]; then
    echo "ERROR: Database file 'medianator.db' not found!"
    echo "Please run the server first to create the database."
    exit 1
fi

echo "=== Database Tables ==="
sqlite3 medianator.db "SELECT name FROM sqlite_master WHERE type='table';"

echo ""
echo "=== Face Detection Statistics ==="
echo "Total faces detected:"
sqlite3 medianator.db "SELECT COUNT(*) FROM faces;"

echo ""
echo "Faces by media file:"
sqlite3 medianator.db "SELECT media_file_id, COUNT(*) as face_count FROM faces GROUP BY media_file_id;"

echo ""
echo "=== Face Groups ==="
echo "Total face groups:"
sqlite3 medianator.db "SELECT COUNT(*) FROM face_groups;"

echo ""
echo "Face groups with members:"
sqlite3 medianator.db "
SELECT fg.id, fg.group_name, fg.face_count
FROM face_groups fg
WHERE fg.face_count > 0;"

echo ""
echo "=== Recent Faces ==="
sqlite3 medianator.db "
SELECT f.id, f.media_file_id, f.confidence, f.detected_at
FROM faces f
ORDER BY f.detected_at DESC
LIMIT 10;"

echo ""
echo "=== Media Files with Faces ==="
sqlite3 medianator.db "
SELECT DISTINCT mf.file_name, mf.file_path
FROM media_files mf
JOIN faces f ON f.media_file_id = mf.id
LIMIT 10;"