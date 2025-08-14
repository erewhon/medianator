#!/bin/bash

# Test script for sub-image extraction feature

echo "Testing sub-image extraction feature..."

# Create directories for test output
mkdir -p test_sub_images
mkdir -p test_thumbnails

# Set environment variables
export DATABASE_URL="sqlite://test_medianator.db"
export THUMBNAILS_DIR="test_thumbnails"
export SUB_IMAGES_DIR="test_sub_images"
export ENABLE_FACE_DETECTION="true"
export USE_OPENCV="true"
export RUST_LOG="medianator=debug"

# Remove old test database if exists
rm -f test_medianator.db

# Run migrations
echo "Running migrations..."
sqlx database create
sqlx migrate run

echo "Starting Medianator with sub-image extraction enabled..."
echo "Sub-images will be extracted to: $SUB_IMAGES_DIR"
echo ""
echo "To test:"
echo "1. Start the server with: ./target/release/medianator"
echo "2. Upload or scan images that contain multiple sub-images (like photo album pages)"
echo "3. Check the API endpoints:"
echo "   - GET /api/media/{id}/sub-images - Get sub-images for a parent image"
echo "   - GET /api/sub-images/{id}/parent - Get parent image for a sub-image"
echo ""
echo "The extracted sub-images will be saved in: $SUB_IMAGES_DIR"
echo "Face detection will run on both parent and sub-images"