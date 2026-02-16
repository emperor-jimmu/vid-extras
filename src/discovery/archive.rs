// Archive.org content discoverer

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceType, VideoSource};
use log::{debug, error, info};
use serde::Deserialize;

use super::ContentDiscoverer;

/// Archive.org API response for search
#[derive(Debug, Deserialize)]
struct ArchiveOrgSearchResponse {
    response: ArchiveOrgResponse,
}

/// Archive.org response wrapper
#[derive(Debug, Deserialize)]
struct ArchiveOrgResponse {
    docs: Vec<ArchiveOrgDoc>,
}

/// Archive.org document entry
#[derive(Debug, Deserialize)]
struct ArchiveOrgDoc {
    identifier: String,
    title: String,
    #[serde(default)]
    subject: Vec<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    collection: Vec<String>,
}

/// Archive.org content discoverer
///
/// Discovers movie extras from Archive.org, searching two sources:
/// 1. General movie content for pre-2010 films (trailers, featurettes, etc.)
/// 2. DVDXtras collection - contains EPK content, behind-the-scenes, deleted scenes
///    from DVD releases (searched for all movies regardless of year)
///
/// Returns detail page URLs in the format `https://archive.org/details/{identifier}`
/// which are fully supported by yt-dlp's archive.org extractor.
pub struct ArchiveOrgDiscoverer {
    client: reqwest::Client,
}

impl Default for ArchiveOrgDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveOrgDiscoverer {
    /// Create a new Archive.org discoverer
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Build Archive.org search query for general movie content
    fn build_general_query(title: &str, year: u16) -> String {
        format!(
            "title:\"{}\" AND year:{} AND mediatype:movies AND (subject:trailer OR subject:featurette OR subject:\"behind the scenes\" OR subject:\"deleted scene\" OR subject:clip)",
            title, year
        )
    }

    /// Build Archive.org search query for DVDXtras collection
    /// This collection contains EPK content from DVD releases
    fn build_dvdxtras_query(title: &str) -> String {
        // Search the DVDXtras collection for the movie title
        // The collection contains behind-the-scenes, deleted scenes, featurettes, etc.
        format!(
            "collection:DVDXtras AND (title:\"{}\" OR description:\"{}\")",
            title, title
        )
    }

    /// Map Archive.org subjects to content categories
    fn map_subjects(subjects: &[String]) -> Option<ContentCategory> {
        let subjects_lower: Vec<String> = subjects.iter().map(|s| s.to_lowercase()).collect();

        // Check for specific content types in order of priority
        if subjects_lower.iter().any(|s| s.contains("trailer")) {
            Some(ContentCategory::Trailer)
        } else if subjects_lower
            .iter()
            .any(|s| s.contains("behind the scenes") || s.contains("making of"))
        {
            Some(ContentCategory::BehindTheScenes)
        } else if subjects_lower
            .iter()
            .any(|s| s.contains("deleted scene") || s.contains("deleted"))
        {
            Some(ContentCategory::DeletedScene)
        } else if subjects_lower
            .iter()
            .any(|s| s.contains("featurette") || s.eq("epk"))
        {
            Some(ContentCategory::Featurette)
        } else if subjects_lower
            .iter()
            .any(|s| s.contains("interview") || s.contains("q&a"))
        {
            Some(ContentCategory::Interview)
        } else if subjects_lower.iter().any(|s| s.contains("clip")) {
            // Generic clips default to featurette
            Some(ContentCategory::Featurette)
        } else {
            None
        }
    }

    /// Infer content category from title and description when subjects are missing
    fn infer_category_from_text(title: &str, description: Option<&str>) -> Option<ContentCategory> {
        let title_lower = title.to_lowercase();
        let desc_lower = description.map(|d| d.to_lowercase()).unwrap_or_default();
        let combined = format!("{} {}", title_lower, desc_lower);

        if combined.contains("behind the scenes") || combined.contains("making of") {
            Some(ContentCategory::BehindTheScenes)
        } else if combined.contains("deleted scene") {
            Some(ContentCategory::DeletedScene)
        } else if combined.contains("interview") || combined.contains("q&a") {
            Some(ContentCategory::Interview)
        } else if combined.contains("trailer") {
            Some(ContentCategory::Trailer)
        } else if combined.contains("featurette")
            || combined.contains("epk")
            || combined.contains("bonus")
        {
            Some(ContentCategory::Featurette)
        } else if combined.contains("bts") {
            // Common abbreviation for behind-the-scenes
            Some(ContentCategory::BehindTheScenes)
        } else {
            // DVDXtras items without clear category default to Featurette
            None
        }
    }

    /// Execute a search query against Archive.org
    async fn execute_search(&self, query: &str) -> Result<Vec<ArchiveOrgDoc>, DiscoveryError> {
        let url = format!(
            "https://archive.org/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&fl[]=subject&fl[]=description&fl[]=collection&rows=15&output=json",
            urlencoding::encode(query)
        );

        debug!("Archive.org query: {}", query);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("Archive.org search request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("Archive.org search failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "Archive.org API returned status {}",
                status
            )));
        }

        let search_result: ArchiveOrgSearchResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Archive.org search response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        Ok(search_result.response.docs)
    }

    /// Search Archive.org general collection for a movie (pre-2010 only)
    async fn search_general(
        &self,
        title: &str,
        year: u16,
    ) -> Result<Vec<ArchiveOrgDoc>, DiscoveryError> {
        let query = Self::build_general_query(title, year);
        debug!("Searching Archive.org general collection for: {}", title);
        self.execute_search(&query).await
    }

    /// Search DVDXtras collection for EPK content (all years)
    async fn search_dvdxtras(&self, title: &str) -> Result<Vec<ArchiveOrgDoc>, DiscoveryError> {
        let query = Self::build_dvdxtras_query(title);
        debug!("Searching Archive.org DVDXtras for: {}", title);
        self.execute_search(&query).await
    }

    /// Convert Archive.org doc to VideoSource
    fn doc_to_video_source(doc: ArchiveOrgDoc) -> Option<VideoSource> {
        // Try to get category from subjects first
        let category = Self::map_subjects(&doc.subject).or_else(|| {
            // Fall back to inferring from title/description
            Self::infer_category_from_text(&doc.title, doc.description.as_deref())
        });

        // For DVDXtras items without clear category, default to Featurette
        let is_dvdxtras = doc.collection.iter().any(|c| c == "DVDXtras");
        let final_category = category.or(if is_dvdxtras {
            Some(ContentCategory::Featurette)
        } else {
            None
        });

        final_category.map(|cat| VideoSource {
            url: format!("https://archive.org/details/{}", doc.identifier),
            source_type: SourceType::ArchiveOrg,
            category: cat,
            title: doc.title,
        })
    }
}

impl ContentDiscoverer for ArchiveOrgDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!("Discovering Archive.org content for: {}", movie);

        let mut all_docs = Vec::new();

        // Search general Archive.org collection for pre-2010 movies
        if movie.year < 2010 {
            match self.search_general(&movie.title, movie.year).await {
                Ok(docs) => {
                    info!(
                        "Found {} results from Archive.org general for {}",
                        docs.len(),
                        movie
                    );
                    all_docs.extend(docs);
                }
                Err(e) => {
                    // Log but continue - DVDXtras search may still succeed
                    info!("Archive.org general search failed for {}: {}", movie, e);
                }
            }
        } else {
            debug!(
                "Skipping Archive.org general search for {} - year {} is >= 2010",
                movie, movie.year
            );
        }

        // Search DVDXtras collection for all movies (EPK content from DVD releases)
        match self.search_dvdxtras(&movie.title).await {
            Ok(docs) => {
                info!(
                    "Found {} results from Archive.org DVDXtras for {}",
                    docs.len(),
                    movie
                );
                all_docs.extend(docs);
            }
            Err(e) => {
                info!("Archive.org DVDXtras search failed for {}: {}", movie, e);
            }
        }

        // Deduplicate by identifier
        all_docs.sort_by(|a, b| a.identifier.cmp(&b.identifier));
        all_docs.dedup_by(|a, b| a.identifier == b.identifier);

        // Convert to VideoSource
        let sources: Vec<VideoSource> = all_docs
            .into_iter()
            .filter_map(Self::doc_to_video_source)
            .collect();

        info!(
            "Discovered {} Archive.org sources for: {}",
            sources.len(),
            movie
        );
        Ok(sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_general_query() {
        let query = ArchiveOrgDiscoverer::build_general_query("The Matrix", 1999);
        assert!(query.contains("title:\"The Matrix\""));
        assert!(query.contains("year:1999"));
        assert!(query.contains("mediatype:movies"));
        assert!(query.contains("subject:trailer"));
        assert!(query.contains("subject:featurette"));
    }

    #[test]
    fn test_build_dvdxtras_query() {
        let query = ArchiveOrgDiscoverer::build_dvdxtras_query("Shrek");
        assert!(query.contains("collection:DVDXtras"));
        assert!(query.contains("title:\"Shrek\""));
        assert!(query.contains("description:\"Shrek\""));
    }

    #[test]
    fn test_map_subjects_trailer() {
        let subjects = vec!["Trailer".to_string(), "Movie".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects),
            Some(ContentCategory::Trailer)
        );
    }

    #[test]
    fn test_map_subjects_behind_the_scenes() {
        let subjects = vec!["Behind the Scenes".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects),
            Some(ContentCategory::BehindTheScenes)
        );

        let subjects2 = vec!["Making of".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects2),
            Some(ContentCategory::BehindTheScenes)
        );
    }

    #[test]
    fn test_map_subjects_deleted_scene() {
        let subjects = vec!["Deleted Scene".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects),
            Some(ContentCategory::DeletedScene)
        );
    }

    #[test]
    fn test_map_subjects_featurette() {
        let subjects = vec!["Featurette".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects),
            Some(ContentCategory::Featurette)
        );

        let subjects2 = vec!["epk".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects2),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_map_subjects_interview() {
        let subjects = vec!["Interview".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects),
            Some(ContentCategory::Interview)
        );

        let subjects2 = vec!["Q&A".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects2),
            Some(ContentCategory::Interview)
        );
    }

    #[test]
    fn test_map_subjects_clip_defaults_to_featurette() {
        let subjects = vec!["Clip".to_string()];
        assert_eq!(
            ArchiveOrgDiscoverer::map_subjects(&subjects),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_map_subjects_unknown() {
        let subjects = vec!["Random".to_string()];
        assert_eq!(ArchiveOrgDiscoverer::map_subjects(&subjects), None);
    }

    #[test]
    fn test_infer_category_from_title_bts() {
        assert_eq!(
            ArchiveOrgDiscoverer::infer_category_from_text("Movie BTS Footage", None),
            Some(ContentCategory::BehindTheScenes)
        );

        assert_eq!(
            ArchiveOrgDiscoverer::infer_category_from_text("Behind the Scenes", None),
            Some(ContentCategory::BehindTheScenes)
        );

        assert_eq!(
            ArchiveOrgDiscoverer::infer_category_from_text("Making of the Movie", None),
            Some(ContentCategory::BehindTheScenes)
        );
    }

    #[test]
    fn test_infer_category_from_title_deleted() {
        assert_eq!(
            ArchiveOrgDiscoverer::infer_category_from_text("Deleted Scene 1", None),
            Some(ContentCategory::DeletedScene)
        );
    }

    #[test]
    fn test_infer_category_from_title_interview() {
        assert_eq!(
            ArchiveOrgDiscoverer::infer_category_from_text("Cast Interview", None),
            Some(ContentCategory::Interview)
        );
    }

    #[test]
    fn test_infer_category_from_description() {
        assert_eq!(
            ArchiveOrgDiscoverer::infer_category_from_text(
                "Movie Extra",
                Some("Behind the scenes footage from the set")
            ),
            Some(ContentCategory::BehindTheScenes)
        );
    }

    #[test]
    fn test_infer_category_epk() {
        assert_eq!(
            ArchiveOrgDiscoverer::infer_category_from_text("EPK Content", None),
            Some(ContentCategory::Featurette)
        );

        assert_eq!(
            ArchiveOrgDiscoverer::infer_category_from_text("Bonus Features", None),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_doc_to_video_source_with_subjects() {
        let doc = ArchiveOrgDoc {
            identifier: "test-video".to_string(),
            title: "Test Behind the Scenes".to_string(),
            subject: vec!["Behind the Scenes".to_string()],
            description: None,
            collection: vec![],
        };

        let source = ArchiveOrgDiscoverer::doc_to_video_source(doc);
        assert!(source.is_some());
        let source = source.expect("should have source");
        assert_eq!(source.url, "https://archive.org/details/test-video");
        assert_eq!(source.category, ContentCategory::BehindTheScenes);
        assert_eq!(source.source_type, SourceType::ArchiveOrg);
    }

    #[test]
    fn test_doc_to_video_source_dvdxtras_fallback() {
        // DVDXtras items without clear category should default to Featurette
        let doc = ArchiveOrgDoc {
            identifier: "dvdxtras-item".to_string(),
            title: "Some DVD Extra".to_string(),
            subject: vec![],
            description: None,
            collection: vec!["DVDXtras".to_string()],
        };

        let source = ArchiveOrgDiscoverer::doc_to_video_source(doc);
        assert!(source.is_some());
        let source = source.expect("should have source");
        assert_eq!(source.category, ContentCategory::Featurette);
    }

    #[test]
    fn test_doc_to_video_source_infers_from_title() {
        let doc = ArchiveOrgDoc {
            identifier: "inferred-item".to_string(),
            title: "Making of the Movie".to_string(),
            subject: vec![],
            description: None,
            collection: vec![],
        };

        let source = ArchiveOrgDiscoverer::doc_to_video_source(doc);
        assert!(source.is_some());
        let source = source.expect("should have source");
        assert_eq!(source.category, ContentCategory::BehindTheScenes);
    }

    #[test]
    fn test_doc_to_video_source_no_category() {
        // Non-DVDXtras items without clear category should return None
        let doc = ArchiveOrgDoc {
            identifier: "unknown-item".to_string(),
            title: "Random Video".to_string(),
            subject: vec![],
            description: None,
            collection: vec!["other-collection".to_string()],
        };

        let source = ArchiveOrgDiscoverer::doc_to_video_source(doc);
        assert!(source.is_none());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Property 8: Archive.org Year-Based Querying (updated for DVDXtras)
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn prop_archive_general_query_year_based(year in 1900u16..2100u16) {
            let query = ArchiveOrgDiscoverer::build_general_query("Test Movie", year);

            // General query should always include year constraint
            let year_str = format!("year:{}", year);
            prop_assert!(query.contains(&year_str));
            prop_assert!(query.contains("mediatype:movies"));
        }
    }

    // Property: DVDXtras query does not include year constraint
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn prop_dvdxtras_query_no_year(title in "[A-Za-z ]{1,50}") {
            let query = ArchiveOrgDiscoverer::build_dvdxtras_query(&title);

            // DVDXtras query should search by collection, not year
            prop_assert!(query.contains("collection:DVDXtras"));
            prop_assert!(!query.contains("year:"));
        }
    }

    // Property 9: Archive.org Query Construction
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn prop_archive_query_construction(
            title in "[A-Za-z0-9 ]{1,50}",
            year in 1900u16..2100u16
        ) {
            let general_query = ArchiveOrgDiscoverer::build_general_query(&title, year);
            let dvdxtras_query = ArchiveOrgDiscoverer::build_dvdxtras_query(&title);

            // General query should contain title and year
            let title_pattern = format!("title:\"{}\"", title);
            let year_pattern = format!("year:{}", year);
            prop_assert!(general_query.contains(&title_pattern));
            prop_assert!(general_query.contains(&year_pattern));

            // DVDXtras query should contain title
            prop_assert!(dvdxtras_query.contains(&title_pattern));
        }
    }

    // Property: Subject mapping is deterministic
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn prop_subject_mapping_deterministic(
            subject in prop::sample::select(vec![
                "Trailer", "Behind the Scenes", "Making of",
                "Deleted Scene", "Featurette", "EPK", "Interview",
                "Q&A", "Clip", "Random"
            ])
        ) {
            let subjects = vec![subject.to_string()];
            let result1 = ArchiveOrgDiscoverer::map_subjects(&subjects);
            let result2 = ArchiveOrgDiscoverer::map_subjects(&subjects);

            // Same input should always produce same output
            prop_assert_eq!(result1, result2);
        }
    }
}
