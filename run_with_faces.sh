#!/bin/bash

# Run Medianator with face detection enabled

echo "Starting Medianator with face detection enabled..."
echo "Make sure you have some images to test with."
echo ""

# Set environment variables
export DATABASE_URL="sqlite://medianator.db"
export THUMBNAILS_DIR="./thumbnails"
export ENABLE_FACE_DETECTION="true"
export RUST_LOG="medianator=debug,tower_http=info"

# Create directories if they don't exist
mkdir -p thumbnails
mkdir -p uploads

echo "Configuration:"
echo "  Database: $DATABASE_URL"
echo "  Thumbnails: $THUMBNAILS_DIR"
echo "  Face Detection: ENABLED"
echo "  Log Level: DEBUG"
echo ""
echo "Server will start on http://localhost:3000"
echo ""
echo "To test face detection:"
echo "1. Upload images through the web UI"
echo "2. Or click 'Batch Reprocess' and select images"
echo "3. Check console logs for face detection output"
echo "4. Click 'View Faces' to see detected faces"
echo ""

# Run the server
cargo run --release