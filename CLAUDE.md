# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`gtakeout` is a Rust utility for processing Google Takeout data, specifically designed to handle the supplemental metadata JSON files that accompany photos and videos in Google Photos exports. It supports both image files (JPEG, PNG, etc.) and video files (MP4, MOV, AVI, MKV, etc.).

## Build and Development Commands

```bash
# Build the project
cargo build

# Build with optimizations
cargo build --release

# Run the application
cargo run

# Run with arguments
cargo run -- <args>

# Run tests (includes integration test that requires exiftool)
cargo test

# Run a specific test
cargo test <test_name>

# Run tests with output visible
cargo test -- --nocapture

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Architecture

### Data Format

The project processes Google Photos supplemental metadata JSON files (e.g., `IMG-20161219-WA0000.jpg.supplemental-metadata.json`) that contain:
- Photo metadata (title, description, views, timestamps)
- Geolocation data (latitude, longitude, altitude)
- Google Photos origin information (device type, folder name)
- App source information (e.g., WhatsApp uploads)
- Archive status and photo URLs

Test data is located in the `test/` directory with sample JSON metadata files paired with their corresponding image files.

### Project Structure

- `src/main.rs` - Entry point (currently a skeleton)
- `test/` - Sample Google Takeout data for development and testing
- `Cargo.toml` - Project configuration with Rust edition 2024

## Implementation Details

### Core Functionality

The application:
1. Recursively searches a directory for `*.supplemental-metadata.json` files
2. Collects all matching metadata file paths into a vector
3. Processes all files in parallel using Rayon, distributing work across all available CPU cores
4. For each metadata file, extracts the base filename (e.g., `IMG-20161219-WA0000.jpg.supplemental-metadata.json` → `IMG-20161219-WA0000.jpg`)
5. Checks if the corresponding image file exists
6. Reads the image's EXIF data to determine if it already has a date/time stamp (checks DateTimeOriginal, DateTime, and DateTimeDigitized tags)
7. If no EXIF date exists, extracts the `photoTakenTime.timestamp` from the JSON metadata
8. Uses `exiftool` (external command) to write the timestamp to the image's EXIF data

### Parallel Processing

The application uses **Rayon** for parallel processing to maximize throughput:
- Directory scanning is done sequentially (fast operation)
- File processing (JSON parsing, EXIF reading/writing) is parallelized across all CPU cores
- Thread-safe atomic counters (`AtomicUsize`) track statistics across parallel operations
- This provides significant speedup when processing large Google Takeout exports with thousands of photos

### Dependencies

- `clap` - Command-line argument parsing
- `serde` / `serde_json` - JSON deserialization
- `walkdir` - Recursive directory traversal
- `kamadak-exif` - Reading EXIF data from images
- `chrono` - Timestamp conversion and formatting
- `rayon` - Data parallelism for multi-core processing
- **External requirement**: `exiftool` must be installed on the system for writing EXIF data

### Key Functions

- `main()` - CLI entry point, orchestrates the directory scan and processing (src/main.rs:27)
- `process_metadata_file()` - Handles processing of a single metadata/media pair (src/main.rs:95)
- `get_base_image_path()` - Strips `.supplemental-metadata.json` suffix to find media filename (src/main.rs:119)
- `is_video_file()` - Determines if a file is a video based on extension (src/main.rs:128)
- `has_exif_date()` - Checks if an image/video already has date metadata; uses kamadak-exif for images, exiftool for videos (src/main.rs:140)
- `update_exif_date()` - Calls exiftool to write timestamp to image/video metadata with appropriate tags (src/main.rs:196)

## Development Notes

- The project uses Rust edition 2024
- The supplemental metadata JSON format includes nested structures for time data, geolocation, and origin information
- EXIF writing is delegated to the external `exiftool` command (the `kamadak-exif` crate is read-only)
- Media files are only modified if they lack date metadata to avoid overwriting existing timestamps
- **Video support**: The tool handles both images and videos, using different metadata tags:
  - Images: Uses kamadak-exif for reading (fast), exiftool for writing
  - Videos: Uses exiftool for both reading and writing (supports QuickTime/MP4 tags)
  - Supported video formats: MP4, MOV, AVI, MKV, M4V, 3GP, WebM, FLV, WMV

## Testing

The project includes both unit tests and integration tests:

- **Unit test**: `test_get_base_image_path` - Verifies filename manipulation logic (src/main.rs:250)
- **Unit test**: `test_is_video_file` - Verifies video file detection by extension (src/main.rs:256)
- **Integration test**: `test_integration_with_real_files` - Full end-to-end test (src/main.rs:273)

The integration test:
1. Copies test files from `test/` to `target/test_data/`
2. Strips EXIF data from the image using exiftool
3. Runs the `process_metadata_file()` function
4. Verifies EXIF data was correctly written with the timestamp from JSON
5. Tests idempotency by running again (should skip file with existing date)
6. Cleans up the test directory

**Note**: The integration test gracefully skips if `exiftool` is not installed on the system.
