// Vimeo discovery module — stub for Story 8.2
//
// The VimeoDiscoverer is wired into both orchestrators but returns
// empty results until Story 8.2 implements the actual Vimeo REST API search.

use crate::error::DiscoveryError;
use crate::models::VideoSource;
use log::info;

/// Discovers video extras from Vimeo using a Personal Access Token.
/// Currently a stub — Story 8.2 will implement the actual API search.
pub(crate) struct VimeoDiscoverer {
    _access_token: String,
    _client: reqwest::Client,
}

impl VimeoDiscoverer {
    /// Create a new VimeoDiscoverer with the given Personal Access Token.
    pub fn new(access_token: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client for Vimeo");
        Self {
            _access_token: access_token,
            _client: client,
        }
    }

    /// Discover extras from Vimeo for the given title and year.
    /// Stub: returns empty results until Story 8.2.
    pub async fn discover(
        &self,
        title: &str,
        year: u16,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!(
            "Vimeo discovery not yet implemented for {} ({})",
            title, year
        );
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn test_vimeo_discoverer_new() {
        let _discoverer = VimeoDiscoverer::new("test_token".to_string());
    }

    #[tokio::test]
    async fn test_vimeo_discoverer_discover_returns_empty() {
        let discoverer = VimeoDiscoverer::new("test_token".to_string());
        let result = discoverer.discover("Inception", 2010).await;
        assert!(result.is_ok());
        assert!(result.expect("should be Ok").is_empty());
    }

    #[test]
    fn test_discovery_orchestrator_with_vimeo_source() {
        use crate::discovery::DiscoveryOrchestrator;
        use crate::models::Source;

        let _orch = DiscoveryOrchestrator::new(
            "tmdb_key".to_string(),
            vec![Source::Vimeo],
            Arc::new(AtomicU32::new(0)),
            "token".to_string(),
        );
    }

    #[test]
    fn test_series_orchestrator_with_vimeo_source() {
        use crate::discovery::SeriesDiscoveryOrchestrator;
        use crate::models::Source;

        let _orch = SeriesDiscoveryOrchestrator::new(
            "tmdb_key".to_string(),
            vec![Source::Vimeo],
            "token".to_string(),
        );
    }
}
