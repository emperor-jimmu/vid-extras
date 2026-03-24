// Models module - shared data structures and types

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
    /// Optional season number for series extras (None = series-level or movie)
    pub season_number: Option<u8>,
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
            write!(
                f,
                "Downloaded: {} -> {:?}",
                self.source.title, self.local_path
            )
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
    /// Content category for organization
    pub category: ContentCategory,
    /// Optional season number for series extras
    pub season_number: Option<u8>,
    /// Whether conversion succeeded
    pub success: bool,
    /// Error message if conversion failed
    pub error: Option<String>,
}

impl fmt::Display for ConversionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(
                f,
                "Converted: {:?} -> {:?}",
                self.input_path, self.output_path
            )
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

/// Discovery source that can be queried for extras
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
pub enum Source {
    /// TheMovieDB API
    Tmdb,
    /// Internet Archive
    Archive,
    /// Dailymotion video platform
    Dailymotion,
    /// YouTube
    Youtube,
    /// Vimeo video platform (opt-in)
    Vimeo,
    /// Bilibili video platform (opt-in)
    Bilibili,
}

impl Source {
    /// Returns the deduplication priority tier (1 = highest)
    pub fn tier(&self) -> u8 {
        match self {
            Source::Tmdb => 1,
            Source::Archive => 2,
            Source::Dailymotion => 2,
            Source::Youtube => 3,
            Source::Vimeo => 2,
            Source::Bilibili => 3,
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Tmdb => write!(f, "tmdb"),
            Source::Archive => write!(f, "archive"),
            Source::Dailymotion => write!(f, "dailymotion"),
            Source::Youtube => write!(f, "youtube"),
            Source::Vimeo => write!(f, "vimeo"),
            Source::Bilibili => write!(f, "bilibili"),
        }
    }
}

/// Returns the default set of discovery sources
pub fn default_sources() -> Vec<Source> {
    vec![
        Source::Tmdb,
        Source::Archive,
        Source::Dailymotion,
        Source::Youtube,
    ]
}

/// Type of content source
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    /// TheMovieDB API
    TMDB,
    /// Internet Archive
    ArchiveOrg,
    /// YouTube
    YouTube,
    /// TheTVDB API
    TheTVDB,
    /// Dailymotion video platform
    Dailymotion,
    /// KinoCheck (implicit TMDB fallback)
    KinoCheck,
    /// Vimeo video platform
    Vimeo,
    /// Bilibili video platform
    Bilibili,
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceType::TMDB => write!(f, "TMDB"),
            SourceType::ArchiveOrg => write!(f, "Archive.org"),
            SourceType::YouTube => write!(f, "YouTube"),
            SourceType::TheTVDB => write!(f, "TheTVDB"),
            SourceType::Dailymotion => write!(f, "Dailymotion"),
            SourceType::KinoCheck => write!(f, "KinoCheck"),
            SourceType::Vimeo => write!(f, "Vimeo"),
            SourceType::Bilibili => write!(f, "Bilibili"),
        }
    }
}

/// Content category for organizing extras
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentCategory {
    /// Movie trailers
    Trailer,
    /// Featurettes and EPK content
    Featurette,
    /// Behind-the-scenes footage
    BehindTheScenes,
    /// Deleted scenes
    DeletedScene,
    /// Cast and crew interviews
    Interview,
    /// Short films and animated shorts
    Short,
    /// Movie scene clips
    Clip,
    /// Full scenes from the movie
    Scene,
    /// Catch-all for uncategorized extras that don't fit other categories
    Extras,
}

impl fmt::Display for ContentCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentCategory::Trailer => write!(f, "Trailer"),
            ContentCategory::Featurette => write!(f, "Featurette"),
            ContentCategory::BehindTheScenes => write!(f, "Behind the Scenes"),
            ContentCategory::DeletedScene => write!(f, "Deleted Scene"),
            ContentCategory::Interview => write!(f, "Interview"),
            ContentCategory::Short => write!(f, "Short"),
            ContentCategory::Clip => write!(f, "Clip"),
            ContentCategory::Scene => write!(f, "Scene"),
            ContentCategory::Extras => write!(f, "Extras"),
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
            ContentCategory::Interview => "interviews",
            ContentCategory::Short => "shorts",
            ContentCategory::Clip => "clips",
            ContentCategory::Scene => "scenes",
            ContentCategory::Extras => "extras",
        }
    }
}

/// Media type classification for library items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    /// Movie file
    Movie,
    /// TV series with seasons
    Series,
    /// Unknown or unclassified
    Unknown,
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaType::Movie => write!(f, "Movie"),
            MediaType::Series => write!(f, "Series"),
            MediaType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Processing mode for media types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingMode {
    /// Process both movies and series
    Both,
    /// Process only movies
    MoviesOnly,
    /// Process only series
    SeriesOnly,
}

impl fmt::Display for ProcessingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessingMode::Both => write!(f, "Both"),
            ProcessingMode::MoviesOnly => write!(f, "Movies Only"),
            ProcessingMode::SeriesOnly => write!(f, "Series Only"),
        }
    }
}

/// Represents a TV series entry discovered during library scanning
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesEntry {
    /// Path to the series folder
    pub path: PathBuf,
    /// Series title extracted from folder name
    pub title: String,
    /// Optional release year extracted from folder name
    pub year: Option<u16>,
    /// Whether a done marker file exists in this folder
    pub has_done_marker: bool,
    /// List of season numbers found in the folder
    pub seasons: Vec<u8>,
}

impl fmt::Display for SeriesEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(year) = self.year {
            write!(f, "{} ({})", self.title, year)
        } else {
            write!(f, "{}", self.title)
        }
    }
}

/// Represents an extra video for a TV series
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesExtra {
    /// Series identifier (for caching and organization)
    pub series_id: String,
    /// Optional season number (None = series-level extra)
    pub season_number: Option<u8>,
    /// Content category
    pub category: ContentCategory,
    /// Title/description
    pub title: String,
    /// Video URL
    pub url: String,
    /// Source type (TMDB, YouTube, etc.)
    pub source_type: SourceType,
    /// Local path after download (optional)
    pub local_path: Option<PathBuf>,
}

impl fmt::Display for SeriesExtra {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let season_str = self
            .season_number
            .map(|s| format!("S{:02}", s))
            .unwrap_or_else(|| "Series".to_string());
        write!(
            f,
            "{} - {} - {} ({})",
            season_str, self.title, self.category, self.source_type
        )
    }
}

impl From<SeriesExtra> for VideoSource {
    fn from(extra: SeriesExtra) -> Self {
        VideoSource {
            url: extra.url,
            source_type: extra.source_type,
            category: extra.category,
            title: extra.title,
            season_number: extra.season_number,
        }
    }
}

/// Represents a Season 0 special episode
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialEpisode {
    /// Episode number within Season 0
    pub episode_number: u8,
    /// Episode title
    pub title: String,
    /// Optional air date
    pub air_date: Option<String>,
    /// Optional video URL
    pub url: Option<String>,
    /// Local path after download (optional)
    pub local_path: Option<PathBuf>,
    /// Optional TheTVDB episode ID
    pub tvdb_id: Option<u64>,
}

impl fmt::Display for SpecialEpisode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S00E{:02} - {}", self.episode_number, self.title)
    }
}

/// Hardware acceleration type for video conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareAccel {
    /// NVIDIA NVENC encoder
    Nvenc,
    /// Intel Quick Sync Video
    Qsv,
    /// Apple VideoToolbox (M-series chips)
    VideoToolbox,
    /// Software encoding (no hardware acceleration)
    Software,
}

impl fmt::Display for HardwareAccel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HardwareAccel::Nvenc => write!(f, "NVIDIA NVENC"),
            HardwareAccel::Qsv => write!(f, "Intel Quick Sync"),
            HardwareAccel::VideoToolbox => write!(f, "Apple VideoToolbox"),
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
    fn test_series_entry_display_with_year() {
        let entry = SeriesEntry {
            path: PathBuf::from("/series/Breaking Bad (2008)"),
            title: "Breaking Bad".to_string(),
            year: Some(2008),
            has_done_marker: false,
            seasons: vec![1, 2, 3, 4, 5],
        };
        assert_eq!(entry.to_string(), "Breaking Bad (2008)");
    }

    #[test]
    fn test_series_entry_display_without_year() {
        let entry = SeriesEntry {
            path: PathBuf::from("/series/Breaking Bad"),
            title: "Breaking Bad".to_string(),
            year: None,
            has_done_marker: false,
            seasons: vec![1, 2, 3, 4, 5],
        };
        assert_eq!(entry.to_string(), "Breaking Bad");
    }

    #[test]
    fn test_series_extra_display() {
        let extra = SeriesExtra {
            series_id: "bb".to_string(),
            season_number: Some(1),
            category: ContentCategory::BehindTheScenes,
            title: "Making of Pilot".to_string(),
            url: "https://example.com/video".to_string(),
            source_type: SourceType::YouTube,
            local_path: None,
        };
        assert_eq!(
            extra.to_string(),
            "S01 - Making of Pilot - Behind the Scenes (YouTube)"
        );
    }

    #[test]
    fn test_series_extra_display_series_level() {
        let extra = SeriesExtra {
            series_id: "bb".to_string(),
            season_number: None,
            category: ContentCategory::Trailer,
            title: "Series Trailer".to_string(),
            url: "https://example.com/video".to_string(),
            source_type: SourceType::TMDB,
            local_path: None,
        };
        assert_eq!(
            extra.to_string(),
            "Series - Series Trailer - Trailer (TMDB)"
        );
    }

    #[test]
    fn test_special_episode_display() {
        let episode = SpecialEpisode {
            episode_number: 5,
            title: "Holiday Special".to_string(),
            air_date: Some("2010-12-25".to_string()),
            url: None,
            local_path: None,
            tvdb_id: Some(123456),
        };
        assert_eq!(episode.to_string(), "S00E05 - Holiday Special");
    }

    #[test]
    fn test_media_type_display() {
        assert_eq!(MediaType::Movie.to_string(), "Movie");
        assert_eq!(MediaType::Series.to_string(), "Series");
        assert_eq!(MediaType::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_processing_mode_display() {
        assert_eq!(ProcessingMode::Both.to_string(), "Both");
        assert_eq!(ProcessingMode::MoviesOnly.to_string(), "Movies Only");
        assert_eq!(ProcessingMode::SeriesOnly.to_string(), "Series Only");
    }

    #[test]
    fn test_content_category_subdirectory() {
        assert_eq!(ContentCategory::Trailer.subdirectory(), "trailers");
        assert_eq!(ContentCategory::Featurette.subdirectory(), "featurettes");
        assert_eq!(
            ContentCategory::BehindTheScenes.subdirectory(),
            "behind the scenes"
        );
        assert_eq!(
            ContentCategory::DeletedScene.subdirectory(),
            "deleted scenes"
        );
        assert_eq!(ContentCategory::Interview.subdirectory(), "interviews");
        assert_eq!(ContentCategory::Short.subdirectory(), "shorts");
        assert_eq!(ContentCategory::Clip.subdirectory(), "clips");
        assert_eq!(ContentCategory::Scene.subdirectory(), "scenes");
        assert_eq!(ContentCategory::Extras.subdirectory(), "extras");
    }

    #[test]
    fn test_content_category_display_new_variants() {
        assert_eq!(format!("{}", ContentCategory::Trailer), "Trailer");
        assert_eq!(format!("{}", ContentCategory::Featurette), "Featurette");
        assert_eq!(
            format!("{}", ContentCategory::BehindTheScenes),
            "Behind the Scenes"
        );
        assert_eq!(format!("{}", ContentCategory::DeletedScene), "Deleted Scene");
        assert_eq!(format!("{}", ContentCategory::Interview), "Interview");
        assert_eq!(format!("{}", ContentCategory::Short), "Short");
        assert_eq!(format!("{}", ContentCategory::Clip), "Clip");
        assert_eq!(format!("{}", ContentCategory::Scene), "Scene");
        assert_eq!(format!("{}", ContentCategory::Extras), "Extras");
    }

    #[test]
    fn test_source_tier() {
        assert_eq!(Source::Tmdb.tier(), 1);
        assert_eq!(Source::Archive.tier(), 2);
        assert_eq!(Source::Dailymotion.tier(), 2);
        assert_eq!(Source::Youtube.tier(), 3);
        assert_eq!(Source::Vimeo.tier(), 2);
        assert_eq!(Source::Bilibili.tier(), 3);
    }

    #[test]
    fn test_default_sources() {
        let sources = default_sources();
        assert_eq!(sources.len(), 4);
        assert!(sources.contains(&Source::Tmdb));
        assert!(sources.contains(&Source::Archive));
        assert!(sources.contains(&Source::Dailymotion));
        assert!(sources.contains(&Source::Youtube));
        assert!(!sources.contains(&Source::Vimeo));
        assert!(!sources.contains(&Source::Bilibili));
    }

    #[test]
    fn test_source_type_new_variants_display() {
        assert_eq!(SourceType::Dailymotion.to_string(), "Dailymotion");
        assert_eq!(SourceType::KinoCheck.to_string(), "KinoCheck");
        assert_eq!(SourceType::Vimeo.to_string(), "Vimeo");
        assert_eq!(SourceType::Bilibili.to_string(), "Bilibili");
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

    // Feature: tv-series-extras, Property 4: SeriesExtra Serialization Round-Trip
    // Validates: Requirements 2.6
    proptest! {
        #[test]
        fn prop_series_extra_serialization_round_trip(
            series_id in "[a-z0-9_]{1,20}",
            season_number in proptest::option::of(1u8..=99u8),
            title in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            url in "https://[a-z0-9.]{5,30}/[a-z0-9]{5,20}",
            local_path_str in proptest::option::of("/tmp/[a-z0-9]{5,20}\\.mp4")
        ) {
            let category = ContentCategory::Featurette;
            let source_type = SourceType::YouTube;
            let local_path = local_path_str.map(PathBuf::from);

            let extra = SeriesExtra {
                series_id: series_id.clone(),
                season_number,
                category,
                title: title.clone(),
                url: url.clone(),
                source_type,
                local_path: local_path.clone(),
            };

            // Serialize to JSON
            let json = serde_json::to_string(&extra).unwrap();

            // Deserialize from JSON
            let deserialized: SeriesExtra = serde_json::from_str(&json).unwrap();

            // Verify round-trip preserves all fields
            prop_assert_eq!(&extra.series_id, &deserialized.series_id);
            prop_assert_eq!(extra.season_number, deserialized.season_number);
            prop_assert_eq!(extra.category, deserialized.category);
            prop_assert_eq!(&extra.title, &deserialized.title);
            prop_assert_eq!(&extra.url, &deserialized.url);
            prop_assert_eq!(extra.source_type, deserialized.source_type);
            prop_assert_eq!(&extra.local_path, &deserialized.local_path);
        }
    }
}
