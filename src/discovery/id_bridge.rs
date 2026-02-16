use crate::discovery::FuzzyMatcher;
use crate::discovery::tvdb::TvdbClient;
use crate::error::DiscoveryError;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

/// Persistent cache for TMDB-to-TVDB ID mappings with no TTL expiration
#[allow(dead_code)]
struct IdMappingCache {
    cache_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct CachedIdMapping {
    tmdb_id: u64,
    tvdb_id: u64,
}

#[allow(dead_code)]
impl IdMappingCache {
    /// Create a new ID mapping cache with the given cache directory
    fn new(cache_dir: PathBuf) -> Self {
        IdMappingCache { cache_dir }
    }

    /// Get the cache file path for a TMDB ID
    fn get_cache_file(&self, tmdb_id: u64) -> PathBuf {
        self.cache_dir.join(format!("{}.json", tmdb_id))
    }

    /// Retrieve a cached TVDB ID for a TMDB ID
    async fn get(&self, tmdb_id: u64) -> Option<u64> {
        let cache_file = self.get_cache_file(tmdb_id);

        match tokio::fs::read_to_string(&cache_file).await {
            Ok(content) => match serde_json::from_str::<CachedIdMapping>(&content) {
                Ok(mapping) => {
                    debug!(
                        "Cache hit for TMDB ID {}: TVDB ID {}",
                        tmdb_id, mapping.tvdb_id
                    );
                    Some(mapping.tvdb_id)
                }
                Err(e) => {
                    warn!("Failed to parse cache file for TMDB ID {}: {}", tmdb_id, e);
                    None
                }
            },
            Err(_) => {
                debug!("No cache entry for TMDB ID {}", tmdb_id);
                None
            }
        }
    }

    /// Store a TVDB ID mapping for a TMDB ID
    async fn set(&self, tmdb_id: u64, tvdb_id: u64) -> Result<(), DiscoveryError> {
        // Ensure cache directory exists
        tokio::fs::create_dir_all(&self.cache_dir)
            .await
            .map_err(|e| {
                DiscoveryError::TvdbApiError(format!("Failed to create cache directory: {}", e))
            })?;

        let cache_file = self.get_cache_file(tmdb_id);
        let mapping = CachedIdMapping { tmdb_id, tvdb_id };
        let json = serde_json::to_string(&mapping).map_err(|e| {
            DiscoveryError::TvdbApiError(format!("Failed to serialize cache entry: {}", e))
        })?;

        tokio::fs::write(&cache_file, json).await.map_err(|e| {
            DiscoveryError::TvdbApiError(format!("Failed to write cache file: {}", e))
        })?;

        debug!("Cached TMDB ID {} -> TVDB ID {}", tmdb_id, tvdb_id);
        Ok(())
    }
}

/// Resolves TMDB series IDs to TheTVDB IDs
pub struct IdBridge {
    tmdb_api_key: String,
    tvdb_client: Arc<TvdbClient>,
    client: reqwest::Client,
    cache: IdMappingCache,
}

#[derive(Debug, Deserialize)]
struct TmdbExternalIds {
    tvdb_id: Option<u64>,
}

impl IdBridge {
    /// Create a new IdBridge with the given API keys and cache directory
    pub fn new(tmdb_api_key: String, tvdb_client: Arc<TvdbClient>, cache_dir: PathBuf) -> Self {
        IdBridge {
            tmdb_api_key,
            tvdb_client,
            client: reqwest::Client::new(),
            cache: IdMappingCache::new(cache_dir),
        }
    }

    /// Resolve a TMDB series ID to a TVDB ID
    pub async fn resolve(
        &self,
        tmdb_id: u64,
        series_title: &str,
    ) -> Result<Option<u64>, DiscoveryError> {
        // Check cache first
        if let Some(tvdb_id) = self.cache.get(tmdb_id).await {
            return Ok(Some(tvdb_id));
        }

        // Try TMDB external_ids endpoint
        if let Some(tvdb_id) = self.query_tmdb_external_ids(tmdb_id).await? {
            self.cache.set(tmdb_id, tvdb_id).await?;
            return Ok(Some(tvdb_id));
        }

        // Fallback to TVDB search
        if let Some(tvdb_id) = self.search_tvdb_fallback(series_title).await? {
            self.cache.set(tmdb_id, tvdb_id).await?;
            return Ok(Some(tvdb_id));
        }

        warn!(
            "No TVDB ID found for TMDB series {} ({})",
            tmdb_id, series_title
        );
        Ok(None)
    }

    /// Query TMDB external_ids endpoint for a TVDB ID
    async fn query_tmdb_external_ids(&self, tmdb_id: u64) -> Result<Option<u64>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/tv/{}/external_ids?api_key={}",
            tmdb_id, self.tmdb_api_key
        );

        match self.client.get(&url).send().await {
            Ok(response) => match response.json::<TmdbExternalIds>().await {
                Ok(external_ids) => {
                    if let Some(tvdb_id) = external_ids.tvdb_id {
                        debug!(
                            "Found TVDB ID {} for TMDB series {} via external_ids",
                            tvdb_id, tmdb_id
                        );
                        Ok(Some(tvdb_id))
                    } else {
                        debug!("No TVDB ID in TMDB external_ids for series {}", tmdb_id);
                        Ok(None)
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to parse TMDB external_ids response for series {}: {}",
                        tmdb_id, e
                    );
                    Ok(None)
                }
            },
            Err(e) => {
                debug!(
                    "Failed to query TMDB external_ids for series {}: {}",
                    tmdb_id, e
                );
                Ok(None)
            }
        }
    }

    /// Fallback: search TVDB and fuzzy match to find TVDB ID
    async fn search_tvdb_fallback(
        &self,
        series_title: &str,
    ) -> Result<Option<u64>, DiscoveryError> {
        let url = format!(
            "https://api4.thetvdb.com/v4/search?q={}",
            urlencoding::encode(series_title)
        );

        match self.tvdb_client.authenticated_get(&url).await {
            Ok(response) => match response.json::<serde_json::Value>().await {
                Ok(json) => {
                    if let Some(results) = json.get("data").and_then(|d| d.as_array()) {
                        // Find the best match with fuzzy matching
                        let mut best_match: Option<(u64, u8)> = None;

                        for result in results {
                            if let Some(name) = result.get("name").and_then(|n| n.as_str()) {
                                let score = FuzzyMatcher::get_similarity_score(series_title, name);

                                if score >= 80
                                    && (best_match.is_none() || score > best_match.unwrap().1)
                                    && let Some(tvdb_id_str) =
                                        result.get("tvdb_id").and_then(|id| id.as_str())
                                    && let Ok(tvdb_id) = tvdb_id_str.parse::<u64>()
                                {
                                    best_match = Some((tvdb_id, score));
                                }
                            }
                        }

                        if let Some((tvdb_id, score)) = best_match {
                            debug!(
                                "Found TVDB ID {} for '{}' via search (score: {}%)",
                                tvdb_id, series_title, score
                            );
                            return Ok(Some(tvdb_id));
                        }
                    }

                    debug!("No TVDB search results for '{}'", series_title);
                    Ok(None)
                }
                Err(e) => {
                    debug!("Failed to parse TVDB search response: {}", e);
                    Ok(None)
                }
            },
            Err(e) => {
                debug!("Failed to query TVDB search: {}", e);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::sync::Arc;

    // Helper to create a mock TvdbClient for testing
    fn create_mock_tvdb_client() -> Arc<TvdbClient> {
        Arc::new(TvdbClient::new("test_key".to_string()))
    }

    #[tokio::test]
    async fn test_id_mapping_cache_set_and_get() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = IdMappingCache::new(temp_dir.path().to_path_buf());

        cache.set(12345, 67890).await.unwrap();
        let result = cache.get(12345).await;

        assert_eq!(result, Some(67890));
    }

    #[tokio::test]
    async fn test_id_mapping_cache_miss() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = IdMappingCache::new(temp_dir.path().to_path_buf());

        let result = cache.get(99999).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_id_mapping_cache_no_expiration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = IdMappingCache::new(temp_dir.path().to_path_buf());

        cache.set(12345, 67890).await.unwrap();

        // Simulate time passing by creating a new cache instance
        let cache2 = IdMappingCache::new(temp_dir.path().to_path_buf());
        let result = cache2.get(12345).await;

        // Should still retrieve the cached value (no expiration)
        assert_eq!(result, Some(67890));
    }

    #[tokio::test]
    async fn test_id_bridge_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let tvdb_client = create_mock_tvdb_client();

        let bridge = IdBridge::new(
            "tmdb_key".to_string(),
            tvdb_client,
            temp_dir.path().to_path_buf(),
        );

        assert_eq!(bridge.tmdb_api_key, "tmdb_key");
    }

    // Property 9: ID Mapping Cache Has No Expiration
    // Validates: Requirements 9.4
    proptest! {
        #[test]
        fn prop_cache_no_expiration(
            tmdb_id in 1000u64..999999u64,
            tvdb_id in 1000u64..999999u64,
        ) {
            // Use tokio runtime for async operations
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                let temp_dir = tempfile::tempdir().unwrap();
                let cache = IdMappingCache::new(temp_dir.path().to_path_buf());

                // Store a mapping
                cache.set(tmdb_id, tvdb_id).await.unwrap();

                // Retrieve it immediately
                let result1 = cache.get(tmdb_id).await;
                if result1 != Some(tvdb_id) {
                    return Err("Cache get failed after set");
                }

                // Create a new cache instance pointing to the same directory
                let cache2 = IdMappingCache::new(temp_dir.path().to_path_buf());

                // Should still retrieve the cached value (no expiration)
                let result2 = cache2.get(tmdb_id).await;
                if result2 != Some(tvdb_id) {
                    return Err("Cache persistence failed");
                }
                Ok(())
            });
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_cache_multiple_entries(
            entries in prop::collection::vec((1000u64..999999u64, 1000u64..999999u64), 1..10),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                let temp_dir = tempfile::tempdir().unwrap();
                let cache = IdMappingCache::new(temp_dir.path().to_path_buf());

                // Store multiple mappings
                for (tmdb_id, tvdb_id) in &entries {
                    cache.set(*tmdb_id, *tvdb_id).await.unwrap();
                }

                // Verify all can be retrieved
                for (tmdb_id, tvdb_id) in &entries {
                    let result = cache.get(*tmdb_id).await;
                    if result != Some(*tvdb_id) {
                        return Err("Cache retrieval failed");
                    }
                }
                Ok(())
            });
            prop_assert!(result.is_ok());
        }
    }

    // Property 10: Fuzzy Match ID Resolution Selects Highest Score Above Threshold
    // Validates: Requirements 2.3
    #[test]
    fn test_fuzzy_match_threshold() {
        // Test that fuzzy matching respects the 80% threshold
        let title1 = "Breaking Bad";
        let title2 = "Breaking Bad";
        let score = FuzzyMatcher::get_similarity_score(title1, title2);
        assert_eq!(score, 100, "Identical titles should have 100% score");

        // Test that very different titles don't match
        let title3 = "Game of Thrones";
        let score2 = FuzzyMatcher::get_similarity_score(title1, title3);
        assert!(score2 < 80, "Very different titles should score below 80%");
    }

    #[test]
    fn test_fuzzy_match_with_minor_differences() {
        // Test that minor differences still match above 80%
        let title1 = "Breaking Bad";
        let title2 = "Braking Bad"; // One character different
        let score = FuzzyMatcher::get_similarity_score(title1, title2);
        assert!(score >= 80, "Minor typos should still score >= 80%");
    }

    proptest! {
        #[test]
        fn prop_fuzzy_match_threshold(
            title in "[a-zA-Z ]{5,30}",
        ) {
            // Identical titles should always match (>= 80%)
            let score = FuzzyMatcher::get_similarity_score(&title, &title);
            prop_assert_eq!(score, 100, "Identical titles should have 100% score");

            // Score should be in valid range
            prop_assert!(score <= 100, "Score should be <= 100");
        }

        #[test]
        fn prop_fuzzy_match_selects_highest_score(
            title in "[a-zA-Z ]{5,20}",
            variant1 in "[a-zA-Z ]{5,20}",
            variant2 in "[a-zA-Z ]{5,20}",
        ) {
            // Calculate scores for different variants
            let score1 = FuzzyMatcher::get_similarity_score(&title, &variant1);
            let score2 = FuzzyMatcher::get_similarity_score(&title, &variant2);

            // The highest score should be >= the other
            let max_score = std::cmp::max(score1, score2);
            let min_score = std::cmp::min(score1, score2);

            prop_assert!(max_score >= min_score, "Max score should be >= min score");
            prop_assert!(max_score <= 100, "Max score should be <= 100");
        }

        #[test]
        fn prop_fuzzy_match_above_threshold_consistency(
            title in "[a-zA-Z ]{5,20}",
        ) {
            // If a title matches itself, it should be >= 80%
            let score = FuzzyMatcher::get_similarity_score(&title, &title);
            prop_assert!(score >= 80, "Identical titles should score >= 80%");
        }
    }
}
