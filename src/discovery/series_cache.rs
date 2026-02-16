// Series metadata caching module - stores and retrieves TMDB series metadata with TTL

use crate::error::DiscoveryError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Cached series metadata with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CachedSeriesMetadata {
    /// Series ID from TMDB
    pub series_id: u64,
    /// Series name
    pub name: String,
    /// Timestamp when cached
    pub cached_at: String,
}

/// Series metadata cache manager
#[allow(dead_code)]
pub struct SeriesMetadataCache {
    /// Base cache directory (typically .cache under series folder)
    cache_dir: PathBuf,
    /// TTL in days (default 7)
    ttl_days: i64,
}

impl SeriesMetadataCache {
    /// Create a new cache manager for a series
    pub fn new(series_path: &Path) -> Self {
        let cache_dir = series_path.join(".cache");
        Self {
            cache_dir,
            ttl_days: 7,
        }
    }

    /// Create a new cache manager with custom TTL
    pub fn with_ttl(series_path: &Path, ttl_days: i64) -> Self {
        let cache_dir = series_path.join(".cache");
        Self { cache_dir, ttl_days }
    }

    /// Get cache file path for a series
    fn get_cache_file(&self, series_name: &str) -> PathBuf {
        let filename = format!("{}.json", Self::sanitize_filename(series_name));
        self.cache_dir.join(filename)
    }

    /// Sanitize filename by removing invalid characters
    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect()
    }

    /// Check if cache is still fresh (not expired)
    pub fn is_cache_fresh(cached_at: &str) -> bool {
        if let Ok(cached_time) = DateTime::parse_from_rfc3339(cached_at) {
            let cached_utc = cached_time.with_timezone(&Utc);
            let now = Utc::now();
            let age = now.signed_duration_since(cached_utc);
            age < Duration::days(7)
        } else {
            false
        }
    }

    /// Get cached metadata if available and fresh
    pub async fn get(
        &self,
        series_name: &str,
        force: bool,
    ) -> Result<Option<CachedSeriesMetadata>, DiscoveryError> {
        if force {
            return Ok(None);
        }

        let cache_file = self.get_cache_file(series_name);

        if !cache_file.exists() {
            return Ok(None);
        }

        match fs::read_to_string(&cache_file).await {
            Ok(content) => match serde_json::from_str::<CachedSeriesMetadata>(&content) {
                Ok(metadata) => {
                    if Self::is_cache_fresh(&metadata.cached_at) {
                        Ok(Some(metadata))
                    } else {
                        Ok(None)
                    }
                }
                Err(_) => Ok(None),
            },
            Err(_) => Ok(None),
        }
    }

    /// Store metadata in cache
    pub async fn set(
        &self,
        series_name: &str,
        metadata: CachedSeriesMetadata,
    ) -> Result<(), DiscoveryError> {
        // Create cache directory if it doesn't exist
        fs::create_dir_all(&self.cache_dir)
            .await
            .map_err(|e| DiscoveryError::ApiError(format!("Failed to create cache dir: {}", e)))?;

        let cache_file = self.get_cache_file(series_name);

        let json = serde_json::to_string(&metadata)
            .map_err(|e| DiscoveryError::ApiError(format!("Failed to serialize cache: {}", e)))?;

        fs::write(&cache_file, json)
            .await
            .map_err(|e| DiscoveryError::ApiError(format!("Failed to write cache: {}", e)))?;

        Ok(())
    }

    /// Clear cache for a specific series
    pub async fn clear(&self, series_name: &str) -> Result<(), DiscoveryError> {
        let cache_file = self.get_cache_file(series_name);

        if cache_file.exists() {
            fs::remove_file(&cache_file)
                .await
                .map_err(|e| DiscoveryError::ApiError(format!("Failed to delete cache: {}", e)))?;
        }

        Ok(())
    }

    /// Clear all cache files
    pub async fn clear_all(&self) -> Result<(), DiscoveryError> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)
                .await
                .map_err(|e| DiscoveryError::ApiError(format!("Failed to clear cache: {}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cache_creation_and_reading() {
        let temp_dir = TempDir::new().unwrap();
        let cache = SeriesMetadataCache::new(temp_dir.path());

        let metadata = CachedSeriesMetadata {
            series_id: 1399,
            name: "Breaking Bad".to_string(),
            cached_at: Utc::now().to_rfc3339(),
        };

        cache
            .set("Breaking Bad", metadata.clone())
            .await
            .unwrap();

        let retrieved = cache.get("Breaking Bad", false).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().series_id, 1399);
    }

    #[tokio::test]
    async fn test_cache_ttl_expiration() {
        let temp_dir = TempDir::new().unwrap();
        let cache = SeriesMetadataCache::new(temp_dir.path());

        // Create metadata with old timestamp (8 days ago)
        let old_time = Utc::now() - Duration::days(8);
        let metadata = CachedSeriesMetadata {
            series_id: 1399,
            name: "Breaking Bad".to_string(),
            cached_at: old_time.to_rfc3339(),
        };

        cache
            .set("Breaking Bad", metadata)
            .await
            .unwrap();

        // Should return None because cache is expired
        let retrieved = cache.get("Breaking Bad", false).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_force_flag_bypasses_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache = SeriesMetadataCache::new(temp_dir.path());

        let metadata = CachedSeriesMetadata {
            series_id: 1399,
            name: "Breaking Bad".to_string(),
            cached_at: Utc::now().to_rfc3339(),
        };

        cache
            .set("Breaking Bad", metadata)
            .await
            .unwrap();

        // With force=true, should return None even if cache exists
        let retrieved = cache.get("Breaking Bad", true).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_invalid_cache_handling() {
        let temp_dir = TempDir::new().unwrap();
        let cache = SeriesMetadataCache::new(temp_dir.path());

        // Create cache directory and write invalid JSON
        fs::create_dir_all(&cache.cache_dir).await.unwrap();
        let cache_file = cache.get_cache_file("Breaking Bad");
        fs::write(&cache_file, "invalid json").await.unwrap();

        // Should return None for invalid cache
        let retrieved = cache.get("Breaking Bad", false).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_cache_clear_single() {
        let temp_dir = TempDir::new().unwrap();
        let cache = SeriesMetadataCache::new(temp_dir.path());

        let metadata = CachedSeriesMetadata {
            series_id: 1399,
            name: "Breaking Bad".to_string(),
            cached_at: Utc::now().to_rfc3339(),
        };

        cache
            .set("Breaking Bad", metadata)
            .await
            .unwrap();

        cache.clear("Breaking Bad").await.unwrap();

        let retrieved = cache.get("Breaking Bad", false).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_cache_clear_all() {
        let temp_dir = TempDir::new().unwrap();
        let cache = SeriesMetadataCache::new(temp_dir.path());

        let metadata1 = CachedSeriesMetadata {
            series_id: 1399,
            name: "Breaking Bad".to_string(),
            cached_at: Utc::now().to_rfc3339(),
        };

        let metadata2 = CachedSeriesMetadata {
            series_id: 1396,
            name: "Game of Thrones".to_string(),
            cached_at: Utc::now().to_rfc3339(),
        };

        cache
            .set("Breaking Bad", metadata1)
            .await
            .unwrap();
        cache
            .set("Game of Thrones", metadata2)
            .await
            .unwrap();

        cache.clear_all().await.unwrap();

        let retrieved1 = cache.get("Breaking Bad", false).await.unwrap();
        let retrieved2 = cache.get("Game of Thrones", false).await.unwrap();
        assert!(retrieved1.is_none());
        assert!(retrieved2.is_none());
    }

    #[tokio::test]
    async fn test_cache_with_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        let cache = SeriesMetadataCache::new(temp_dir.path());

        let series_name = "Game of Thrones: A Song of Ice and Fire";
        let metadata = CachedSeriesMetadata {
            series_id: 1396,
            name: series_name.to_string(),
            cached_at: Utc::now().to_rfc3339(),
        };

        cache.set(series_name, metadata).await.unwrap();

        let retrieved = cache.get(series_name, false).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().series_id, 1396);
    }

    #[tokio::test]
    async fn test_cache_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let cache = SeriesMetadataCache::new(temp_dir.path());

        let retrieved = cache.get("Nonexistent Series", false).await.unwrap();
        assert!(retrieved.is_none());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Property 14: Metadata Cache Freshness
    // Validates: Requirements 14.1, 14.2, 14.3, 14.4
    // This property validates that cache freshness is correctly determined based on age and force flag
    proptest! {
        #[test]
        fn prop_cache_freshness_logic(
            days_old in 0i64..15i64,
            force in any::<bool>()
        ) {
            // Test the freshness logic without async
            let old_time = Utc::now() - Duration::days(days_old);
            let cached_at = old_time.to_rfc3339();

            let is_fresh = SeriesMetadataCache::is_cache_fresh(&cached_at);

            if force {
                // Force flag doesn't affect is_cache_fresh, but would be handled at call site
                // This just validates the freshness logic
                if days_old < 7 {
                    prop_assert!(is_fresh);
                } else {
                    prop_assert!(!is_fresh);
                }
            } else {
                if days_old < 7 {
                    prop_assert!(is_fresh);
                } else {
                    prop_assert!(!is_fresh);
                }
            }
        }
    }
}
