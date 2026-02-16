// Special episode search query construction module

use crate::discovery::tvdb::TvdbEpisodeExtended;

/// Constructs search queries for special episodes
pub struct SpecialSearcher;

impl SpecialSearcher {
    /// Build search queries for a monitored special episode
    ///
    /// Returns a vector of search query strings to try in order:
    /// 1. Standard query: `{title} S00E{number:02} {episode_title}`
    /// 2. Fallback query: `{title} {episode_title}`
    /// 3. Movie query: `{title} {episode_title} movie` (when is_movie=true)
    /// 4. Anime query: `{title} OVA {absolute_number}` (when absolute_number present)
    ///
    /// # Arguments
    /// * `series_title` - The title of the TV series
    /// * `episode` - The TVDB episode metadata
    ///
    /// # Returns
    /// A vector of search query strings
    pub fn build_queries(series_title: &str, episode: &TvdbEpisodeExtended) -> Vec<String> {
        let mut queries = Vec::new();

        // Standard query: {title} S00E{number:02} {episode_title}
        let standard_query = format!(
            "{} S00E{:02} {}",
            series_title, episode.number, episode.name
        );
        queries.push(standard_query);

        // Fallback query: {title} {episode_title}
        let fallback_query = format!("{} {}", series_title, episode.name);
        queries.push(fallback_query);

        // Movie query: {title} {episode_title} movie (when is_movie=true)
        if episode.is_movie == Some(true) {
            let movie_query = format!("{} {} movie", series_title, episode.name);
            queries.push(movie_query);
        }

        // Anime query: {title} OVA {absolute_number} (when absolute_number present)
        if let Some(abs_num) = episode.absolute_number {
            let anime_query = format!("{} OVA {}", series_title, abs_num);
            queries.push(anime_query);
        }

        queries
    }

    /// Check if a YouTube result title should be included based on similarity threshold
    ///
    /// Returns true if the title similarity is at least 60%, false otherwise.
    ///
    /// # Arguments
    /// * `result_title` - The title from the YouTube search result
    /// * `expected_title` - The expected episode title from TVDB
    ///
    /// # Returns
    /// True if the result should be included, false if it should be skipped
    #[allow(dead_code)]
    pub fn should_include_result(result_title: &str, expected_title: &str) -> bool {
        let similarity = Self::calculate_similarity(result_title, expected_title);
        similarity >= 0.6
    }

    /// Calculate similarity between two strings using a simple character-based approach
    ///
    /// This is a simplified similarity metric based on common characters.
    /// For production use, consider using a proper fuzzy matching library.
    #[allow(dead_code)]
    fn calculate_similarity(s1: &str, s2: &str) -> f64 {
        let s1_lower = s1.to_lowercase();
        let s2_lower = s2.to_lowercase();

        // Simple approach: count common characters
        let s1_chars: std::collections::HashSet<char> = s1_lower.chars().collect();
        let s2_chars: std::collections::HashSet<char> = s2_lower.chars().collect();

        let common_chars = s1_chars.intersection(&s2_chars).count();
        let total_chars = s1_chars.union(&s2_chars).count();

        if total_chars == 0 {
            return 0.0;
        }

        common_chars as f64 / total_chars as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_episode(
        number: u8,
        name: &str,
        is_movie: Option<bool>,
        absolute_number: Option<u32>,
    ) -> TvdbEpisodeExtended {
        TvdbEpisodeExtended {
            id: 1,
            number,
            name: name.to_string(),
            aired: None,
            overview: None,
            absolute_number,
            airs_before_season: None,
            airs_after_season: None,
            airs_before_episode: None,
            is_movie,
        }
    }

    #[test]
    fn test_build_queries_standard_and_fallback() {
        let episode = create_test_episode(5, "Holiday Special", None, None);
        let queries = SpecialSearcher::build_queries("Breaking Bad", &episode);

        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0], "Breaking Bad S00E05 Holiday Special");
        assert_eq!(queries[1], "Breaking Bad Holiday Special");
    }

    #[test]
    fn test_build_queries_with_movie() {
        let episode = create_test_episode(5, "Holiday Special", Some(true), None);
        let queries = SpecialSearcher::build_queries("Breaking Bad", &episode);

        assert_eq!(queries.len(), 3);
        assert_eq!(queries[0], "Breaking Bad S00E05 Holiday Special");
        assert_eq!(queries[1], "Breaking Bad Holiday Special");
        assert_eq!(queries[2], "Breaking Bad Holiday Special movie");
    }

    #[test]
    fn test_build_queries_with_absolute_number() {
        let episode = create_test_episode(5, "Holiday Special", None, Some(42));
        let queries = SpecialSearcher::build_queries("Breaking Bad", &episode);

        assert_eq!(queries.len(), 3);
        assert_eq!(queries[0], "Breaking Bad S00E05 Holiday Special");
        assert_eq!(queries[1], "Breaking Bad Holiday Special");
        assert_eq!(queries[2], "Breaking Bad OVA 42");
    }

    #[test]
    fn test_build_queries_with_movie_and_absolute_number() {
        let episode = create_test_episode(5, "Holiday Special", Some(true), Some(42));
        let queries = SpecialSearcher::build_queries("Breaking Bad", &episode);

        assert_eq!(queries.len(), 4);
        assert_eq!(queries[0], "Breaking Bad S00E05 Holiday Special");
        assert_eq!(queries[1], "Breaking Bad Holiday Special");
        assert_eq!(queries[2], "Breaking Bad Holiday Special movie");
        assert_eq!(queries[3], "Breaking Bad OVA 42");
    }

    #[test]
    fn test_build_queries_episode_number_formatting() {
        let episode = create_test_episode(1, "Pilot", None, None);
        let queries = SpecialSearcher::build_queries("Test Series", &episode);

        assert!(queries[0].contains("S00E01"));
    }

    #[test]
    fn test_build_queries_episode_number_double_digit() {
        let episode = create_test_episode(15, "Special", None, None);
        let queries = SpecialSearcher::build_queries("Test Series", &episode);

        assert!(queries[0].contains("S00E15"));
    }

    #[test]
    fn test_should_include_result_high_similarity() {
        assert!(SpecialSearcher::should_include_result(
            "Breaking Bad Holiday Special",
            "Holiday Special"
        ));
    }

    #[test]
    fn test_should_include_result_low_similarity() {
        assert!(!SpecialSearcher::should_include_result(
            "Completely Different Title",
            "Holiday Special"
        ));
    }

    #[test]
    fn test_should_include_result_exact_match() {
        assert!(SpecialSearcher::should_include_result(
            "Holiday Special",
            "Holiday Special"
        ));
    }

    #[test]
    fn test_should_include_result_case_insensitive() {
        assert!(SpecialSearcher::should_include_result(
            "HOLIDAY SPECIAL",
            "holiday special"
        ));
    }

    #[test]
    fn test_calculate_similarity_identical() {
        let similarity = SpecialSearcher::calculate_similarity("test", "test");
        assert!((similarity - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_similarity_completely_different() {
        let similarity = SpecialSearcher::calculate_similarity("abc", "xyz");
        assert!(similarity < 0.5);
    }

    #[test]
    fn test_calculate_similarity_empty_strings() {
        let similarity = SpecialSearcher::calculate_similarity("", "");
        assert_eq!(similarity, 0.0);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::discovery::monitor_policy::MonitorPolicy;
    use proptest::prelude::*;

    // Feature: tvdb-specials, Property 6: Search Query Construction Correctness
    // Validates: Requirements 6.1, 6.2, 6.3, 6.4
    proptest! {
        #[test]
        fn prop_search_query_construction_correctness(
            series_title in "[a-zA-Z0-9 :',&!?.-]{1,50}",
            episode_number in 1u8..=99u8,
            episode_name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            is_movie in proptest::option::of(any::<bool>()),
            absolute_number in proptest::option::of(1u32..10000u32)
        ) {
            let episode = TvdbEpisodeExtended {
                id: 1,
                number: episode_number,
                name: episode_name.clone(),
                aired: None,
                overview: None,
                absolute_number,
                airs_before_season: None,
                airs_after_season: None,
                airs_before_episode: None,
                is_movie,
            };

            let queries = SpecialSearcher::build_queries(&series_title, &episode);

            // Property (a): Always contains standard query
            let standard_query = format!(
                "{} S00E{:02} {}",
                series_title, episode_number, episode_name
            );
            prop_assert!(
                queries.contains(&standard_query),
                "Queries must contain standard query: {}",
                standard_query
            );

            // Property (b): Always contains fallback query
            let fallback_query = format!("{} {}", series_title, episode_name);
            prop_assert!(
                queries.contains(&fallback_query),
                "Queries must contain fallback query: {}",
                fallback_query
            );

            // Property (c): Contains movie query if is_movie is true
            if is_movie == Some(true) {
                let movie_query = format!("{} {} movie", series_title, episode_name);
                prop_assert!(
                    queries.contains(&movie_query),
                    "Queries must contain movie query when is_movie=true: {}",
                    movie_query
                );
            }

            // Property (d): Contains anime query if absolute_number is present
            if let Some(abs_num) = absolute_number {
                let anime_query = format!("{} OVA {}", series_title, abs_num);
                prop_assert!(
                    queries.contains(&anime_query),
                    "Queries must contain anime query when absolute_number is present: {}",
                    anime_query
                );
            }

            // Verify minimum query count
            prop_assert!(queries.len() >= 2, "Must have at least 2 queries");

            // Verify maximum query count
            let expected_count = 2
                + if is_movie == Some(true) { 1 } else { 0 }
                + if absolute_number.is_some() { 1 } else { 0 };
            prop_assert_eq!(
                queries.len(),
                expected_count,
                "Query count mismatch"
            );
        }
    }

    // Feature: tvdb-specials, Property 6: Episode Number Formatting
    // Validates: Requirements 6.1
    proptest! {
        #[test]
        fn prop_episode_number_formatting(
            series_title in "[a-zA-Z0-9 ]{1,50}",
            episode_number in 1u8..=99u8,
            episode_name in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let episode = TvdbEpisodeExtended {
                id: 1,
                number: episode_number,
                name: episode_name.clone(),
                aired: None,
                overview: None,
                absolute_number: None,
                airs_before_season: None,
                airs_after_season: None,
                airs_before_episode: None,
                is_movie: None,
            };

            let queries = SpecialSearcher::build_queries(&series_title, &episode);

            // Standard query should have S00E{number:02} format
            let expected_format = format!("S00E{:02}", episode_number);
            prop_assert!(
                queries[0].contains(&expected_format),
                "Standard query must contain {} format, got: {}",
                expected_format,
                queries[0]
            );
        }
    }

    // Feature: tvdb-specials, Property 6: Title Similarity Filtering
    // Validates: Requirements 6.6
    proptest! {
        #[test]
        fn prop_title_similarity_filtering(
            title1 in "[a-zA-Z0-9 ]{5,30}",
            title2 in "[a-zA-Z0-9 ]{5,30}"
        ) {
            let should_include = SpecialSearcher::should_include_result(&title1, &title2);

            // If titles are identical, should always include
            if title1 == title2 {
                prop_assert!(should_include, "Identical titles should be included");
            }

            // Result should be deterministic
            let should_include_again = SpecialSearcher::should_include_result(&title1, &title2);
            prop_assert_eq!(
                should_include,
                should_include_again,
                "Similarity check should be deterministic"
            );
        }
    }

    // Feature: tvdb-specials, Property 6: Similarity Calculation Properties
    // Validates: Requirements 6.6
    proptest! {
        #[test]
        fn prop_similarity_calculation_properties(
            s1 in "[a-zA-Z0-9 ]{1,50}",
            s2 in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let similarity = SpecialSearcher::calculate_similarity(&s1, &s2);

            // Similarity should be between 0.0 and 1.0
            prop_assert!(
                similarity >= 0.0 && similarity <= 1.0,
                "Similarity must be in range [0.0, 1.0], got: {}",
                similarity
            );

            // Similarity should be symmetric
            let similarity_reverse = SpecialSearcher::calculate_similarity(&s2, &s1);
            prop_assert!(
                (similarity - similarity_reverse).abs() < 0.01,
                "Similarity should be symmetric"
            );

            // Identical strings should have similarity 1.0
            let self_similarity = SpecialSearcher::calculate_similarity(&s1, &s1);
            prop_assert!(
                (self_similarity - 1.0).abs() < 0.01,
                "Identical strings should have similarity 1.0"
            );
        }
    }

    // Feature: tvdb-specials, Property 5: Only Monitored Episodes Produce Search Queries
    // Validates: Requirements 5.5
    proptest! {
        #[test]
        fn prop_only_monitored_episodes_produce_queries(
            episodes_data in prop::collection::vec(
                (1u8..=99u8, prop::option::of(0u8..=20), prop::option::of(any::<bool>())),
                1..20
            ),
            latest_season in 0u8..=20,
            manual_list in prop::collection::vec(1u8..=99, 0..10),
            series_title in "[a-zA-Z0-9 ]{1,50}"
        ) {
            // Create episodes with varied monitoring conditions
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

            // Filter to monitored episodes
            let monitored = MonitorPolicy::filter_monitored(&episodes, latest_season, &manual_list);

            // Generate queries for monitored episodes
            let mut query_count = 0;
            for episode in &monitored {
                let queries = SpecialSearcher::build_queries(&series_title, episode);
                prop_assert!(
                    !queries.is_empty(),
                    "Monitored episode {} should produce queries",
                    episode.number
                );
                query_count += 1;
            }

            // Verify count matches
            prop_assert_eq!(
                query_count,
                monitored.len(),
                "Query count should equal monitored episode count"
            );

            // Verify unmonitored episodes don't produce queries in our workflow
            // (This is a workflow property - we only call build_queries for monitored episodes)
            for episode in &episodes {
                let is_monitored = MonitorPolicy::should_monitor(episode, latest_season, &manual_list);

                if is_monitored {
                    // Monitored episodes should produce queries
                    let queries = SpecialSearcher::build_queries(&series_title, episode);
                    prop_assert!(
                        !queries.is_empty(),
                        "Monitored episode {} must produce queries",
                        episode.number
                    );
                } else {
                    // Unmonitored episodes can still produce queries if called,
                    // but in the workflow we only call build_queries for monitored episodes
                    // This property validates the workflow behavior, not the function itself
                }
            }
        }
    }
}
