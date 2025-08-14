#!/bin/bash

echo "Testing OpenCV collage detection integration..."

# Clean up previous test
rm -rf test_sub_images/
mkdir -p test_sub_images

# Build with OpenCV
echo "Building with OpenCV support..."
./build-with-opencv.sh > /dev/null 2>&1

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

# Run medianator with OpenCV collage detection enabled
echo "Running medianator with OpenCV collage detection..."
env DATABASE_URL=sqlite://test_medianator.db \
    SUB_IMAGES_DIR=test_sub_images \
    USE_OPENCV_COLLAGE=true \
    RUST_LOG=medianator=info \
    timeout 10 ./target/debug/medianator 2>&1 | grep -E "Sub-image|OpenCV|collage" &

SCANNER_PID=$!

# Wait a moment for server to start
sleep 2

# Trigger scan of test image
echo "Triggering scan of pexels image..."
curl -X POST "http://localhost:3000/api/scan" \
    -H "Content-Type: application/json" \
    -d '{"path": "test_images/pexels-fotios-photos-3024995.jpg"}' \
    2>/dev/null

# Wait for scan to complete
sleep 5

# Kill the server
kill $SCANNER_PID 2>/dev/null

# Check results
echo ""
echo "Checking extracted sub-images..."
if [ -d "test_sub_images" ]; then
    count=$(ls -1 test_sub_images/*.jpg 2>/dev/null | wc -l)
    echo "Found $count extracted sub-images"
    
    if [ $count -gt 0 ]; then
        echo "First 5 sub-images:"
        ls -1 test_sub_images/*.jpg 2>/dev/null | head -5
    fi
else
    echo "No sub-images directory found"
fi

echo ""
echo "Test complete!"