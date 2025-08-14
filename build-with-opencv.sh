#!/bin/bash
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
export LIBCLANG_PATH="/opt/homebrew/opt/llvm/lib"
env DATABASE_URL=sqlite://medianator.db \
    DYLD_FALLBACK_LIBRARY_PATH="$(xcode-select --print-path)/Toolchains/XcodeDefault.xctoolchain/usr/lib/" \
    cargo build --features opencv-face
