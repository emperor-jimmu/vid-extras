//! Done marker: the `done.ext` file that records a library item as finished.
//!
//! One module owns the filename, the payload, the validity rule and the removal rule.
//! A marker that cannot be read or parsed is not a marker, so a corrupt file causes
//! reprocessing rather than a permanent skip.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// The marker file name. Jellyfin ignores it; only this tool reads it.
pub const DONE_MARKER_FILENAME: &str = "done.ext";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneMarker {
    /// ISO 8601 timestamp when processing finished
    pub finished_at: String,
    /// Version of the tool that created this marker
    pub version: String,
}

impl Default for DoneMarker {
    fn default() -> Self {
        Self::new()
    }
}

impl DoneMarker {
    pub fn new() -> Self {
        Self {
            finished_at: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn path(dir: &Path) -> PathBuf {
        dir.join(DONE_MARKER_FILENAME)
    }

    /// True only when the marker exists and parses. Unreadable or malformed markers
    /// mean "not done".
    pub fn exists(dir: &Path) -> bool {
        std::fs::read_to_string(Self::path(dir))
            .ok()
            .and_then(|content| serde_json::from_str::<Self>(&content).ok())
            .is_some()
    }

    /// Write a fresh marker for `dir`.
    pub async fn write(dir: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(&Self::new())
            .map_err(|e| io::Error::other(format!("failed to serialize done marker: {}", e)))?;
        tokio::fs::write(Self::path(dir), json).await
    }

    /// Remove the marker. Missing marker is success: reprocessing is idempotent.
    pub async fn remove(dir: &Path) -> io::Result<()> {
        match tokio::fs::remove_file(Self::path(dir)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_marker_is_not_done() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(!DoneMarker::exists(dir.path()));
    }

    #[tokio::test]
    async fn written_marker_round_trips() {
        let dir = tempfile::TempDir::new().unwrap();
        DoneMarker::write(dir.path()).await.unwrap();
        assert!(DoneMarker::exists(dir.path()));

        let content = std::fs::read_to_string(DoneMarker::path(dir.path())).unwrap();
        let marker: DoneMarker = serde_json::from_str(&content).unwrap();
        assert!(!marker.finished_at.is_empty());
        assert_eq!(marker.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn malformed_marker_is_not_done() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(DoneMarker::path(dir.path()), "not json").unwrap();
        assert!(!DoneMarker::exists(dir.path()));
    }

    #[tokio::test]
    async fn remove_is_idempotent() {
        let dir = tempfile::TempDir::new().unwrap();
        DoneMarker::write(dir.path()).await.unwrap();
        DoneMarker::remove(dir.path()).await.unwrap();
        assert!(!DoneMarker::exists(dir.path()));
        DoneMarker::remove(dir.path()).await.unwrap();
    }

    // Feature: extras-fetcher, Property 2: Done Marker Round-Trip
    proptest::proptest! {
        #[test]
        fn prop_done_marker_round_trip(
            year in 2000i32..2100i32,
            month in 1u32..=12u32,
            day in 1u32..=28u32,
            hour in 0u32..24u32,
            minute in 0u32..60u32,
            second in 0u32..60u32,
            version in "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}"
        ) {
            let timestamp = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                year, month, day, hour, minute, second
            );
            let marker = DoneMarker {
                finished_at: timestamp.clone(),
                version: version.clone(),
            };

            let json = serde_json::to_string(&marker).unwrap();
            let deserialized: DoneMarker = serde_json::from_str(&json).unwrap();

            proptest::prop_assert_eq!(&marker.finished_at, &deserialized.finished_at);
            proptest::prop_assert_eq!(&marker.version, &deserialized.version);
            proptest::prop_assert!(
                chrono::DateTime::parse_from_rfc3339(&deserialized.finished_at).is_ok(),
                "Timestamp should be valid ISO 8601/RFC 3339 format: {}",
                deserialized.finished_at
            );
        }
    }
}
