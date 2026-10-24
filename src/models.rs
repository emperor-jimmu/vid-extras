// Models module - shared data structures and types

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Subtitle file extensions recognised throughout the pipeline.
pub const SUBTITLE_EXTENSIONS: &[&str] =
    &["vtt", "srt", "ass", "ttml", "srv3", "srv2", "srv1", "json3"];

/// Represents a movie entry discovered during library scanning
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovieEntry {
    /// Path to the movie folder
    pub path: PathBuf,
    /// Movie title extracted from folder name
    pub title: String,
    /// Release year extracted from folder name
    pub year: u16,
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
    /// Duration in seconds, populated from API metadata where available
    pub duration_secs: Option<u32>,
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
    /// Subtitle files downloaded alongside the video (best-effort, may be empty)
    pub subtitle_paths: Vec<PathBuf>,
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

/// Returns all available discovery sources (used by --all flag)
pub fn all_sources() -> Vec<Source> {
    vec![
        Source::Tmdb,
        Source::Archive,
        Source::Dailymotion,
        Source::Youtube,
        Source::Vimeo,
        Source::Bilibili,
    ]
}

/// Type of content source
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl SourceType {
    /// Returns the deduplication priority tier (1 = highest).
    /// Mirrors `Source::tier()` for use in deduplication logic.
    pub fn tier(&self) -> u8 {
        match self {
            SourceType::TMDB => 1,
            SourceType::KinoCheck => 1,
            SourceType::TheTVDB => 1,
            SourceType::ArchiveOrg => 2,
            SourceType::Dailymotion => 2,
            SourceType::Vimeo => 2,
            SourceType::YouTube => 3,
            SourceType::Bilibili => 3,
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
        };
        assert_eq!(entry.to_string(), "The Matrix (1999)");
    }

    #[test]
    fn test_series_entry_display_with_year() {
        let entry = SeriesEntry {
            path: PathBuf::from("/series/Breaking Bad (2008)"),
            title: "Breaking Bad".to_string(),
            year: Some(2008),
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
            seasons: vec![1, 2, 3, 4, 5],
        };
        assert_eq!(entry.to_string(), "Breaking Bad");
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
        assert_eq!(
            format!("{}", ContentCategory::DeletedScene),
            "Deleted Scene"
        );
        assert_eq!(format!("{}", ContentCategory::Interview), "Interview");
        assert_eq!(format!("{}", ContentCategory::Short), "Short");
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
    fn test_source_type_tier_matches_source_tier() {
        // Verify SourceType::tier() stays in sync with Source::tier()
        // for all user-selectable source types.
        assert_eq!(SourceType::TMDB.tier(), Source::Tmdb.tier());
        assert_eq!(SourceType::ArchiveOrg.tier(), Source::Archive.tier());
        assert_eq!(SourceType::Dailymotion.tier(), Source::Dailymotion.tier());
        assert_eq!(SourceType::YouTube.tier(), Source::Youtube.tier());
        assert_eq!(SourceType::Vimeo.tier(), Source::Vimeo.tier());
        assert_eq!(SourceType::Bilibili.tier(), Source::Bilibili.tier());
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
