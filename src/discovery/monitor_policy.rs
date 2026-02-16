use crate::discovery::tvdb::TvdbEpisodeExtended;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

/// Configuration for manually monitored special episodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualMonitorConfig {
    /// List of episode numbers to manually monitor
    pub monitored_episodes: Vec<u8>,
}

/// Policy for determining which Season 0 episodes should be monitored for download
pub struct MonitorPolicy;

impl MonitorPolicy {
    /// Check if a single episode should be monitored based on the policy rules
    ///
    /// An episode is monitored if ANY of the following conditions are true:
    /// 1. Its `airs_after_season` equals the latest season number on disk
    /// 2. Its `is_movie` flag is true
    /// 3. Its episode number appears in the manual monitor list
    ///
    /// Otherwise, the episode defaults to unmonitored.
    pub fn should_monitor(
        episode: &TvdbEpisodeExtended,
        latest_season: u8,
        manual_monitor_list: &[u8],
    ) -> bool {
        // Check if airs_after_season matches latest season
        if let Some(airs_after) = episode.airs_after_season {
            if airs_after == latest_season {
                return true;
            }
        }

        // Check if is_movie is true
        if episode.is_movie == Some(true) {
            return true;
        }

        // Check if episode number is in manual monitor list
        if manual_monitor_list.contains(&episode.number) {
            return true;
        }

        // Default: unmonitored
        false
    }

    /// Filter episodes to only those that should be monitored
    ///
    /// Returns a vector of references to episodes that pass the monitoring policy.
    pub fn filter_monitored<'a>(
        episodes: &'a [TvdbEpisodeExtended],
        latest_season: u8,
        manual_monitor_list: &[u8],
    ) -> Vec<&'a TvdbEpisodeExtended> {
        episodes
            .iter()
            .filter(|episode| Self::should_monitor(episode, latest_season, manual_monitor_list))
            .collect()
    }

    /// Load manual monitor configuration from a JSON file
    ///
    /// Reads from `{series_folder}/specials_monitor.json` if it exists.
    /// Returns an empty list if the file doesn't exist or can't be parsed.
    pub async fn load_manual_monitor_list(series_folder: &Path) -> Vec<u8> {
        let config_path = series_folder.join("specials_monitor.json");

        match fs::read_to_string(&config_path).await {
            Ok(content) => {
                match serde_json::from_str::<ManualMonitorConfig>(&content) {
                    Ok(config) => config.monitored_episodes,
                    Err(e) => {
                        log::warn!(
                            "Failed to parse specials_monitor.json: {}. Using empty list.",
                            e
                        );
                        Vec::new()
                    }
                }
            }
            Err(_) => {
                // File doesn't exist or can't be read - this is normal, just use empty list
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
    fn test_default_unmonitored() {
        let episode = create_test_episode(1, None, None);
        assert!(!MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitor_airs_after_season_match() {
        let episode = create_test_episode(1, Some(5), None);
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitor_airs_after_season_no_match() {
        let episode = create_test_episode(1, Some(4), None);
        assert!(!MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitor_is_movie() {
        let episode = create_test_episode(1, None, Some(true));
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitor_is_movie_false() {
        let episode = create_test_episode(1, None, Some(false));
        assert!(!MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitor_manual_list() {
        let episode = create_test_episode(3, None, None);
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[1, 3, 5]));
    }

    #[test]
    fn test_monitor_manual_list_not_in_list() {
        let episode = create_test_episode(2, None, None);
        assert!(!MonitorPolicy::should_monitor(&episode, 5, &[1, 3, 5]));
    }

    #[test]
    fn test_monitor_multiple_conditions_airs_after_and_movie() {
        let episode = create_test_episode(1, Some(5), Some(true));
        assert!(MonitorPolicy::should_monitor(&episode, 5, &[]));
    }

    #[test]
    fn test_monitor_multiple_conditions_airs_after_and_manual() {
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
    fn test_filter_monitored_mixed() {
        let episodes = vec![
            create_test_episode(1, Some(5), None),      // monitored: airs_after_season
            create_test_episode(2, None, None),         // unmonitored
            create_test_episode(3, None, Some(true)),   // monitored: is_movie
            create_test_episode(4, None, None),         // unmonitored
            create_test_episode(5, None, None),         // monitored: in manual list
        ];

        let filtered = MonitorPolicy::filter_monitored(&episodes, 5, &[5]);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].number, 1);
        assert_eq!(filtered[1].number, 3);
        assert_eq!(filtered[2].number, 5);
    }

    #[test]
    fn test_filter_monitored_all_unmonitored() {
        let episodes = vec![
            create_test_episode(1, None, None),
            create_test_episode(2, None, None),
            create_test_episode(3, None, None),
        ];

        let filtered = MonitorPolicy::filter_monitored(&episodes, 5, &[]);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_monitored_all_monitored() {
        let episodes = vec![
            create_test_episode(1, Some(5), None),
            create_test_episode(2, Some(5), None),
            create_test_episode(3, Some(5), None),
        ];

        let filtered = MonitorPolicy::filter_monitored(&episodes, 5, &[]);
        assert_eq!(filtered.len(), 3);
    }

    #[tokio::test]
    async fn test_load_manual_monitor_list_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let list = MonitorPolicy::load_manual_monitor_list(temp_dir.path()).await;
        assert_eq!(list, Vec::<u8>::new());
    }

    #[tokio::test]
    async fn test_load_manual_monitor_list_valid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("specials_monitor.json");

        let config = ManualMonitorConfig {
            monitored_episodes: vec![1, 3, 5],
        };
        let json = serde_json::to_string(&config).unwrap();
        fs::write(&config_path, json).await.unwrap();

        let list = MonitorPolicy::load_manual_monitor_list(temp_dir.path()).await;
        assert_eq!(list, vec![1, 3, 5]);
    }

    #[tokio::test]
    async fn test_load_manual_monitor_list_invalid_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("specials_monitor.json");

        fs::write(&config_path, "invalid json").await.unwrap();

        let list = MonitorPolicy::load_manual_monitor_list(temp_dir.path()).await;
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
            manual_list in prop::collection::vec(0u8..=255, 0..10),
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

            let should_monitor = MonitorPolicy::should_monitor(&episode, latest_season, &manual_list);

            // Determine expected monitoring status
            let expected_monitored = {
                // Condition 1: airs_after_season matches latest season
                let airs_after_matches = airs_after_season.map_or(false, |s| s == latest_season);

                // Condition 2: is_movie is true
                let is_movie_true = is_movie == Some(true);

                // Condition 3: episode number in manual list
                let in_manual_list = manual_list.contains(&episode_number);

                airs_after_matches || is_movie_true || in_manual_list
            };

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
            manual_list in prop::collection::vec(0u8..=255, 0..10),
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

            let filtered = MonitorPolicy::filter_monitored(&episodes, latest_season, &manual_list);

            // Verify all filtered episodes should be monitored
            for episode in &filtered {
                prop_assert!(
                    MonitorPolicy::should_monitor(episode, latest_season, &manual_list),
                    "Filtered episode {} should be monitored",
                    episode.number
                );
            }

            // Verify no unmonitored episodes are in the filtered list
            for episode in &episodes {
                let is_in_filtered = filtered.iter().any(|e| e.id == episode.id);
                let should_be_monitored =
                    MonitorPolicy::should_monitor(episode, latest_season, &manual_list);

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
        fn prop_default_unmonitored(
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

            prop_assert!(!should_monitor, "Episode should default to unmonitored");
        }
    }

    proptest! {
        #[test]
        fn prop_manual_list_independent(
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

            let manual_list = vec![episode_number];
            let should_monitor = MonitorPolicy::should_monitor(&episode, latest_season, &manual_list);

            prop_assert!(
                should_monitor,
                "Episode in manual list should be monitored"
            );
        }
    }
}
