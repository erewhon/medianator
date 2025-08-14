#!/bin/bash
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
export LIBCLANG_PATH="/opt/homebrew/opt/llvm/lib"
env DATABASE_URL=sqlite://medianator.db \
    DYLD_FALLBACK_LIBRARY_PATH="$(xcode-select --print-path)/Toolchains/XcodeDefault.xctoolchain/usr/lib/" \
    ENABLE_FACE_DETECTION=true \
    FACE_DETECTOR_TYPE=opencv-rust \
    THUMBNAILS=./thumbnails \
    SUB_IMAGES_DIR=./sub_images \
    USE_OPENCV_COLLAGE=true \
    cargo run --features opencv-face
