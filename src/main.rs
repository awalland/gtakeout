use clap::Parser;
use rayon::prelude::*;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "gtakeout")]
#[command(about = "Process Google Takeout metadata and update EXIF data", long_about = None)]
struct Args {
    /// Directory to search recursively for supplemental metadata files
    #[arg(value_name = "DIRECTORY")]
    directory: PathBuf,
}

#[derive(Debug, Deserialize)]
struct PhotoTakenTime {
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    #[serde(rename = "photoTakenTime")]
    photo_taken_time: PhotoTakenTime,
}

fn main() {
    let args = Args::parse();

    if !args.directory.exists() {
        eprintln!("Error: Directory '{}' does not exist", args.directory.display());
        std::process::exit(1);
    }

    if !args.directory.is_dir() {
        eprintln!("Error: '{}' is not a directory", args.directory.display());
        std::process::exit(1);
    }

    println!("Searching for supplemental metadata files in: {}", args.directory.display());

    // Collect all metadata file paths first
    let metadata_files: Vec<PathBuf> = WalkDir::new(&args.directory)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            if !path.is_file() {
                return None;
            }

            let filename = path.file_name()?.to_string_lossy();
            if filename.ends_with(".supplemental-metadata.json") {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
        .collect();

    let processed_count = metadata_files.len();
    let updated_count = AtomicUsize::new(0);
    let error_count = AtomicUsize::new(0);

    // Process files in parallel across all CPU cores
    metadata_files.par_iter().for_each(|path| {
        match process_metadata_file(path) {
            Ok(true) => {
                println!("Updated: {}", path.display());
                updated_count.fetch_add(1, Ordering::Relaxed);
            }
            Ok(false) => {
                println!("Skipped (already has EXIF date): {}", path.display());
            }
            Err(e) => {
                eprintln!("Error processing {}: {}", path.display(), e);
                error_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    println!("\nSummary:");
    println!("  Metadata files found: {}", processed_count);
    println!("  Media files updated: {}", updated_count.load(Ordering::Relaxed));
    println!("  Errors: {}", error_count.load(Ordering::Relaxed));
}

fn process_metadata_file(json_path: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    // Find corresponding media file
    let media_path = get_base_media_path(json_path)?;

    // Check if media file exists before reading JSON
    if !media_path.exists() {
        return Err(format!("Media file not found: {}", media_path.display()).into());
    }

    // Check if media already has date metadata before parsing JSON
    if has_exif_date(&media_path)? {
        return Ok(false); // Already has date, skip
    }

    // Only parse JSON if we need to update the file
    let json_content = fs::read_to_string(json_path)?;
    let metadata: Metadata = serde_json::from_str(&json_content)?;

    // Update EXIF data
    let timestamp: i64 = metadata.photo_taken_time.timestamp.parse()?;
    update_exif_date(&media_path, timestamp)?;

    Ok(true)
}

fn get_base_media_path(json_path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path_str = json_path.to_string_lossy();

    if !path_str.ends_with(".supplemental-metadata.json") {
        return Err("Path does not end with .supplemental-metadata.json".into());
    }

    // Remove the .supplemental-metadata.json suffix
    let base_path = path_str.trim_end_matches(".supplemental-metadata.json");
    Ok(PathBuf::from(base_path))
}

fn is_video_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        matches!(
            ext_lower.as_str(),
            "mp4" | "mov" | "avi" | "mkv" | "m4v" | "3gp" | "webm" | "flv" | "wmv"
        )
    } else {
        false
    }
}

fn has_exif_date(file_path: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    // For video files, use exiftool to check for dates since kamadak-exif doesn't support videos
    if is_video_file(file_path) {
        use std::process::Command;

        let output = Command::new("exiftool")
            .arg("-DateTimeOriginal")
            .arg("-CreateDate")
            .arg("-MediaCreateDate")
            .arg("-TrackCreateDate")
            .arg("-s3")
            .arg(file_path)
            .output()?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // If any date field has a value (non-empty line), the video has date metadata
        for line in stdout.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed != "0000:00:00 00:00:00" {
                return Ok(true);
            }
        }

        return Ok(false);
    }

    // For image files, use kamadak-exif (faster than calling exiftool)
    let file = fs::File::open(file_path)?;
    let mut bufreader = std::io::BufReader::new(&file);

    let exifreader = exif::Reader::new();
    let exif_data = match exifreader.read_from_container(&mut bufreader) {
        Ok(data) => data,
        Err(_) => return Ok(false), // No EXIF data means no date
    };

    // Check for common date/time fields
    let date_fields = [
        exif::Tag::DateTimeOriginal,
        exif::Tag::DateTime,
        exif::Tag::DateTimeDigitized,
    ];

    for tag in &date_fields {
        if exif_data.get_field(*tag, exif::In::PRIMARY).is_some() {
            return Ok(true);
        }
    }

    Ok(false)
}

fn update_exif_date(file_path: &Path, timestamp: i64) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::{DateTime, Utc};
    use std::process::Command;

    // Convert timestamp to datetime
    let dt = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .ok_or("Invalid timestamp")?;

    // Format as EXIF datetime string (YYYY:MM:DD HH:MM:SS)
    let exif_datetime = dt.format("%Y:%m:%d %H:%M:%S").to_string();

    // Use exiftool to write EXIF data
    // Check if exiftool is available
    let exiftool_check = Command::new("exiftool")
        .arg("-ver")
        .output();

    if exiftool_check.is_err() {
        return Err("exiftool not found. Please install exiftool to update EXIF data.".into());
    }

    // Build exiftool command with appropriate tags
    let mut cmd = Command::new("exiftool");
    cmd.arg("-overwrite_original")
        .arg(format!("-DateTimeOriginal={}", exif_datetime))
        .arg(format!("-DateTime={}", exif_datetime))
        .arg(format!("-CreateDate={}", exif_datetime));

    // For video files, also set video-specific date tags
    if is_video_file(file_path) {
        cmd.arg(format!("-MediaCreateDate={}", exif_datetime))
            .arg(format!("-MediaModifyDate={}", exif_datetime))
            .arg(format!("-TrackCreateDate={}", exif_datetime))
            .arg(format!("-TrackModifyDate={}", exif_datetime));
    }

    cmd.arg(file_path);

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("exiftool failed: {}", stderr).into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn test_get_base_media_path() {
        let json_path = Path::new("/test/IMG-20161219-WA0000.jpg.supplemental-metadata.json");
        let result = get_base_media_path(json_path).unwrap();
        assert_eq!(result, PathBuf::from("/test/IMG-20161219-WA0000.jpg"));
    }

    #[test]
    fn test_is_video_file() {
        assert!(is_video_file(Path::new("video.mp4")));
        assert!(is_video_file(Path::new("video.MOV")));
        assert!(is_video_file(Path::new("video.m4v")));
        assert!(is_video_file(Path::new("video.avi")));
        assert!(is_video_file(Path::new("video.mkv")));
        assert!(is_video_file(Path::new("video.webm")));
        assert!(is_video_file(Path::new("/path/to/video.MP4")));

        assert!(!is_video_file(Path::new("image.jpg")));
        assert!(!is_video_file(Path::new("image.png")));
        assert!(!is_video_file(Path::new("image.jpeg")));
        assert!(!is_video_file(Path::new("document.pdf")));
        assert!(!is_video_file(Path::new("noextension")));
    }

    #[test]
    fn test_integration_with_real_files() {
        // Check if exiftool is available, skip test if not
        if Command::new("exiftool").arg("-ver").output().is_err() {
            eprintln!("Skipping integration test: exiftool not installed");
            return;
        }

        // Setup: Create test directory in target
        let test_dir = PathBuf::from("target/test_data");
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).expect("Failed to clean test directory");
        }
        fs::create_dir_all(&test_dir).expect("Failed to create test directory");

        // Copy test files
        let source_media = Path::new("test/IMG-20161219-WA0000.jpg");
        let source_json = Path::new("test/IMG-20161219-WA0000.jpg.supplemental-metadata.json");
        let dest_media = test_dir.join("IMG-20161219-WA0000.jpg");
        let dest_json = test_dir.join("IMG-20161219-WA0000.jpg.supplemental-metadata.json");

        fs::copy(source_media, &dest_media).expect("Failed to copy media file");
        fs::copy(source_json, &dest_json).expect("Failed to copy JSON");

        // Remove EXIF data from the copied media file to simulate a file without dates
        let strip_output = Command::new("exiftool")
            .arg("-overwrite_original")
            .arg("-DateTimeOriginal=")
            .arg("-DateTime=")
            .arg("-CreateDate=")
            .arg(&dest_media)
            .output()
            .expect("Failed to strip EXIF data");

        assert!(
            strip_output.status.success(),
            "Failed to strip EXIF data: {}",
            String::from_utf8_lossy(&strip_output.stderr)
        );

        // Verify EXIF data is removed
        let has_date = has_exif_date(&dest_media).expect("Failed to check EXIF");
        assert!(!has_date, "Media file should not have EXIF date after stripping");

        // Run the processing function
        let result = process_metadata_file(&dest_json);
        assert!(result.is_ok(), "Processing failed: {:?}", result.err());
        assert_eq!(result.unwrap(), true, "Should have updated the media file");

        // Verify EXIF data was written
        let has_date_after = has_exif_date(&dest_media).expect("Failed to check EXIF after update");
        assert!(has_date_after, "Media file should have EXIF date after processing");

        // Verify the timestamp is correct by reading it
        let verify_output = Command::new("exiftool")
            .arg("-DateTimeOriginal")
            .arg("-s3")
            .arg(&dest_media)
            .output()
            .expect("Failed to read EXIF data");

        let datetime = String::from_utf8_lossy(&verify_output.stdout);
        let datetime = datetime.trim();

        // Expected timestamp from JSON: 1511480066 = 2017:11:23 23:34:26 UTC
        assert!(
            datetime.starts_with("2017:11:23"),
            "Expected date to start with 2017:11:23, got: {}",
            datetime
        );

        // Test that running again skips the file (already has date)
        let result_second = process_metadata_file(&dest_json);
        assert!(result_second.is_ok(), "Second processing failed: {:?}", result_second.err());
        assert_eq!(
            result_second.unwrap(),
            false,
            "Should have skipped the media file on second run"
        );

        // Cleanup
        fs::remove_dir_all(&test_dir).expect("Failed to cleanup test directory");
    }
}
