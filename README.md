# gtakeout

A command-line utility for processing Google Takeout photo metadata and updating EXIF timestamps.

## Overview

When you export your photos from Google Photos using Google Takeout, you receive `.supplemental-metadata.json` files alongside your images. These JSON files contain metadata including the original photo taken time. This tool automatically updates the EXIF data of images that lack timestamps using the information from these metadata files.

## Features

- Recursively searches directories for Google Takeout metadata files
- **Parallel processing across all CPU cores for maximum performance**
- Extracts photo timestamps from JSON metadata
- Checks existing EXIF data to avoid overwriting
- Updates EXIF DateTimeOriginal, DateTime, and CreateDate fields
- Provides summary statistics after processing

## Prerequisites

- Rust toolchain (for building)
- [exiftool](https://exiftool.org/) must be installed on your system

### Installing exiftool

**Linux (Debian/Ubuntu):**
```bash
sudo apt install libimage-exiftool-perl
```

**Linux (Fedora/RHEL):**
```bash
sudo dnf install perl-Image-ExifTool
```

**macOS:**
```bash
brew install exiftool
```

**Other systems:**
Download from [exiftool.org](https://exiftool.org/)

## Installation

```bash
cargo build --release
```

The binary will be available at `target/release/gtakeout`

## Usage

```bash
gtakeout <DIRECTORY>
```

Where `<DIRECTORY>` is the path to your Google Takeout export folder.

### Example

```bash
# Process all photos in the Takeout directory
gtakeout ~/Downloads/Takeout

# Process a specific album
gtakeout ~/Downloads/Takeout/Google\ Photos/My\ Album
```

### Output

The tool will:
- Display which files are being updated
- Skip files that already have EXIF dates
- Report any errors encountered
- Show a summary of operations performed

Example output:
```
Searching for supplemental metadata files in: test
Updated: test/IMG-20161219-WA0000.jpg
Skipped (already has EXIF date): test/IMG-20200101-WA0001.jpg

Summary:
  Metadata files found: 2
  Images updated: 1
  Errors: 0
```

## How It Works

1. Scans directory recursively for `*.supplemental-metadata.json` files
2. Collects all matching files into a list
3. **Processes all files in parallel using all available CPU cores**
4. For each metadata file, determines the corresponding image filename
5. Checks if the image has existing EXIF date information
6. If no date exists, extracts `photoTakenTime.timestamp` from JSON
7. Converts Unix timestamp to EXIF datetime format (YYYY:MM:DD HH:MM:SS)
8. Uses exiftool to write the timestamp to the image

### Performance

The application uses **Rayon** for parallel processing, which automatically distributes the workload across all available CPU cores. This provides significant performance improvements when processing large Google Takeout exports:

- On a quad-core system, expect up to 3-4x speedup compared to sequential processing
- On an 8-core system, expect up to 6-8x speedup
- Actual speedup depends on I/O performance and the speed of exiftool operations

## Safety

- Original files are modified directly (exiftool uses `-overwrite_original` flag)
- Only images without existing EXIF dates are modified
- Make a backup of your photos before running if you're concerned

## License

This project is licensed under the WTFPL (Do What The Fuck You Want To Public License) - see the LICENSE file for details.

In short: You just DO WHAT THE FUCK YOU WANT TO.
