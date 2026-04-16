// Bilibili content discoverer - searches Bilibili via yt-dlp for movie/series extras.
// Bilibili is a Chinese video platform that hosts movie extras, documentaries, and interviews.

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceType, VideoSource};
use log::{debug, info};
use std::process::Command;

use super::ContentDiscoverer;
use super::title_matching;

/// Bilibili content discoverer
/// Uses yt-dlp to search Bilibili (Chinese: 哔哩哔哩, aka "B站")
pub struct BilibiliDiscoverer {}

impl BilibiliDiscoverer {
    pub fn new() -> Self {
        Self {}
    }

    fn build_search_queries(title: &str, year: u16) -> Vec<(String, ContentCategory)> {
        vec![
            (
                format!("{} {} 删除片段", title, year),
                ContentCategory::DeletedScene,
            ),
            (
                format!("{} {} 幕后花絮", title, year),
                ContentCategory::BehindTheScenes,
            ),
            (
                format!("{} {} 预告片", title, year),
                ContentCategory::Trailer,
            ),
            (
                format!("{} {} 访谈", title, year),
                ContentCategory::Interview,
            ),
            (
                format!("{} {} 纪录片", title, year),
                ContentCategory::Featurette,
            ),
            (
                format!("{} {} 花絮", title, year),
                ContentCategory::BehindTheScenes,
            ),
        ]
    }

    fn run_search(query: &str) -> Result<String, DiscoveryError> {
        let output = Command::new("yt-dlp")
            .args([
                "--dump-json",
                "--flat-playlist",
                "--no-download",
                &format!("bilisearch5:{}", query),
            ])
            .output()
            .map_err(|e| {
                debug!("Bilibili search command failed: {}", e);
                DiscoveryError::YtDlpError(format!("yt-dlp failed: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!("Bilibili search stderr: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn parse_json_lines(json_output: &str) -> Vec<(String, u32, String)> {
        let mut results = Vec::new();

        for line in json_output.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(video) = serde_json::from_str::<serde_json::Value>(line) {
                let title = video["title"].as_str().unwrap_or("").to_string();
                let duration = video["duration"].as_u64().unwrap_or(0) as u32;
                let url = video["url"].as_str().unwrap_or("").to_string();
                results.push((title, duration, url));
            }
        }

        results
    }
}

impl Default for BilibiliDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(async_fn_in_trait)]
impl ContentDiscoverer for BilibiliDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!("Searching Bilibili for: {} ({})", movie.title, movie.year);

        let queries = Self::build_search_queries(&movie.title, movie.year);
        let mut all_sources = Vec::new();

        for (query, _category) in &queries {
            match Self::run_search(query) {
                Ok(json_output) => {
                    let videos = Self::parse_json_lines(&json_output);
                    for (title, duration, url) in videos {
                        // Duration filter: 30s–40min (exclude full films and shorts)
                        if !(30..=2400).contains(&duration) {
                            continue;
                        }

                        let category = title_matching::infer_category_from_title(&title)
                            .unwrap_or(ContentCategory::Extras);

                        if url.contains("bilibili.com") {
                            all_sources.push(VideoSource {
                                url: url.clone(),
                                title,
                                category,
                                source_type: SourceType::Bilibili,
                                season_number: None,
                                duration_secs: Some(duration),
                            });
                        }
                    }
                }
                Err(e) => {
                    debug!("Bilibili query '{}' failed: {}", query, e);
                }
            }
        }

        let mut seen = std::collections::HashSet::new();
        all_sources.retain(|s| seen.insert(s.url.clone()));

        info!("Found {} sources from Bilibili", all_sources.len());
        Ok(all_sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_search_queries() {
        let queries = BilibiliDiscoverer::build_search_queries("Inception", 2010);
        assert!(!queries.is_empty());
        assert!(
            queries
                .iter()
                .all(|(q, _)| q.contains("Inception") && q.contains("2010"))
        );
    }

    #[test]
    fn test_parse_json_lines_empty() {
        let result = BilibiliDiscoverer::parse_json_lines("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_json_lines_invalid() {
        let result = BilibiliDiscoverer::parse_json_lines("not json\nmore text");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_json_lines_valid() {
        let json = r#"{"title":"Test Video","duration":300,"url":"https://www.bilibili.com/video/test123"}"#;
        let result = BilibiliDiscoverer::parse_json_lines(json);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "Test Video");
        assert_eq!(result[0].1, 300);
    }
}
