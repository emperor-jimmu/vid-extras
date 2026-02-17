use crate::discovery::tvdb::TvdbEpisodeExtended;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

/// Configuration for manually excluded special episodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualExcludeConfig {
    /// List of episode numbers to exclude from monitoring
    pub excluded_episodes: Vec<u8>,
}

/// Policy for determining which Season 0 episodes should be monitored for download.
///
/// All Season 0 episodes are monitored by default. Users can exclude specific
/// episodes via a `specials_exclude.json` file in the series folder.
pub struct MonitorPolicy;

impl MonitorPolicy {
    /// Check if a single episode should be monitored.
    ///
    /// All Season 0 episodes are monitored unless their episode number
    /// appears in the manual exclusion list.
    pub fn should_monitor(
        episode: &TvdbEpisodeExtended,
        _latest_season: u8,
        manual_exclude_list: &[u8],
    ) -> bool {
        !manual_exclude_list.contains(&episode.number)
    }

    /// Filter episodes to only those that should be monitored.
    ///
    /// Returns all episodes except those in the exclusion list.
    pub fn filter_monitored<'a>(
        episodes: &'a [TvdbEpisodeExtended],
        latest_season: u8,
        manual_exclude_list: &[u8],
    ) -> Vec<&'a TvdbEpisodeExtended> {
        episodes
            .iter()
            .filter(|episode| Self::should_monitor(episode, latest_season, manual_exclude_list))
            .collect()
    }

    /// Load manual exclusion configuration from a JSON file.
    ///
    /// Reads from `{series_folder}/specials_exclude.json` if it exists.
    /// Returns an empty list if the file doesn't exist or can't be parsed,
    /// meaning all episodes will be monitored.
    pub async fn load_manual_exclude_list(series_folder: &Path) -> Vec<u8> {
        let config_path = series_folder.join("specials_exclude.json");

        match fs::read_to_string(&config_path).await {
            Ok(content) => match serde_json::from_str::<ManualExcludeConfig>(&content) {
                Ok(config) => config.excluded_episodes,
                Err(e) => {
                    log::warn!(
                        "Failed to parse specials_exclude.json: {}. Using empty list.",
                        e
                    );
                    Vec::new()
                }
            },
            Err(_) => {
                // File doesn't exist or can't be read - this is normal, all episodes monitored
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_episode(
        number: u8,
        airs_after_season: Option<u8>,
        is_movie: Option<bool>,
    ) -> TvdbEpisodeExtended {
        TvdbEpisodeExtended {
            id: 1,
            number,
            name: "Test Episode".to_string(),
            aired: None,
            overview: None,
            absolute_number: None,
            airs_before_season: None,
            airs_after_season,
            airs_before_episode: None,
            is_movie,
        }
    }

    #[test]
    fn test_default_monitored() {
        let episode = create_test_episode(1, None, None);
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitored_with_airs_after_season() {
        // airs_after_season is informational only, episode is still monitored
        let episode = create_test_episode(1, Some(5), None);
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitored_with_airs_after_season_mismatch() {
        // airs_after_season doesn't affect monitoring
        let episode = create_test_episode(1, Some(4), None);
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitored_is_movie_true() {
        // is_movie is informational only, episode is still monitored
        let episode = create_test_episode(1, None, Some(true));
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitored_is_movie_false() {
        // is_movie doesn't affect monitoring
        let episode = create_test_episode(1, None, Some(false));
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_excluded_by_list() {
        let episode = create_test_episode(3, None, None);
        assert!(!MonitorPolicy::should_monitor(&episode, 5, &[1, 3, 5]));
    }

    #[test]
    fn test_not_excluded_by_list() {
        let episode = create_test_episode(2, None, None);
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[1, 3, 5]));
    }

    #[test]
    fn test_excluded_overrides_metadata() {
        // Even with airs_after_season and is_movie, exclusion list wins
        let episode = create_test_episode(1, Some(5), Some(true));
        assert!(!MonitorPolicy::should_monitor(&episode, 5, &[1]));
    }

    #[test]
    fn test_not_excluded_with_metadata() {
        let episode = create_test_episode(1, Some(5), None);
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[2, 3]));
    }

    #[test]
    fn test_filter_monitored_empty() {
        let episodes = vec![];
        let filtered = MonitorPolicy::filter_monitored(&episodes, 5, &[]);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_monitored_with_exclusions() {
        let episodes = vec![
            create_test_episode(1, Some(5), None),    // monitored
            create_test_episode(2, None, None),       // excluded
            create_test_episode(3, None, Some(true)), // monitored
            create_test_episode(4, None, None),       // excluded
            create_test_episode(5, None, None),       // monitored
        ];

        let filtered = MonitorPolicy::filter_monitored(&episodes, 5, &[2, 4]);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].number, 1);
        assert_eq!(filtered[1].number, 3);
        assert_eq!(filtered[2].number, 5);
    }

    #[test]
    fn test_filter_monitored_all_monitored_no_exclusions() {
        let episodes = vec![
            create_test_episode(1, None, None),
            create_test_episode(2, None, None),
            create_test_episode(3, None, None),
        ];

        let filtered = MonitorPolicy::filter_monitored(&episodes, 5, &[]);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_filter_monitored_all_excluded() {
        let episodes = vec![
            create_test_episode(1, Some(5), None),
            create_test_episode(2, Some(5), None),
            create_test_episode(3, Some(5), None),
        ];

        let filtered = MonitorPolicy::filter_monitored(&episodes, 5, &[1, 2, 3]);
        assert_eq!(filtered.len(), 0);
    }

    #[tokio::test]
    async fn test_load_manual_exclude_list_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let list = MonitorPolicy::load_manual_exclude_list(temp_dir.path()).await;
        assert_eq!(list, Vec::<u8>::new());
    }

    #[tokio::test]
    async fn test_load_manual_exclude_list_valid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("specials_exclude.json");

        let config = ManualExcludeConfig {
            excluded_episodes: vec![1, 3, 5],
        };
        let json = serde_json::to_string(&config).unwrap();
        fs::write(&config_path, json).await.unwrap();

        let list = MonitorPolicy::load_manual_exclude_list(temp_dir.path()).await;
        assert_eq!(list, vec![1, 3, 5]);
    }

    #[tokio::test]
    async fn test_load_manual_exclude_list_invalid_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("specials_exclude.json");

        fs::write(&config_path, "invalid json").await.unwrap();

        let list = MonitorPolicy::load_manual_exclude_list(temp_dir.path()).await;
        assert_eq!(list, Vec::<u8>::new());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_monitor_policy_correctness(
            episode_number in 0u8..=255,
            latest_season in 0u8..=20,
            airs_after_season in prop::option::of(0u8..=20),
            is_movie in prop::option::of(any::<bool>()),
            exclude_list in prop::collection::vec(0u8..=255, 0..10),
        ) {
            let episode = TvdbEpisodeExtended {
                id: 1,
                number: episode_number,
                name: "Test".to_string(),
                aired: None,
                overview: None,
                absolute_number: None,
                airs_before_season: None,
                airs_after_season,
                airs_before_episode: None,
                is_movie,
            };

            let should_monitor = MonitorPolicy::should_monitor(&episode, latest_season, &exclude_list);

            // All episodes are monitored unless explicitly excluded
            let expected_monitored = !exclude_list.contains(&episode_number);

            prop_assert_eq!(
                should_monitor, expected_monitored,
                "Episode {} should be monitored={}, but got monitored={}",
                episode_number, expected_monitored, should_monitor
            );
        }
    }

    proptest! {
        #[test]
        fn prop_filter_monitored_correctness(
            episodes_data in prop::collection::vec(
                (0u8..=255, prop::option::of(0u8..=20), prop::option::of(any::<bool>())),
                0..20
            ),
            latest_season in 0u8..=20,
            exclude_list in prop::collection::vec(0u8..=255, 0..10),
        ) {
            let episodes: Vec<TvdbEpisodeExtended> = episodes_data
                .iter()
                .enumerate()
                .map(|(idx, (number, airs_after, is_movie))| {
                    TvdbEpisodeExtended {
                        id: idx as u64,
                        number: *number,
                        name: format!("Episode {}", idx),
                        aired: None,
                        overview: None,
                        absolute_number: None,
                        airs_before_season: None,
                        airs_after_season: *airs_after,
                        airs_before_episode: None,
                        is_movie: *is_movie,
                    }
                })
                .collect();

            let filtered = MonitorPolicy::filter_monitored(&episodes, latest_season, &exclude_list);

            // Verify all filtered episodes should be monitored
            for episode in &filtered {
                prop_assert!(
                    MonitorPolicy::should_monitor(episode, latest_season, &exclude_list),
                    "Filtered episode {} should be monitored",
                    episode.number
                );
            }

            // Verify no unmonitored episodes are in the filtered list
            for episode in &episodes {
                let is_in_filtered = filtered.iter().any(|e| e.id == episode.id);
                let should_be_monitored =
                    MonitorPolicy::should_monitor(episode, latest_season, &exclude_list);

                prop_assert_eq!(
                    is_in_filtered, should_be_monitored,
                    "Episode {} filtering mismatch",
                    episode.number
                );
            }
        }
    }

    proptest! {
        #[test]
        fn prop_default_monitored(
            episode_number in 0u8..=255,
            latest_season in 0u8..=20,
        ) {
            let episode = TvdbEpisodeExtended {
                id: 1,
                number: episode_number,
                name: "Test".to_string(),
                aired: None,
                overview: None,
                absolute_number: None,
                airs_before_season: None,
                airs_after_season: None,
                airs_before_episode: None,
                is_movie: Some(false),
            };

            let should_monitor = MonitorPolicy::should_monitor(&episode, latest_season, &[]);

            prop_assert!(should_monitor, "Episode should default to monitored");
        }
    }

    proptest! {
        #[test]
        fn prop_exclude_list_removes_episodes(
            episode_number in 0u8..=255,
            latest_season in 0u8..=20,
        ) {
            let episode = TvdbEpisodeExtended {
                id: 1,
                number: episode_number,
                name: "Test".to_string(),
                aired: None,
                overview: None,
                absolute_number: None,
                airs_before_season: None,
                airs_after_season: None,
                airs_before_episode: None,
                is_movie: Some(false),
            };

            let exclude_list = vec![episode_number];
            let should_monitor = MonitorPolicy::should_monitor(&episode, latest_season, &exclude_list);

            prop_assert!(
                !should_monitor,
                "Episode in exclusion list should not be monitored"
            );
        }
    }
}
