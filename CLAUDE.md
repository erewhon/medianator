# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Medianator is a high-performance asynchronous media catalog engine written in Rust that scans, indexes, and serves media files with comprehensive metadata extraction and Prometheus monitoring.

## Development Commands

### Building and Running
```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run the application
cargo run

# Run with hot reload during development
cargo watch -x run
```

### Testing and Code Quality
```bash
# Run tests
cargo test

# Run linter
cargo clippy -- -D warnings

# Format code
cargo fmt

# Check code without building
cargo check
```

### Database Management
```bash
# Run migrations (handled automatically on startup)
# Database URL is configured via DATABASE_URL env var or defaults to sqlite://medianator.db
```

## Architecture

### Core Components

1. **Scanner Engine** (`src/scanner/`)
   - Asynchronously scans directories for media files
   - Extracts metadata from images (EXIF), videos (codec info), and audio files
   - Computes SHA-256 hashes for deduplication
   - Uses walkdir for recursive directory traversal

2. **Database Layer** (`src/db/`)
   - SQLx with SQLite backend
   - Async connection pooling
   - Migrations in `migrations/` directory
   - Full-text search capabilities

3. **Web API** (`src/api/`)
   - Built on Axum framework
   - RESTful endpoints for media queries, scanning, and statistics
   - Prometheus metrics endpoint at `/metrics`
   - CORS support via tower-http

4. **Models** (`src/models/`)
   - Data structures for media files and metadata
   - Serde serialization for JSON API responses

### Key Design Patterns

- **Async-first**: All I/O operations use Tokio async runtime
- **Streaming**: Large file operations use streaming to minimize memory usage
- **Connection pooling**: Database connections managed by SQLx pool
- **Error propagation**: Uses `thiserror` and `anyhow` for structured error handling

## Configuration

Environment variables (or `.env` file):
- `SERVER_HOST`: Bind address (default: `0.0.0.0`)
- `SERVER_PORT`: Port (default: `3000`)
- `DATABASE_URL`: SQLite URL (default: `sqlite://medianator.db`)
- `AUTO_SCAN_PATHS`: Comma-separated paths to scan on startup
- `RUST_LOG`: Logging level (default: `medianator=info`)

## API Endpoints

- `GET /health` - Health check
- `GET /api/media` - List media with pagination
- `GET /api/media/search` - Full-text search
- `GET /api/media/{id}` - Get media metadata
- `GET /api/media/{id}/image` - Stream image file
- `POST /api/scan` - Start directory scan
- `GET /api/stats` - Catalog statistics
- `GET /api/scan/history` - Scan history
- `GET /metrics` - Prometheus metrics

## Dependencies

Key dependencies managed in `Cargo.toml`:
- **tokio**: Async runtime
- **axum**: Web framework
- **sqlx**: Database access with SQLite
- **image**: Image metadata extraction
- **kamadak-exif**: EXIF data parsing
- **symphonia**: Audio metadata
- **prometheus**: Metrics collection