#!/bin/bash

# WhisperX and Whisper Installation Script for Medianator
# This script installs WhisperX (preferred) or OpenAI Whisper for transcription functionality
# Supports installation via pipx, uvx, or regular pip

echo "🎙️ Medianator Transcription Tools Installation Script"
echo "====================================================="
echo ""
echo "This script will install WhisperX (recommended) or Whisper for transcription."
echo "Installation methods supported: pipx (recommended), uvx, or pip"
echo ""

# Check for Python 3
if ! command -v python3 &> /dev/null; then
    echo "❌ Python 3 is not installed. Please install Python 3.8 or higher first."
    exit 1
fi

PYTHON_VERSION=$(python3 -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')
echo "✅ Found Python $PYTHON_VERSION"

# Check specifically for Python 3.12 (recommended for pipx)
if command -v python3.12 &> /dev/null; then
    PYTHON312_VERSION=$(python3.12 -c 'import sys; print(".".join(map(str, sys.version_info[:3])))')
    echo "✅ Found Python 3.12 ($PYTHON312_VERSION) - Recommended for pipx installations"
    PYTHON_CMD="python3.12"
else
    echo "📝 Python 3.12 not found. For best compatibility with pipx, consider installing Python 3.12:"
    echo "   macOS: brew install python@3.12"
    echo "   Ubuntu/Debian: sudo apt install python3.12"
    echo "   Other: https://www.python.org/downloads/"
    PYTHON_CMD=""
fi

# Check for package managers
has_pipx=false
has_uvx=false
has_pip=false

if command -v pipx &> /dev/null; then
    echo "✅ pipx is available (recommended)"
    has_pipx=true
fi

if command -v uvx &> /dev/null; then
    echo "✅ uvx is available"
    has_uvx=true
fi

if command -v pip3 &> /dev/null; then
    echo "✅ pip3 is available"
    has_pip=true
elif command -v pip &> /dev/null; then
    echo "✅ pip is available"
    has_pip=true
fi

# If none are available, try to install pipx
if [ "$has_pipx" = false ] && [ "$has_uvx" = false ] && [ "$has_pip" = false ]; then
    echo "❌ No package managers found. Attempting to install pipx..."
    
    # Prefer Python 3.12 for pipx installation
    if [ -n "$PYTHON_CMD" ] && [ "$PYTHON_CMD" = "python3.12" ]; then
        echo "Installing pipx with Python 3.12..."
        python3.12 -m ensurepip --default-pip
        if [ $? -eq 0 ]; then
            has_pip=true
            python3.12 -m pip install --user pipx
            if [ $? -eq 0 ]; then
                echo "✅ pipx installed successfully with Python 3.12"
                echo "📝 You may need to add pipx to your PATH:"
                echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
                has_pipx=true
            fi
        fi
    else
        # Fallback to regular Python 3
        python3 -m ensurepip --default-pip
        if [ $? -eq 0 ]; then
            has_pip=true
            pip3 install --user pipx
            if [ $? -eq 0 ]; then
                echo "✅ pipx installed successfully"
                echo "📝 You may need to add pipx to your PATH:"
                echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
                has_pipx=true
            fi
        fi
    fi
    
    if [ "$has_pipx" = false ]; then
        echo "❌ Failed to install pipx. Please install it manually."
        exit 1
    fi
fi

# Determine which tool to use
INSTALL_TOOL=""
if [ "$has_pipx" = true ]; then
    INSTALL_TOOL="pipx"
elif [ "$has_uvx" = true ]; then
    INSTALL_TOOL="uvx"
elif [ "$has_pip" = true ]; then
    INSTALL_TOOL="pip"
fi

echo ""
echo "📦 Will use $INSTALL_TOOL for installation"
echo ""

# Check for ffmpeg
if ! command -v ffmpeg &> /dev/null; then
    echo "⚠️  ffmpeg is not installed."
    echo ""
    
    # Detect OS and provide installation instructions
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "📦 Installing ffmpeg using Homebrew..."
        if command -v brew &> /dev/null; then
            brew install ffmpeg
        else
            echo "❌ Homebrew is not installed. Please install ffmpeg manually:"
            echo "   brew install ffmpeg"
            echo "   Or download from: https://ffmpeg.org/download.html"
            exit 1
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "📦 Please install ffmpeg using your package manager:"
        echo "   Ubuntu/Debian: sudo apt update && sudo apt install ffmpeg"
        echo "   Fedora: sudo dnf install ffmpeg"
        echo "   Arch: sudo pacman -S ffmpeg"
        exit 1
    else
        echo "📦 Please install ffmpeg from: https://ffmpeg.org/download.html"
        exit 1
    fi
else
    echo "✅ ffmpeg is installed"
fi

# Ask user which tool to install
echo "Which transcription tool would you like to install?"
echo "1) WhisperX (Recommended - includes speaker diarization, better performance)"
echo "2) OpenAI Whisper (Basic transcription)"
echo "3) Both"
echo ""
read -p "Enter your choice (1-3): " choice

install_whisperx=false
install_whisper=false

case $choice in
    1)
        install_whisperx=true
        ;;
    2)
        install_whisper=true
        ;;
    3)
        install_whisperx=true
        install_whisper=true
        ;;
    *)
        echo "Invalid choice. Installing WhisperX by default."
        install_whisperx=true
        ;;
esac

# Install WhisperX if selected
if [ "$install_whisperx" = true ]; then
    echo ""
    echo "📦 Installing WhisperX..."
    
    case $INSTALL_TOOL in
        pipx)
            echo "Using pipx for isolated installation with Python 3.12..."
            
            # Check if Python 3.12 is available
            if command -v python3.12 &> /dev/null; then
                echo "✅ Found Python 3.12"
                PYTHON_CMD="python3.12"
            elif command -v python3 &> /dev/null && python3 --version | grep -q "3.12"; then
                echo "✅ Python 3 is version 3.12"
                PYTHON_CMD="python3"
            else
                echo "⚠️  Python 3.12 not found. Attempting to install with available Python..."
                echo "   For best compatibility, install Python 3.12 first."
                PYTHON_CMD="python3"
            fi
            
            # Install with specific Python version
            pipx install --python $PYTHON_CMD git+https://github.com/m-bain/whisperx.git
            if [ $? -eq 0 ]; then
                echo "✅ WhisperX installed with pipx using $PYTHON_CMD!"
                # Inject additional dependencies if needed
                pipx inject whisperx torch torchvision torchaudio
            else
                echo "⚠️  pipx installation from git failed, trying PyPI..."
                pipx install --python $PYTHON_CMD whisperx
                if [ $? -ne 0 ]; then
                    echo "⚠️  Installation with specific Python failed, trying default..."
                    pipx install whisperx
                fi
            fi
            ;;
            
        uvx)
            echo "Using uvx for installation..."
            uvx install whisperx
            if [ $? -ne 0 ]; then
                echo "⚠️  uvx installation failed, trying from git..."
                uvx install git+https://github.com/m-bain/whisperx.git
            fi
            ;;
            
        pip)
            echo "Using pip for installation..."
            # WhisperX requires torch, so install it first
            pip3 install torch torchvision torchaudio
            # Install WhisperX
            pip3 install git+https://github.com/m-bain/whisperx.git
            if [ $? -ne 0 ]; then
                echo "⚠️  Git installation failed, trying PyPI..."
                pip3 install whisperx
            fi
            ;;
    esac
    
    # Test WhisperX installation
    echo ""
    echo "Testing WhisperX installation..."
    
    # Try different ways to run whisperx
    if command -v whisperx &> /dev/null; then
        echo "🎉 WhisperX is available as 'whisperx'"
        whisperx --help | head -n 3
    elif [ "$INSTALL_TOOL" = "pipx" ] && pipx list | grep -q whisperx; then
        echo "🎉 WhisperX is installed via pipx"
        echo "   Run with: pipx run whisperx"
        pipx run whisperx --help | head -n 3
    elif [ "$INSTALL_TOOL" = "uvx" ] && uvx list | grep -q whisperx; then
        echo "🎉 WhisperX is installed via uvx"
        echo "   Run with: uvx whisperx"
        uvx whisperx --help | head -n 3
    else
        echo "⚠️  WhisperX installation status unclear. You may need to add it to PATH."
    fi
fi

# Install OpenAI Whisper if selected
if [ "$install_whisper" = true ]; then
    echo ""
    echo "📦 Installing OpenAI Whisper..."
    
    case $INSTALL_TOOL in
        pipx)
            echo "Using pipx for isolated installation with Python 3.12..."
            
            # Check if Python 3.12 is available (reuse if already set)
            if [ -z "$PYTHON_CMD" ]; then
                if command -v python3.12 &> /dev/null; then
                    echo "✅ Found Python 3.12"
                    PYTHON_CMD="python3.12"
                elif command -v python3 &> /dev/null && python3 --version | grep -q "3.12"; then
                    echo "✅ Python 3 is version 3.12"
                    PYTHON_CMD="python3"
                else
                    echo "⚠️  Python 3.12 not found. Attempting to install with available Python..."
                    echo "   For best compatibility, install Python 3.12 first."
                    PYTHON_CMD="python3"
                fi
            fi
            
            # Install with specific Python version
            pipx install --python $PYTHON_CMD openai-whisper
            if [ $? -ne 0 ]; then
                echo "⚠️  Installation with specific Python failed, trying default..."
                pipx install openai-whisper
            fi
            ;;
            
        uvx)
            echo "Using uvx for installation..."
            uvx install openai-whisper
            ;;
            
        pip)
            echo "Using pip for installation..."
            pip3 install --upgrade openai-whisper
            ;;
    esac
    
    # Test whisper installation
    echo ""
    echo "Testing Whisper installation..."
    
    if command -v whisper &> /dev/null; then
        echo "🎉 Whisper is available as 'whisper'"
        whisper --help | head -n 3
    elif [ "$INSTALL_TOOL" = "pipx" ] && pipx list | grep -q openai-whisper; then
        echo "🎉 Whisper is installed via pipx"
        echo "   Run with: pipx run whisper"
        pipx run whisper --help | head -n 3
    elif [ "$INSTALL_TOOL" = "uvx" ] && uvx list | grep -q openai-whisper; then
        echo "🎉 Whisper is installed via uvx"
        echo "   Run with: uvx whisper"
        uvx whisper --help | head -n 3
    else
        echo "⚠️  Whisper installation status unclear. You may need to add it to PATH."
    fi
fi

echo ""
echo "📝 Installation Summary:"
echo "========================"

whisperx_available=false
whisper_available=false
run_method=""

# Check WhisperX availability
if command -v whisperx &> /dev/null; then
    echo "✅ WhisperX is available directly as 'whisperx'"
    whisperx_available=true
    run_method="direct"
elif [ "$INSTALL_TOOL" = "pipx" ] && pipx list 2>/dev/null | grep -q whisperx; then
    echo "✅ WhisperX is available via pipx"
    echo "   Run with: pipx run whisperx"
    whisperx_available=true
    run_method="pipx"
elif [ "$INSTALL_TOOL" = "uvx" ] && uvx list 2>/dev/null | grep -q whisperx; then
    echo "✅ WhisperX is available via uvx"
    echo "   Run with: uvx whisperx"
    whisperx_available=true
    run_method="uvx"
fi

# Check Whisper availability
if command -v whisper &> /dev/null; then
    echo "✅ OpenAI Whisper is available directly as 'whisper'"
    whisper_available=true
elif [ "$INSTALL_TOOL" = "pipx" ] && pipx list 2>/dev/null | grep -q openai-whisper; then
    echo "✅ OpenAI Whisper is available via pipx"
    echo "   Run with: pipx run whisper"
    whisper_available=true
elif [ "$INSTALL_TOOL" = "uvx" ] && uvx list 2>/dev/null | grep -q openai-whisper; then
    echo "✅ OpenAI Whisper is available via uvx"
    echo "   Run with: uvx whisper"
    whisper_available=true
fi

if [ "$whisperx_available" = false ] && [ "$whisper_available" = false ]; then
    echo "❌ No transcription tools were successfully installed."
    echo "   Please check the error messages above and try manual installation:"
    case $INSTALL_TOOL in
        pipx)
            echo "   - WhisperX: pipx install git+https://github.com/m-bain/whisperx.git"
            echo "   - Whisper: pipx install openai-whisper"
            ;;
        uvx)
            echo "   - WhisperX: uvx install whisperx"
            echo "   - Whisper: uvx install openai-whisper"
            ;;
        *)
            echo "   - WhisperX: pip3 install git+https://github.com/m-bain/whisperx.git"
            echo "   - Whisper: pip3 install openai-whisper"
            ;;
    esac
    exit 1
fi

echo ""
echo "📝 Configuration Notes:"
echo "========================"
echo "   - The first time you use transcription, models will be downloaded (~1.5GB)"
echo "   - WhisperX provides better performance and speaker diarization"
echo "   - Set TRANSCRIPTION_ENGINE=whisperx or TRANSCRIPTION_ENGINE=whisper to choose"
echo "   - By default, WhisperX will be used if available"

if [ "$run_method" = "pipx" ]; then
    echo ""
    echo "📝 Using pipx:"
    echo "   - Tools are installed in isolated environments"
    echo "   - Medianator will automatically use 'pipx run' to execute them"
    echo "   - Set WHISPER_RUN_METHOD=pipx in your environment"
elif [ "$run_method" = "uvx" ]; then
    echo ""
    echo "📝 Using uvx:"
    echo "   - Tools are installed in isolated environments"
    echo "   - Medianator will automatically use 'uvx' to execute them"
    echo "   - Set WHISPER_RUN_METHOD=uvx in your environment"
fi

echo ""
echo "🚀 You can now use the transcription feature in Medianator!"
echo "   Just click the 'Transcribe' button on any audio or video file."