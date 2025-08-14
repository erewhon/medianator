# Transcription Setup for Medianator

## Transcription Tools

Medianator supports two transcription engines:

1. **WhisperX** (Recommended) - Advanced features including:
   - Real-time progress streaming
   - Speaker diarization (identifies different speakers)
   - Word-level timestamps
   - Better performance through optimizations
   - Automatic language detection

2. **OpenAI Whisper** - Basic transcription with:
   - High-quality transcription
   - Multiple language support
   - Various model sizes

## Quick Installation

Run the installation script:
```bash
./install_whisper.sh
```

Choose option 1 for WhisperX (recommended), option 2 for Whisper, or option 3 for both.

### Prerequisites
- Python 3.12 (recommended for pipx) or Python 3.8+ (minimum)
- pip (Python package manager)
- ffmpeg (for audio processing)
- pipx (recommended) or uvx for isolated installations

## Manual Installation

### Using pipx (Recommended)

pipx installs Python applications in isolated environments, preventing dependency conflicts. **For best compatibility, use Python 3.12.**

#### Install Python 3.12 (if needed)
```bash
# macOS (using Homebrew)
brew install python@3.12

# Ubuntu/Debian
sudo apt update
sudo apt install python3.12 python3.12-venv python3.12-dev

# Fedora
sudo dnf install python3.12

# Windows (download from python.org)
# https://www.python.org/downloads/release/python-3120/
```

#### Install pipx with Python 3.12
```bash
# Install pipx using Python 3.12
python3.12 -m pip install --user pipx
python3.12 -m pipx ensurepath

# Or if python3 is already 3.12
python3 -m pip install --user pipx
python3 -m pipx ensurepath
```

#### Install WhisperX with pipx (Python 3.12)
```bash
# Install WhisperX with Python 3.12 environment
pipx install --python python3.12 git+https://github.com/m-bain/whisperx.git

# Or from PyPI if available
pipx install --python python3.12 whisperx

# Verify installation
pipx run whisperx --help

# Check Python version used
pipx runpip whisperx --version
```

#### Install Whisper with pipx (Python 3.12)
```bash
# Install OpenAI Whisper with Python 3.12
pipx install --python python3.12 openai-whisper

# Verify installation
pipx run whisper --help

# Check Python version used
pipx runpip openai-whisper --version
```

#### Reinstall Existing Tools with Python 3.12
If you already have tools installed with a different Python version:
```bash
# Reinstall WhisperX with Python 3.12
pipx reinstall --python python3.12 whisperx

# Reinstall Whisper with Python 3.12
pipx reinstall --python python3.12 openai-whisper
```

### Using uvx

uvx is an alternative to pipx with similar benefits.

```bash
# Install WhisperX
uvx install whisperx

# Install Whisper
uvx install openai-whisper

# Run commands
uvx whisperx --help
uvx whisper --help
```

### Traditional pip Installation

If you prefer traditional installation:

#### WhisperX Installation

1. **Install PyTorch** (required by WhisperX):
   ```bash
   pip install torch torchvision torchaudio
   ```

2. **Install WhisperX**:
   ```bash
   pip install git+https://github.com/m-bain/whisperx.git
   ```

3. **Verify installation**:
   ```bash
   whisperx --help
   ```

#### OpenAI Whisper Installation

1. **Install ffmpeg** (if not already installed):
   ```bash
   # On macOS with Homebrew:
   brew install ffmpeg
   
   # On Ubuntu/Debian:
   sudo apt update && sudo apt install ffmpeg
   
   # On Windows:
   # Download from https://ffmpeg.org/download.html
   ```

2. **Install OpenAI Whisper**:
   ```bash
   pip install openai-whisper
   ```

3. **Verify installation**:
   ```bash
   whisper --help
   ```

### Usage

Once installed, the transcription feature will automatically work when you:
1. Open a media file (audio or video) in the Medianator UI
2. Click the "Transcribe" button
3. The system will use Whisper to generate transcriptions with real-time progress updates

### Model Options

By default, Medianator uses the "base" model for a balance of speed and accuracy. You can modify this in the code if needed:
- `tiny` - Fastest, least accurate (~1GB)
- `base` - Good balance (default) (~1.5GB)
- `small` - Better accuracy (~2.5GB)
- `medium` - Even better (~5GB)
- `large` - Best accuracy (~10GB)

The model will be downloaded automatically on first use.

### Troubleshooting

If you encounter issues:

1. **Check Python version**:
   ```bash
   python --version
   # Should be 3.8 or higher
   ```

2. **Check if Whisper is in PATH**:
   ```bash
   which whisper
   # Should return the path to whisper executable
   ```

3. **Reinstall Whisper**:
   ```bash
   pip uninstall openai-whisper
   pip install --upgrade openai-whisper
   ```

4. **Check ffmpeg**:
   ```bash
   ffmpeg -version
   ```

### Alternative: Using Whisper.cpp (Faster)

For better performance, you can use whisper.cpp instead:

```bash
# Clone and build whisper.cpp
git clone https://github.com/ggerganov/whisper.cpp
cd whisper.cpp
make

# Download a model
bash ./models/download-ggml-model.sh base

# Create a wrapper script at /usr/local/bin/whisper
sudo tee /usr/local/bin/whisper << 'EOF'
#!/bin/bash
/path/to/whisper.cpp/main "$@"
EOF

sudo chmod +x /usr/local/bin/whisper
```

## Configuration

### Choosing the Transcription Engine

By default, Medianator will use WhisperX if available, falling back to Whisper if not.

To explicitly choose an engine, set the environment variable:
```bash
# Use WhisperX (default if available)
export TRANSCRIPTION_ENGINE=whisperx

# Use OpenAI Whisper
export TRANSCRIPTION_ENGINE=whisper

# Or set it when running the server
TRANSCRIPTION_ENGINE=whisperx DATABASE_URL="sqlite://medianator.db" ./target/release/medianator
```

### Choosing the Run Method

Medianator can automatically detect how to run the transcription tools, or you can specify:

```bash
# Auto-detect (default) - tries direct, then pipx, then uvx
export WHISPER_RUN_METHOD=auto

# Force pipx execution
export WHISPER_RUN_METHOD=pipx

# Force uvx execution
export WHISPER_RUN_METHOD=uvx

# Force direct execution (traditional PATH-based)
export WHISPER_RUN_METHOD=direct

# Example: Use WhisperX via pipx
TRANSCRIPTION_ENGINE=whisperx WHISPER_RUN_METHOD=pipx ./target/release/medianator
```

### Benefits of pipx/uvx

1. **Isolated Environments**: Each tool has its own virtual environment
2. **No Dependency Conflicts**: Tools don't interfere with each other
3. **Easy Updates**: `pipx upgrade whisperx` or `uvx upgrade whisperx`
4. **Clean Uninstalls**: `pipx uninstall whisperx` removes everything
5. **System Python Unchanged**: Your system Python remains clean

## Features

### Common Features (Both Engines)
- Real-time progress updates via WebSocket
- Segment-by-segment streaming of transcription results
- Support for multiple languages
- Detailed logging of the transcription process
- Automatic error handling and user feedback
- Automatic fallback from WhisperX to Whisper if needed

### WhisperX-Specific Features
- **Speaker Diarization**: Identifies and labels different speakers
- **Word-Level Timestamps**: More precise timing information
- **Streaming Progress**: Real-time progress percentage during processing
- **Better Performance**: Optimized for speed with batched inference
- **Auto Language Detection**: Automatically detects the spoken language
- **VAD (Voice Activity Detection)**: Better handling of silence

### Usage with Speaker Diarization
When using WhisperX, enable speaker diarization in the UI to:
- Identify different speakers in the audio
- Label segments with speaker IDs (SPEAKER_00, SPEAKER_01, etc.)
- Useful for interviews, meetings, and multi-person conversations

**Note: Speaker diarization requires a HuggingFace token:**

1. Create a free account at https://huggingface.co/
2. Generate an access token at https://huggingface.co/settings/tokens
3. Accept the terms for the diarization model at https://huggingface.co/pyannote/speaker-diarization-3.1
4. Set the token as an environment variable:
   ```bash
   export HF_TOKEN=your_token_here
   # or
   export HUGGING_FACE_TOKEN=your_token_here
   ```
5. Run Medianator with the token:
   ```bash
   HF_TOKEN=your_token_here ./target/release/medianator
   ```

Without the HF_TOKEN, transcription will still work but speaker diarization will be disabled.

## WebSocket Events

The transcription process sends the following WebSocket events:
- `transcription_progress` - Updates on transcription progress (0-100%)
- `transcription_segment` - Individual segments as they're processed
- `error` - Any errors during transcription

These events are displayed in real-time in the UI, providing immediate feedback to users.

## Troubleshooting

### Python Version Issues

#### pipx using wrong Python version
```bash
# Check current Python version for a pipx package
pipx runpip whisperx list  # Shows pip and Python info

# Reinstall with Python 3.12
pipx uninstall whisperx
pipx install --python python3.12 whisperx
```

#### Multiple Python versions
```bash
# List all Python versions
ls -la /usr/bin/python* /usr/local/bin/python*

# Check pipx default Python
pipx environment

# Set pipx to use Python 3.12 by default
export PIPX_DEFAULT_PYTHON=python3.12
```

#### Python 3.12 not found
```bash
# macOS - Install with Homebrew
brew install python@3.12
brew link python@3.12

# Ubuntu/Debian - Add deadsnakes PPA
sudo add-apt-repository ppa:deadsnakes/ppa
sudo apt update
sudo apt install python3.12 python3.12-venv

# Check installation
python3.12 --version
```

### pipx Installation Issues

#### pipx command not found after installation
```bash
# Add to PATH manually
export PATH="$HOME/.local/bin:$PATH"

# Make permanent (add to ~/.bashrc or ~/.zshrc)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

#### pipx package conflicts
```bash
# List all pipx packages
pipx list

# Uninstall conflicting package
pipx uninstall <package-name>

# Reinstall with Python 3.12
pipx install --python python3.12 <package-name>
```