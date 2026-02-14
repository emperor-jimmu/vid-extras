use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Represents a movie entry discovered during library scanning
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovieEntry {
    /// Path to the movie folder
    pub path: PathBuf,
    /// Movie title extracted from folder name
    pub title: String,
    /// Release year extracted from folder name
    pub year: u16,
    /// Whether a done marker file exists in this folder
    pub has_done_marker: bool,
}

impl fmt::Display for MovieEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.title, self.year)
    }
}

/// Represents a video source discovered from content providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoSource {
    /// URL of the video
    pub url: String,
    /// Type of source (TMDB, Archive.org, YouTube)
    pub source_type: SourceType,
    /// Content category for organization
    pub category: ContentCategory,
    /// Title/description of the video
    pub title: String,
}

impl fmt::Display for VideoSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} - {} ({})",
            self.title, self.category, self.source_type
        )
    }
}

/// Result of a download operation
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// Original video source
    pub source: VideoSource,
    /// Local path where file was downloaded
    pub local_path: PathBuf,
    /// Whether download succeeded
    pub success: bool,
    /// Error message if download failed
    pub error: Option<String>,
}

impl fmt::Display for DownloadResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(f, "Downloaded: {} -> {:?}", self.source.title, self.local_path)
        } else {
            write!(
                f,
                "Failed: {} - {}",
                self.source.title,
                self.error.as_deref().unwrap_or("Unknown error")
            )
        }
    }
}

/// Result of a video conversion operation
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// Path to input file
    pub input_path: PathBuf,
    /// Path to output file
    pub output_path: PathBuf,
    /// Whether conversion succeeded
    pub success: bool,
    /// Error message if conversion failed
    pub error: Option<String>,
}

impl fmt::Display for ConversionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(f, "Converted: {:?} -> {:?}", self.input_path, self.output_path)
        } else {
            write!(
                f,
                "Conversion failed: {:?} - {}",
                self.input_path,
                self.error.as_deref().unwrap_or("Unknown error")
            )
        }
    }
}

/// Done marker file content indicating completed processing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneMarker {
    /// ISO 8601 timestamp when processing finished
    pub finished_at: String,
    /// Version of the tool that created this marker
    pub version: String,
}

impl fmt::Display for DoneMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Completed at {} (v{})", self.finished_at, self.version)
    }
}

/// Source mode configuration for content discovery
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    /// Query all sources (TMDB, Archive.org, YouTube)
    All,
    /// Query only YouTube
    YoutubeOnly,
}

impl fmt::Display for SourceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceMode::All => write!(f, "All Sources"),
            SourceMode::YoutubeOnly => write!(f, "YouTube Only"),
        }
    }
}

/// Type of content source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    /// TheMovieDB API
    TMDB,
    /// Internet Archive
    ArchiveOrg,
    /// YouTube
    YouTube,
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceType::TMDB => write!(f, "TMDB"),
            SourceType::ArchiveOrg => write!(f, "Archive.org"),
            SourceType::YouTube => write!(f, "YouTube"),
        }
    }
}

/// Content category for organizing extras
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentCategory {
    /// Movie trailers
    Trailer,
    /// Featurettes and EPK content
    Featurette,
    /// Behind-the-scenes footage
    BehindTheScenes,
    /// Deleted scenes
    DeletedScene,
}

impl fmt::Display for ContentCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentCategory::Trailer => write!(f, "Trailer"),
            ContentCategory::Featurette => write!(f, "Featurette"),
            ContentCategory::BehindTheScenes => write!(f, "Behind the Scenes"),
            ContentCategory::DeletedScene => write!(f, "Deleted Scene"),
        }
    }
}

impl ContentCategory {
    /// Get the subdirectory name for this category
    pub fn subdirectory(&self) -> &'static str {
        match self {
            ContentCategory::Trailer => "trailers",
            ContentCategory::Featurette => "featurettes",
            ContentCategory::BehindTheScenes => "behind the scenes",
            ContentCategory::DeletedScene => "deleted scenes",
        }
    }
}

/// Hardware acceleration type for video conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareAccel {
    /// NVIDIA NVENC encoder
    Nvenc,
    /// Intel Quick Sync Video
    Qsv,
    /// Software encoding (no hardware acceleration)
    Software,
}

impl fmt::Display for HardwareAccel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HardwareAccel::Nvenc => write!(f, "NVIDIA NVENC"),
            HardwareAccel::Qsv => write!(f, "Intel Quick Sync"),
            HardwareAccel::Software => write!(f, "Software"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movie_entry_display() {
        let entry = MovieEntry {
            path: PathBuf::from("/movies/The Matrix (1999)"),
            title: "The Matrix".to_string(),
            year: 1999,
            has_done_marker: false,
        };
        assert_eq!(entry.to_string(), "The Matrix (1999)");
    }

    #[test]
    fn test_content_category_subdirectory() {
        assert_eq!(ContentCategory::Trailer.subdirectory(), "trailers");
        assert_eq!(ContentCategory::Featurette.subdirectory(), "featurettes");
        assert_eq!(ContentCategory::BehindTheScenes.subdirectory(), "behind the scenes");
        assert_eq!(ContentCategory::DeletedScene.subdirectory(), "deleted scenes");
    }

    #[test]
    fn test_source_mode_display() {
        assert_eq!(SourceMode::All.to_string(), "All Sources");
        assert_eq!(SourceMode::YoutubeOnly.to_string(), "YouTube Only");
    }

    #[test]
    fn test_hardware_accel_display() {
        assert_eq!(HardwareAccel::Nvenc.to_string(), "NVIDIA NVENC");
        assert_eq!(HardwareAccel::Qsv.to_string(), "Intel Quick Sync");
        assert_eq!(HardwareAccel::Software.to_string(), "Software");
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: extras-fetcher, Property 2: Done Marker Round-Trip
    // Validates: Requirements 2.2
    proptest! {
        #[test]
        fn prop_done_marker_round_trip(
            year in 2000i32..2100i32,
            month in 1u32..=12u32,
            day in 1u32..=28u32,  // Use 28 to avoid invalid dates
            hour in 0u32..24u32,
            minute in 0u32..60u32,
            second in 0u32..60u32,
            version in "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}"
        ) {
            // Create ISO 8601 timestamp
            let timestamp = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                year, month, day, hour, minute, second
            );
            
            let marker = DoneMarker {
                finished_at: timestamp.clone(),
                version: version.clone(),
            };
            
            // Serialize to JSON
            let json = serde_json::to_string(&marker).unwrap();
            
            // Deserialize from JSON
            let deserialized: DoneMarker = serde_json::from_str(&json).unwrap();
            
            // Verify round-trip preserves data
            prop_assert_eq!(&marker.finished_at, &deserialized.finished_at);
            prop_assert_eq!(&marker.version, &deserialized.version);
            
            // Verify ISO 8601 format is valid
            // The timestamp should be parseable as RFC 3339 (which is compatible with ISO 8601)
            prop_assert!(
                chrono::DateTime::parse_from_rfc3339(&deserialized.finished_at).is_ok(),
                "Timestamp should be valid ISO 8601/RFC 3339 format: {}",
                deserialized.finished_at
            );
        }
    }
}
