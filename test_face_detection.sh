#!/bin/bash

# Test face detection functionality

echo "Setting up test environment for face detection..."

# Set environment variables
export DATABASE_URL="sqlite://medianator.db"
export THUMBNAILS_DIR="./thumbnails"
export ENABLE_FACE_DETECTION="true"

# Create thumbnails directory if it doesn't exist
mkdir -p thumbnails

echo "Starting Medianator with face detection enabled..."
echo "The simple face detector will:"
echo "  - Detect skin-tone regions in images"
echo "  - Create face embeddings for detected regions"
echo "  - Store faces in the database for grouping"
echo ""
echo "To test:"
echo "1. Start the server: cargo run --release"
echo "2. Upload images via the web UI at http://localhost:3000"
echo "3. Check the faces API endpoint: http://localhost:3000/api/faces/{media_id}"
echo ""
echo "Starting server..."

cargo run --release