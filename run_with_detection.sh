#!/bin/bash

# Run Medianator with all detection features enabled

echo "🎯 Starting Medianator with Detection Features"
echo "=============================================="
echo ""

# Check for required tools
echo "Checking dependencies..."

# Check FFmpeg (required for scene detection)
if command -v ffmpeg &> /dev/null; then
    echo "✅ FFmpeg is available"
else
    echo "❌ FFmpeg not found - scene detection may not work"
fi

# Check PySceneDetect (optional, enhances scene detection)
if command -v scenedetect &> /dev/null || pipx list 2>/dev/null | grep -q scenedetect; then
    echo "✅ PySceneDetect is available (enhanced scene detection)"
else
    echo "⚠️  PySceneDetect not found - using basic scene detection"
fi

# Check Tesseract (for text detection)
if command -v tesseract &> /dev/null; then
    echo "✅ Tesseract OCR is available (text detection enabled)"
else
    echo "⚠️  Tesseract not found - text detection disabled"
fi

# Check ImageMagick (for color extraction)
if command -v convert &> /dev/null; then
    echo "✅ ImageMagick is available (color extraction enabled)"
else
    echo "⚠️  ImageMagick not found - color extraction disabled"
fi

echo ""
echo "Building release version..."
DATABASE_URL="sqlite://medianator.db" cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo ""
echo "Starting Medianator with detection features..."
echo "Server will be available at: http://localhost:3000"
echo ""

# Set environment variables for detection features
export ENABLE_SCENE_DETECTION=true
export ENABLE_OBJECT_DETECTION=true
export ENABLE_FACE_DETECTION=true
export FACE_DETECTION_MODEL="opencv-rust"  # or "viola-jones" or "opencv-python"
export DATABASE_URL="sqlite://medianator.db"
export RUST_LOG=medianator=info

# Run with all features
./target/release/medianator