# Medianator

A high-performance asynchronous media catalog engine written in Rust that scans, indexes, and serves media files with comprehensive metadata extraction and Prometheus monitoring.

## Features

- **🚀 Async Architecture**: Built on Tokio for high-performance concurrent operations
- **📁 Multi-format Support**: Handles 30+ image, video, and audio formats
- **🔍 Smart Metadata Extraction**: 
  - EXIF data from images (camera, lens, settings)
  - Video codec information and dimensions
  - Audio codec and bitrate details
  - SHA-256 file hashing for deduplication
- **💾 SQLite Storage**: Lightweight embedded database with full-text search
- **🌐 RESTful API**: Complete web API with CORS support
- **📊 Prometheus Metrics**: Built-in monitoring and performance tracking
- **⚡ Real-time Scanning**: Track scan progress and history
- **🖼️ Direct Image Serving**: Stream images directly through the API

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/medianator.git
cd medianator

# Build the project
cargo build --release

# Run with default settings
cargo run --release
```

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/medianator /usr/local/bin/
EXPOSE 3000
CMD ["medianator"]
```

## Configuration

Configure via environment variables or `.env` file:

| Variable | Description | Default |
|----------|-------------|---------|
| `SERVER_HOST` | Server bind address | `0.0.0.0` |
| `SERVER_PORT` | Server port | `3000` |
| `DATABASE_URL` | SQLite database URL | `sqlite://medianator.db` |
| `AUTO_SCAN_PATHS` | Comma-separated paths to scan on startup | _(empty)_ |
| `SCAN_INTERVAL_MINUTES` | Auto-rescan interval | _(disabled)_ |
| `RUST_LOG` | Logging level | `medianator=info` |

Example `.env`:
```bash
SERVER_PORT=8080
DATABASE_URL=sqlite:///data/media_catalog.db
AUTO_SCAN_PATHS=/media/photos,/media/videos
RUST_LOG=medianator=debug,tower_http=info
```

## API Reference

### Core Endpoints

#### Health Check
```http
GET /health
```
Returns service status and version.

#### List Media
```http
GET /api/media?media_type=image&limit=50&offset=0
```
Lists media files with optional filtering and pagination.

#### Search Media
```http
GET /api/media/search?q=vacation
```
Full-text search across file names and paths.

#### Get Media Metadata
```http
GET /api/media/{id}
```
Returns complete metadata for a specific media file.

#### Retrieve Image
```http
GET /api/media/{id}/image
```
Streams the actual image file (images only).

#### Start Scan
```http
POST /api/scan
Content-Type: application/json

{
  "path": "/path/to/media/folder"
}
```
Initiates an asynchronous directory scan.

#### Statistics
```http
GET /api/stats
```
Returns catalog statistics including file counts and total size.

#### Scan History
```http
GET /api/scan/history
```
Lists recent scan operations with results.

### Metrics Endpoint

```http
GET /metrics
```
Prometheus-compatible metrics including:
- `http_requests_total` - HTTP request counter
- `http_request_duration_seconds` - Request latency histogram
- `media_files_scanned_total` - Total files scanned
- `media_files_indexed_total` - Total files indexed
- `database_queries_total` - Database operation metrics
- `scan_duration_seconds` - Scan operation duration

## Supported Formats

### Images
`jpg`, `jpeg`, `png`, `gif`, `bmp`, `webp`, `tiff`, `tif`, `svg`, `ico`

### Videos
`mp4`, `avi`, `mov`, `wmv`, `flv`, `mkv`, `webm`, `m4v`, `mpg`, `mpeg`

### Audio
`mp3`, `wav`, `flac`, `aac`, `ogg`, `wma`, `m4a`, `opus`, `aiff`, `ape`

## Database Schema

The SQLite database stores comprehensive metadata:

```sql
media_files
├── id (UUID)
├── file_path (unique)
├── file_name
├── file_size
├── file_hash (SHA-256)
├── media_type (image|video|audio)
├── mime_type
├── dimensions (width, height)
├── duration_seconds
├── camera_info (EXIF data)
├── codec_info
├── timestamps
└── extra_metadata (JSON)
```

## Usage Examples

### Python Client
```python
import requests

# Start a scan
response = requests.post('http://localhost:3000/api/scan', 
                         json={'path': '/media/photos'})
scan_result = response.json()

# Search for files
results = requests.get('http://localhost:3000/api/media/search',
                       params={'q': 'sunset'}).json()

# Get image metadata
metadata = requests.get(f'http://localhost:3000/api/media/{media_id}').json()

# Download image
image_data = requests.get(f'http://localhost:3000/api/media/{media_id}/image').content
```

### cURL Examples
```bash
# Start scanning a directory
curl -X POST http://localhost:3000/api/scan \
  -H "Content-Type: application/json" \
  -d '{"path": "/home/user/Pictures"}'

# List all videos
curl "http://localhost:3000/api/media?media_type=video&limit=20"

# Get catalog statistics
curl http://localhost:3000/api/stats | jq

# Monitor with Prometheus
curl http://localhost:3000/metrics | grep media_files
```

## Performance

- **Concurrent Scanning**: Processes multiple files simultaneously
- **Streaming Architecture**: Minimal memory usage for large catalogs
- **Indexed Queries**: Fast searches via SQLite indexes
- **Connection Pooling**: Efficient database connection management
- **Zero-Copy File Serving**: Direct file streaming for images

## Development

### Requirements
- Rust 1.75+
- SQLite 3.35+

### Building from Source
```bash
# Development build with hot reload
cargo watch -x run

# Run tests
cargo test

# Check code
cargo clippy -- -D warnings

# Format code
cargo fmt
```

### Project Structure
```
medianator/
├── src/
│   ├── main.rs           # Application entry point
│   ├── config.rs         # Configuration management
│   ├── models/           # Data structures
│   ├── db/               # Database layer
│   ├── scanner/          # Media scanning engine
│   │   ├── mod.rs        # Scanner orchestration
│   │   └── metadata.rs   # Metadata extraction
│   └── api/              # Web API
│       ├── handlers.rs   # Request handlers
│       └── metrics.rs    # Prometheus metrics
├── migrations/           # Database migrations
└── Cargo.toml           # Dependencies
```

## Monitoring

### Grafana Dashboard
```json
{
  "dashboard": {
    "panels": [
      {
        "title": "Request Rate",
        "targets": [
          {"expr": "rate(http_requests_total[5m])"}
        ]
      },
      {
        "title": "Files Indexed",
        "targets": [
          {"expr": "media_files_indexed_total"}
        ]
      }
    ]
  }
}
```

### Prometheus Scrape Config
```yaml
scrape_configs:
  - job_name: 'medianator'
    static_configs:
      - targets: ['localhost:3000']
    metrics_path: '/metrics'
```

## Roadmap

- [ ] Thumbnail generation
- [ ] Video preview extraction  
- [ ] Duplicate detection
- [ ] Face recognition
- [ ] S3/Cloud storage support
- [ ] WebSocket real-time updates
- [ ] Batch operations API
- [ ] Plugin system

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

Built with:
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [SQLx](https://github.com/launchbadge/sqlx) - Async SQL toolkit
- [Tokio](https://tokio.rs/) - Async runtime
- [Prometheus](https://prometheus.io/) - Monitoring