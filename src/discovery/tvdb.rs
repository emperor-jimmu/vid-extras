// TheTVDB API v4 integration module

use serde::{Deserialize, Serialize};

/// Base episode data from the Season 0 listing endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvdbEpisode {
    /// TheTVDB episode ID
    pub id: u64,
    /// Episode number within the season
    pub number: u8,
    /// Episode title
    pub name: String,
    /// Optional air date (ISO 8601 format)
    #[serde(default)]
    pub aired: Option<String>,
    /// Optional episode overview/description
    #[serde(default)]
    pub overview: Option<String>,
}

/// Enriched episode data from the extended endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvdbEpisodeExtended {
    /// TheTVDB episode ID
    pub id: u64,
    /// Episode number within the season
    pub number: u8,
    /// Episode title
    pub name: String,
    /// Optional air date (ISO 8601 format)
    #[serde(default)]
    pub aired: Option<String>,
    /// Optional episode overview/description
    #[serde(default)]
    pub overview: Option<String>,
    /// Optional absolute episode number (for anime)
    #[serde(default)]
    pub absolute_number: Option<u32>,
    /// Optional season number this episode airs before
    #[serde(default)]
    pub airs_before_season: Option<u8>,
    /// Optional season number this episode airs after
    #[serde(default)]
    pub airs_after_season: Option<u8>,
    /// Optional episode number this episode airs before
    #[serde(default)]
    pub airs_before_episode: Option<u8>,
    /// Whether this episode is a movie-type special
    #[serde(default)]
    pub is_movie: Option<bool>,
}

/// Search result from the TVDB `/search` endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct TvdbSearchResult {
    /// TheTVDB series ID
    pub tvdb_id: String,
    /// Series name
    pub name: String,
    /// Optional year of release
    #[serde(default)]
    pub year: Option<String>,
}

/// Generic API response wrapper
#[derive(Debug, Deserialize)]
pub struct TvdbApiResponse<T> {
    /// Response status
    pub status: String,
    /// Response data
    pub data: T,
}

/// Episodes page response from the API
#[derive(Debug, Deserialize)]
pub struct TvdbEpisodesPage {
    /// List of episodes on this page
    pub episodes: Vec<TvdbEpisode>,
    /// Optional URL to next page
    #[serde(default)]
    pub next: Option<String>,
}

/// Login response containing Bearer token
#[derive(Debug, Deserialize)]
pub struct TvdbLoginResponse {
    /// Bearer token for authenticated requests
    pub token: String,
}

/// Search response containing results
#[derive(Debug, Deserialize)]
pub struct TvdbSearchResponse {
    /// List of search results
    pub data: Vec<TvdbSearchResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tvdb_episode_deserialization() {
        let json = r#"{
            "id": 123456,
            "number": 5,
            "name": "Holiday Special",
            "aired": "2010-12-25",
            "overview": "A special holiday episode"
        }"#;

        let episode: TvdbEpisode = serde_json::from_str(json).unwrap();
        assert_eq!(episode.id, 123456);
        assert_eq!(episode.number, 5);
        assert_eq!(episode.name, "Holiday Special");
        assert_eq!(episode.aired, Some("2010-12-25".to_string()));
    }

    #[test]
    fn test_tvdb_episode_extended_deserialization() {
        let json = r#"{
            "id": 123456,
            "number": 5,
            "name": "Holiday Special",
            "aired": "2010-12-25",
            "overview": "A special holiday episode",
            "absolute_number": 42,
            "airs_before_season": 2,
            "airs_after_season": 1,
            "airs_before_episode": 3,
            "is_movie": false
        }"#;

        let episode: TvdbEpisodeExtended = serde_json::from_str(json).unwrap();
        assert_eq!(episode.id, 123456);
        assert_eq!(episode.number, 5);
        assert_eq!(episode.absolute_number, Some(42));
        assert_eq!(episode.airs_before_season, Some(2));
        assert_eq!(episode.is_movie, Some(false));
    }

    #[test]
    fn test_tvdb_search_result_deserialization() {
        let json = r#"{
            "tvdb_id": "81189",
            "name": "Breaking Bad",
            "year": "2008"
        }"#;

        let result: TvdbSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.tvdb_id, "81189");
        assert_eq!(result.name, "Breaking Bad");
        assert_eq!(result.year, Some("2008".to_string()));
    }

    #[test]
    fn test_tvdb_login_response_deserialization() {
        let json = r#"{"token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"}"#;

        let response: TvdbLoginResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.token, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
    }

    #[test]
    fn test_tvdb_episodes_page_deserialization() {
        let json = r#"{
            "episodes": [
                {"id": 1, "number": 1, "name": "Ep1", "aired": null, "overview": null},
                {"id": 2, "number": 2, "name": "Ep2", "aired": null, "overview": null}
            ],
            "next": "https://api4.thetvdb.com/v4/series/123/episodes/default?page=1"
        }"#;

        let page: TvdbEpisodesPage = serde_json::from_str(json).unwrap();
        assert_eq!(page.episodes.len(), 2);
        assert!(page.next.is_some());
    }

    #[test]
    fn test_tvdb_episode_with_missing_optional_fields() {
        let json = r#"{
            "id": 123456,
            "number": 5,
            "name": "Holiday Special"
        }"#;

        let episode: TvdbEpisode = serde_json::from_str(json).unwrap();
        assert_eq!(episode.id, 123456);
        assert_eq!(episode.aired, None);
        assert_eq!(episode.overview, None);
    }

    #[test]
    fn test_tvdb_episode_extended_with_missing_optional_fields() {
        let json = r#"{
            "id": 123456,
            "number": 5,
            "name": "Holiday Special"
        }"#;

        let episode: TvdbEpisodeExtended = serde_json::from_str(json).unwrap();
        assert_eq!(episode.id, 123456);
        assert_eq!(episode.absolute_number, None);
        assert_eq!(episode.is_movie, None);
    }
}


#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: tvdb-specials, Property 3: TVDB Episode Parsing Completeness
    // Validates: Requirements 3.3, 4.2
    proptest! {
        #[test]
        fn prop_tvdb_episode_parsing_completeness(
            id in 1u64..1_000_000u64,
            number in 1u8..100u8,
            name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            aired in proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            overview in proptest::option::of("[a-zA-Z0-9 .,;:!?'-]{1,200}")
        ) {
            let episode = TvdbEpisode {
                id,
                number,
                name: name.clone(),
                aired: aired.clone(),
                overview: overview.clone(),
            };

            // Verify all fields are populated
            prop_assert_eq!(episode.id, id);
            prop_assert_eq!(episode.number, number);
            prop_assert_eq!(&episode.name, &name);
            prop_assert_eq!(&episode.aired, &aired);
            prop_assert_eq!(&episode.overview, &overview);
        }
    }

    // Feature: tvdb-specials, Property 3: TVDB Episode Extended Parsing Completeness
    // Validates: Requirements 3.3, 4.2
    proptest! {
        #[test]
        fn prop_tvdb_episode_extended_parsing_completeness(
            id in 1u64..1_000_000u64,
            number in 1u8..100u8,
            name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            aired in proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            overview in proptest::option::of("[a-zA-Z0-9 .,;:!?'-]{1,200}"),
            absolute_number in proptest::option::of(1u32..10000u32),
            airs_before_season in proptest::option::of(0u8..100u8),
            airs_after_season in proptest::option::of(0u8..100u8),
            airs_before_episode in proptest::option::of(1u8..100u8),
            is_movie in proptest::option::of(any::<bool>())
        ) {
            let episode = TvdbEpisodeExtended {
                id,
                number,
                name: name.clone(),
                aired: aired.clone(),
                overview: overview.clone(),
                absolute_number,
                airs_before_season,
                airs_after_season,
                airs_before_episode,
                is_movie,
            };

            // Verify all fields are populated
            prop_assert_eq!(episode.id, id);
            prop_assert_eq!(episode.number, number);
            prop_assert_eq!(&episode.name, &name);
            prop_assert_eq!(&episode.aired, &aired);
            prop_assert_eq!(&episode.overview, &overview);
            prop_assert_eq!(episode.absolute_number, absolute_number);
            prop_assert_eq!(episode.airs_before_season, airs_before_season);
            prop_assert_eq!(episode.airs_after_season, airs_after_season);
            prop_assert_eq!(episode.airs_before_episode, airs_before_episode);
            prop_assert_eq!(episode.is_movie, is_movie);
        }
    }

    // Feature: tvdb-specials, Property 3: TVDB Episode Serialization Round-Trip
    // Validates: Requirements 3.3, 4.2
    proptest! {
        #[test]
        fn prop_tvdb_episode_serialization_round_trip(
            id in 1u64..1_000_000u64,
            number in 1u8..100u8,
            name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            aired in proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            overview in proptest::option::of("[a-zA-Z0-9 .,;:!?'-]{1,200}")
        ) {
            let episode = TvdbEpisode {
                id,
                number,
                name: name.clone(),
                aired: aired.clone(),
                overview: overview.clone(),
            };

            // Serialize to JSON
            let json = serde_json::to_string(&episode).unwrap();

            // Deserialize from JSON
            let deserialized: TvdbEpisode = serde_json::from_str(&json).unwrap();

            // Verify round-trip preserves all fields
            prop_assert_eq!(episode.id, deserialized.id);
            prop_assert_eq!(episode.number, deserialized.number);
            prop_assert_eq!(&episode.name, &deserialized.name);
            prop_assert_eq!(&episode.aired, &deserialized.aired);
            prop_assert_eq!(&episode.overview, &deserialized.overview);
        }
    }

    // Feature: tvdb-specials, Property 3: TVDB Episode Extended Serialization Round-Trip
    // Validates: Requirements 3.3, 4.2
    proptest! {
        #[test]
        fn prop_tvdb_episode_extended_serialization_round_trip(
            id in 1u64..1_000_000u64,
            number in 1u8..100u8,
            name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            aired in proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            overview in proptest::option::of("[a-zA-Z0-9 .,;:!?'-]{1,200}"),
            absolute_number in proptest::option::of(1u32..10000u32),
            airs_before_season in proptest::option::of(0u8..100u8),
            airs_after_season in proptest::option::of(0u8..100u8),
            airs_before_episode in proptest::option::of(1u8..100u8),
            is_movie in proptest::option::of(any::<bool>())
        ) {
            let episode = TvdbEpisodeExtended {
                id,
                number,
                name: name.clone(),
                aired: aired.clone(),
                overview: overview.clone(),
                absolute_number,
                airs_before_season,
                airs_after_season,
                airs_before_episode,
                is_movie,
            };

            // Serialize to JSON
            let json = serde_json::to_string(&episode).unwrap();

            // Deserialize from JSON
            let deserialized: TvdbEpisodeExtended = serde_json::from_str(&json).unwrap();

            // Verify round-trip preserves all fields
            prop_assert_eq!(episode.id, deserialized.id);
            prop_assert_eq!(episode.number, deserialized.number);
            prop_assert_eq!(&episode.name, &deserialized.name);
            prop_assert_eq!(&episode.aired, &deserialized.aired);
            prop_assert_eq!(&episode.overview, &deserialized.overview);
            prop_assert_eq!(episode.absolute_number, deserialized.absolute_number);
            prop_assert_eq!(episode.airs_before_season, deserialized.airs_before_season);
            prop_assert_eq!(episode.airs_after_season, deserialized.airs_after_season);
            prop_assert_eq!(episode.airs_before_episode, deserialized.airs_before_episode);
            prop_assert_eq!(episode.is_movie, deserialized.is_movie);
        }
    }
}
