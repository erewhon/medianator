#!/bin/bash

# WhisperX Diagnostic Script
# This script tests WhisperX installation and helps diagnose issues

echo "🔍 WhisperX Diagnostic Script"
echo "============================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✅ $2${NC}"
    else
        echo -e "${RED}❌ $2${NC}"
    fi
}

print_info() {
    echo -e "${YELLOW}ℹ️  $1${NC}"
}

# 1. Check Python version
echo "1. Checking Python versions..."
echo "------------------------------"
if command -v python3 &> /dev/null; then
    PYTHON_VERSION=$(python3 --version 2>&1)
    echo "   Python 3: $PYTHON_VERSION"
fi

if command -v python3.12 &> /dev/null; then
    PYTHON312_VERSION=$(python3.12 --version 2>&1)
    echo "   Python 3.12: $PYTHON312_VERSION"
    PYTHON_CMD="python3.12"
else
    print_info "Python 3.12 not found (recommended for pipx)"
    PYTHON_CMD="python3"
fi
echo ""

# 2. Check package managers
echo "2. Checking package managers..."
echo "-------------------------------"
HAS_PIPX=false
HAS_UVX=false

if command -v pipx &> /dev/null; then
    print_status 0 "pipx is installed"
    HAS_PIPX=true
    
    # Check pipx packages
    echo "   Pipx packages:"
    pipx list 2>/dev/null | grep -E "(whisperx|openai-whisper)" | sed 's/^/   /'
else
    print_status 1 "pipx is not installed"
fi

if command -v uvx &> /dev/null; then
    print_status 0 "uvx is installed"
    HAS_UVX=true
else
    print_status 1 "uvx is not installed"
fi
echo ""

# 3. Check WhisperX availability
echo "3. Checking WhisperX availability..."
echo "------------------------------------"

# Direct execution
if command -v whisperx &> /dev/null; then
    print_status 0 "whisperx found in PATH"
    WHISPERX_CMD="whisperx"
    WHISPERX_AVAILABLE=true
else
    print_status 1 "whisperx not in PATH"
    WHISPERX_AVAILABLE=false
fi

# Via pipx (only if not already in PATH to avoid conflicts)
if [ "$HAS_PIPX" = true ] && [ "$WHISPERX_AVAILABLE" = false ]; then
    if pipx list 2>/dev/null | grep -q "whisperx"; then
        print_status 0 "whisperx available via pipx"
        print_info "Note: whisperx is in PATH, so direct execution will be used instead of pipx run"
        
        # Check Python version used by pipx
        echo "   Checking pipx Python version for whisperx..."
        pipx runpip whisperx --version 2>&1 | grep -i python | sed 's/^/   /'
    else
        print_status 1 "whisperx not installed via pipx"
    fi
fi

# Via uvx
if [ "$HAS_UVX" = true ] && [ "$WHISPERX_AVAILABLE" = false ]; then
    if uvx list 2>/dev/null | grep -q "whisperx"; then
        print_status 0 "whisperx available via uvx"
        WHISPERX_CMD="uvx whisperx"
        WHISPERX_AVAILABLE=true
    fi
fi
echo ""

# 4. Test WhisperX functionality
if [ "$WHISPERX_AVAILABLE" = true ]; then
    echo "4. Testing WhisperX functionality..."
    echo "------------------------------------"
    
    # Test --help
    echo "   Testing: $WHISPERX_CMD --help"
    $WHISPERX_CMD --help > /dev/null 2>&1
    print_status $? "whisperx --help"
    
    # Test --version
    echo "   Testing: $WHISPERX_CMD --version"
    VERSION_OUTPUT=$($WHISPERX_CMD --version 2>&1)
    if [ $? -eq 0 ]; then
        print_status 0 "whisperx --version"
        echo "   Version: $VERSION_OUTPUT"
    else
        print_status 1 "whisperx --version"
        echo "   Error: $VERSION_OUTPUT" | head -n 3
    fi
    
    # Test Python imports
    echo ""
    echo "   Testing Python imports..."
    
    if [[ "$WHISPERX_CMD" == *"pipx"* ]]; then
        # Using pipx
        # Test whisperx import
        pipx runpip whisperx -c "import whisperx; print('WhisperX module: OK')" 2>&1
        print_status $? "import whisperx"
        
        # Test torch import
        pipx runpip whisperx -c "import torch; print(f'PyTorch version: {torch.__version__}')" 2>&1
        if [ $? -ne 0 ]; then
            print_status 1 "import torch"
            print_info "Fix: pipx inject whisperx torch torchaudio"
        fi
        
        # Test transformers import
        pipx runpip whisperx -c "import transformers; print(f'Transformers version: {transformers.__version__}')" 2>&1
        if [ $? -ne 0 ]; then
            print_status 1 "import transformers"
            print_info "Fix: pipx inject whisperx transformers"
        fi
    else
        # Direct installation - try to test imports
        # Test whisperx import
        python3 -c "import whisperx; print('WhisperX module: OK')" 2>&1
        print_status $? "import whisperx"
        
        # Test torch import
        python3 -c "import torch; print(f'PyTorch version: {torch.__version__}')" 2>&1
        if [ $? -ne 0 ]; then
            print_status 1 "import torch"
            print_info "Fix: pip install torch torchaudio"
        fi
        
        # Test transformers import
        python3 -c "import transformers; print(f'Transformers version: {transformers.__version__}')" 2>&1
        if [ $? -ne 0 ]; then
            print_status 1 "import transformers"
            print_info "Fix: pip install transformers"
        fi
    fi
    echo ""
else
    echo "4. WhisperX is not available"
    echo "-----------------------------"
    print_info "Install WhisperX with one of these commands:"
    echo "   pipx install --python python3.12 whisperx"
    echo "   pipx install git+https://github.com/m-bain/whisperx.git"
    echo "   pip install git+https://github.com/m-bain/whisperx.git"
    echo ""
fi

# 5. Check FFmpeg
echo "5. Checking FFmpeg..."
echo "---------------------"
if command -v ffmpeg &> /dev/null; then
    FFMPEG_VERSION=$(ffmpeg -version 2>&1 | head -n 1)
    print_status 0 "FFmpeg is installed"
    echo "   Version: $FFMPEG_VERSION"
else
    print_status 1 "FFmpeg is not installed"
    print_info "Install FFmpeg:"
    echo "   macOS: brew install ffmpeg"
    echo "   Ubuntu: sudo apt install ffmpeg"
fi
echo ""

# 6. Test with a sample file (if provided)
if [ -n "$1" ]; then
    if [ -f "$1" ]; then
        echo "6. Testing transcription with: $1"
        echo "-----------------------------------"
        
        if [ "$WHISPERX_AVAILABLE" = true ]; then
            # Create temp directory
            TEMP_DIR=$(mktemp -d)
            echo "   Output directory: $TEMP_DIR"
            
            # Run WhisperX with minimal settings
            echo "   Running: $WHISPERX_CMD \"$1\" --model tiny --output_dir \"$TEMP_DIR\" --compute_type int8"
            $WHISPERX_CMD "$1" --model tiny --output_dir "$TEMP_DIR" --compute_type int8 2>&1 | tee "$TEMP_DIR/whisperx_output.log"
            
            EXIT_CODE=$?
            print_status $EXIT_CODE "WhisperX transcription"
            
            if [ $EXIT_CODE -ne 0 ]; then
                echo ""
                echo "   Error details from log:"
                grep -i error "$TEMP_DIR/whisperx_output.log" | head -n 5 | sed 's/^/   /'
            else
                echo "   Output files:"
                ls -la "$TEMP_DIR" | sed 's/^/   /'
            fi
            
            # Cleanup
            rm -rf "$TEMP_DIR"
        else
            print_info "WhisperX not available, skipping test"
        fi
    else
        print_info "File not found: $1"
    fi
else
    echo "6. Sample file test"
    echo "-------------------"
    print_info "To test transcription, run: $0 <audio_or_video_file>"
fi
echo ""

# 7. Summary and recommendations
echo "7. Summary and Recommendations"
echo "==============================="

if [ "$WHISPERX_AVAILABLE" = true ]; then
    print_status 0 "WhisperX is available and can be used"
    echo ""
    echo "To use in Medianator:"
    echo "  export TRANSCRIPTION_ENGINE=whisperx"
    
    if [[ "$WHISPERX_CMD" == *"pipx"* ]]; then
        echo "  export WHISPER_RUN_METHOD=pipx"
    elif [[ "$WHISPERX_CMD" == *"uvx"* ]]; then
        echo "  export WHISPER_RUN_METHOD=uvx"
    else
        echo "  export WHISPER_RUN_METHOD=direct"
    fi
else
    print_status 1 "WhisperX is not available"
    echo ""
    echo "Recommended installation steps:"
    echo "  1. Install Python 3.12 (if not present):"
    echo "     brew install python@3.12"
    echo ""
    echo "  2. Install pipx with Python 3.12:"
    echo "     python3.12 -m pip install --user pipx"
    echo "     python3.12 -m pipx ensurepath"
    echo ""
    echo "  3. Install WhisperX:"
    echo "     pipx install --python python3.12 whisperx"
    echo ""
    echo "  4. Install dependencies if needed:"
    echo "     pipx inject whisperx torch torchaudio transformers"
fi
echo ""
echo "For more help, see: README_TRANSCRIPTION.md"