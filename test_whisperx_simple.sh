#!/bin/bash

# Simple WhisperX functionality test
echo "Testing WhisperX functionality..."
echo "================================="
echo ""

# Create a test audio file using ffmpeg (1 second of silence)
TEST_FILE="/tmp/test_audio.wav"
echo "Creating test audio file..."
ffmpeg -f lavfi -i anullsrc=r=44100:cl=mono -t 1 "$TEST_FILE" -y 2>/dev/null

if [ ! -f "$TEST_FILE" ]; then
    echo "❌ Failed to create test audio file"
    exit 1
fi

echo "✅ Created test audio at: $TEST_FILE"
echo ""

# Create temp directory for output
TEMP_DIR=$(mktemp -d)
echo "Output directory: $TEMP_DIR"
echo ""

# Test 1: Direct whisperx execution
echo "Test 1: Direct whisperx execution"
echo "----------------------------------"
whisperx "$TEST_FILE" \
    --model tiny \
    --output_dir "$TEMP_DIR" \
    --output_format json \
    --compute_type int8 \
    --language en \
    2>&1 | tee "$TEMP_DIR/direct_test.log"

if [ $? -eq 0 ]; then
    echo "✅ Direct execution succeeded"
    echo "Output files:"
    ls -la "$TEMP_DIR"/*.json 2>/dev/null || echo "   No JSON files found"
else
    echo "❌ Direct execution failed"
    echo "Error log:"
    tail -20 "$TEMP_DIR/direct_test.log"
fi

echo ""
echo "Test 2: WhisperX with minimal options"
echo "--------------------------------------"
whisperx "$TEST_FILE" --model tiny 2>&1 | head -10

echo ""
echo "Cleaning up..."
rm -f "$TEST_FILE"
rm -rf "$TEMP_DIR"

echo ""
echo "Done!"