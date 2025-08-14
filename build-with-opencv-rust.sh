#!/bin/bash
# Build script for OpenCV Rust face detection feature

echo "Building Medianator with OpenCV Rust face detection..."

# Set environment variables for OpenCV
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
export LIBCLANG_PATH="/opt/homebrew/opt/llvm/lib"

# Build with the opencv-face feature
env DATABASE_URL=sqlite://medianator.db \
    DYLD_FALLBACK_LIBRARY_PATH="$(xcode-select --print-path)/Toolchains/XcodeDefault.xctoolchain/usr/lib/" \
    cargo build --features opencv-face

if [ $? -eq 0 ]; then
    echo "Build successful! You can now run with OpenCV Rust face detection:"
    echo ""
    echo "ENABLE_FACE_DETECTION=true FACE_DETECTOR_TYPE=opencv-rust DATABASE_URL=sqlite://medianator.db ./target/debug/medianator"
else
    echo "Build failed. Make sure OpenCV is installed:"
    echo "  brew install opencv"
    echo ""
    echo "Also ensure the Haar cascade file exists in models/ directory."
fi