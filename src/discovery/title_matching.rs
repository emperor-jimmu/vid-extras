// Title matching and filtering logic for YouTube video discovery

use crate::models::ContentCategory;
use log::debug;

/// Normalize a title for comparison by removing special characters, brackets, and extra spaces
pub fn normalize_title(title: &str) -> String {
    // First, convert superscript numbers to regular numbers
    let mut title = title
        .replace('²', " 2")
        .replace('³', " 3")
        .replace('⁴', " 4")
        .replace('⁵', " 5");

    // Convert Roman numerals at word boundaries (must be surrounded by spaces or at end)
    // We need to be careful to only match standalone Roman numerals
    let roman_patterns = [
        (" II ", " 2 "),
        (" III ", " 3 "),
        (" IV ", " 4 "),
        (" V ", " 5 "),
    ];

    for (roman, arabic) in &roman_patterns {
        title = title.replace(roman, arabic);
    }

    // Handle Roman numerals at the end of the string
    if title.ends_with(" II") {
        title = title.strip_suffix(" II").unwrap().to_string() + " 2";
    } else if title.ends_with(" III") {
        title = title.strip_suffix(" III").unwrap().to_string() + " 3";
    } else if title.ends_with(" IV") {
        title = title.strip_suffix(" IV").unwrap().to_string() + " 4";
    } else if title.ends_with(" V") {
        title = title.strip_suffix(" V").unwrap().to_string() + " 5";
    }

    // Then apply standard normalization
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Check if video title contains the movie title (with normalization)
pub fn contains_movie_title(video_title: &str, movie_title: &str) -> bool {
    let normalized_video = normalize_title(video_title);
    let normalized_movie = normalize_title(movie_title);

    // Check if the normalized movie title appears in the normalized video title
    normalized_video.contains(&normalized_movie)
}

/// Check if a video title contains excluded keywords
pub fn contains_excluded_keywords(title: &str) -> bool {
    let excluded_keywords = [
        "Review",
        "Reaction",
        "Analysis",
        "Explained",
        "Ending",
        "Theory",
        "React",
        "Blooper",
        "Gag",
    ];

    let title_lower = title.to_lowercase();
    excluded_keywords
        .iter()
        .any(|keyword| title_lower.contains(&keyword.to_lowercase()))
}
/// Infer the content category from a video title based on keyword analysis.
/// Returns `None` if no strong signal is found, in which case the caller
/// should fall back to the search-query category.
pub fn infer_category_from_title(title: &str) -> Option<ContentCategory> {
    let lower = title.to_lowercase();

    // Order matters: check more specific patterns first.

    // Deleted scenes
    if lower.contains("deleted scene") || lower.contains("deleted clip") {
        return Some(ContentCategory::DeletedScene);
    }

    // Behind the scenes / making of
    if lower.contains("behind the scene")
        || lower.contains("making of")
        || lower.contains("on set")
        || lower.contains("on the set")
        || lower.contains("b-roll")
        || lower.contains("broll")
        || lower.contains("bts")
    {
        return Some(ContentCategory::BehindTheScenes);
    }

    // Interviews
    if lower.contains("interview")
        || lower.contains("q&a")
        || lower.contains("q & a")
        || lower.contains("press conference")
        || lower.contains("talks about")
        || lower.contains("podcast")
    {
        return Some(ContentCategory::Interview);
    }

    // Shorts — check before Trailer since "short film trailer" should be Short
    if lower.contains("short film")
        || lower.contains("animated short")
        || lower
            .split_whitespace()
            .any(|w| w.trim_matches(|c: char| !c.is_alphabetic()) == "short")
    {
        return Some(ContentCategory::Short);
    }

    // Trailers — check after BTS/interview since "behind the scenes trailer" should be BTS
    if lower.contains("trailer") || lower.contains("teaser") || lower.contains("promo") {
        return Some(ContentCategory::Trailer);
    }

    // Featurettes
    if lower.contains("featurette")
        || lower.contains("documentary")
        || lower.contains("bonus clip")
        || lower.contains("why we love")
    {
        return Some(ContentCategory::Featurette);
    }

    None
}
/// Extract season numbers mentioned in a video title.
///
/// Matches patterns like "Season 3", "season 03", "S03", "S05E02", "S3".
/// Returns a list of unique season numbers found.
pub fn extract_season_numbers(title: &str) -> Vec<u8> {
    use regex::Regex;
    use std::sync::LazyLock;

    static SEASON_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        vec![
            // "Season (1)", "Season(1)" - parentheses format
            Regex::new(r"(?i)\bseason\s*\(\s*(\d{1,2})\s*\)").expect("valid regex"),
            // "Season 3", "season 03", "Season 12" - standard format
            Regex::new(r"(?i)\bseason\s+(\d{1,2})\b").expect("valid regex"),
            // "S03E01", "S3E1", "S05" (with or without episode)
            Regex::new(r"(?i)\bS(\d{1,2})(?:E\d+)?\b").expect("valid regex"),
        ]
    });

    let mut seasons: Vec<u8> = Vec::new();
    for pattern in SEASON_PATTERNS.iter() {
        for cap in pattern.captures_iter(title) {
            if let Some(num_str) = cap.get(1)
                && let Ok(num) = num_str.as_str().parse::<u8>()
                && num > 0
                && !seasons.contains(&num)
            {
                seasons.push(num);
            }
        }
    }
    seasons
}

/// Check if a video title references a season that is not in the available seasons list.
///
/// Returns true if the title mentions at least one specific season AND none of those
/// seasons are in the available list. Returns false if no season is mentioned (general content).
pub fn references_unavailable_season(title: &str, available_seasons: &[u8]) -> bool {
    let mentioned = extract_season_numbers(title);
    if mentioned.is_empty() {
        // No season reference — general series content, keep it
        return false;
    }
    // Exclude if none of the mentioned seasons are available on disk
    !mentioned.iter().any(|s| available_seasons.contains(s))
}

/// Convert Roman numeral to integer (supports I-XIX, i.e., 1-19)
/// Returns None if the string is not a valid Roman numeral
pub fn roman_to_int(roman: &str) -> Option<u32> {
    let roman_upper = roman.to_uppercase();

    // Map of Roman numeral characters to values
    let char_values = |c: char| -> Option<u32> {
        match c {
            'I' => Some(1),
            'V' => Some(5),
            'X' => Some(10),
            _ => None,
        }
    };

    let chars: Vec<char> = roman_upper.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let mut result = 0;
    let mut i = 0;

    while i < chars.len() {
        let current = char_values(chars[i])?;

        if i + 1 < chars.len() {
            let next = char_values(chars[i + 1])?;

            // Subtractive notation (e.g., IV = 4, IX = 9)
            if current < next {
                result += next - current;
                i += 2;
                continue;
            }
        }

        result += current;
        i += 1;
    }

    // Only return valid sequel numbers (2-19)
    if (2..=19).contains(&result) {
        Some(result)
    } else {
        None
    }
}

/// Check if video title mentions a sequel number (e.g., "REC 2", "REC3", "[REC]2", "REC II")
/// This is a fallback for when TMDB doesn't provide collection information
pub fn mentions_sequel_number(video_title: &str, movie_title: &str) -> bool {
    let normalized_video = normalize_title(video_title);
    let normalized_video_no_spaces = normalized_video.replace(' ', "");
    let normalized_movie = normalize_title(movie_title);

    // Look for patterns like "rec 2", "rec2", "rec 3", "rec3", etc.
    // We check for numbers 2-19 (sequels)
    for num in 2..=19 {
        let with_space = format!("{} {}", normalized_movie, num);
        let without_space = format!("{}{}", normalized_movie, num);

        // Check if the pattern appears in the video title
        // But make sure it's not part of a year like "(2007)"
        if normalized_video.contains(&with_space)
            || normalized_video_no_spaces.contains(&without_space)
        {
            // Additional check: make sure the number isn't part of a 4-digit year
            // by checking if it's followed by more digits
            let year_pattern = format!("{} {}0", normalized_movie, num);
            let year_pattern_no_space = format!("{}{}0", normalized_movie, num);

            if normalized_video.contains(&year_pattern)
                || normalized_video_no_spaces.contains(&year_pattern_no_space)
            {
                // This looks like a year (e.g., "REC 2007"), not a sequel number
                continue;
            }

            debug!(
                "Detected sequel number {} in '{}' (movie: '{}')",
                num, video_title, movie_title
            );
            return true;
        }
    }

    // Check for Roman numerals (II-XIX)
    // Split the normalized video title into words and check each for Roman numerals
    for word in normalized_video.split_whitespace() {
        if let Some(num) = roman_to_int(word) {
            // Check if this Roman numeral appears after the movie title
            // by looking for patterns like "rec ii" or "rec iii"
            let roman_upper = word.to_uppercase();
            let with_space = format!("{} {}", normalized_movie, word);

            if normalized_video.contains(&with_space) {
                debug!(
                    "Detected sequel Roman numeral {} ({}) in '{}' (movie: '{}')",
                    roman_upper, num, video_title, movie_title
                );
                return true;
            }
        }
    }

    // Also check for Roman numerals without spaces (e.g., "recii", "reciii")
    // by checking if the video title (without spaces) contains movie+roman
    for num in 2..=19 {
        // Generate Roman numeral for this number
        let roman = match num {
            2 => "ii",
            3 => "iii",
            4 => "iv",
            5 => "v",
            6 => "vi",
            7 => "vii",
            8 => "viii",
            9 => "ix",
            10 => "x",
            11 => "xi",
            12 => "xii",
            13 => "xiii",
            14 => "xiv",
            15 => "xv",
            16 => "xvi",
            17 => "xvii",
            18 => "xviii",
            19 => "xix",
            _ => continue,
        };

        let without_space = format!("{}{}", normalized_movie, roman);

        if normalized_video_no_spaces.contains(&without_space) {
            debug!(
                "Detected sequel Roman numeral {} ({}) in '{}' (movie: '{}')",
                roman.to_uppercase(),
                num,
                video_title,
                movie_title
            );
            return true;
        }
    }

    false
}

/// Check if video title mentions a different year (potential sequel/different movie)
pub fn mentions_different_year(title: &str, expected_year: u16) -> bool {
    // Look for 4-digit years in the title
    let year_regex = regex::Regex::new(r"\b(19\d{2}|20\d{2})\b").unwrap();

    for capture in year_regex.captures_iter(title) {
        if let Some(year_str) = capture.get(1)
            && let Ok(found_year) = year_str.as_str().parse::<u16>()
        {
            // If we find a different year, this might be about a sequel or different movie
            if found_year != expected_year {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_title_basic() {
        assert_eq!(normalize_title("The Matrix"), "the matrix");
        assert_eq!(normalize_title("INCEPTION"), "inception");
    }

    #[test]
    fn test_normalize_title_with_superscripts() {
        assert_eq!(normalize_title("Movie²"), "movie 2");
        assert_eq!(normalize_title("Movie³"), "movie 3");
    }

    #[test]
    fn test_normalize_title_with_roman_numerals() {
        assert_eq!(normalize_title("REC II"), "rec 2");
        assert_eq!(normalize_title("REC III"), "rec 3");
    }

    #[test]
    fn test_contains_movie_title() {
        assert!(contains_movie_title("REC Official Trailer", "REC"));
        assert!(contains_movie_title(
            "The Matrix Behind the Scenes",
            "The Matrix"
        ));
        assert!(!contains_movie_title("Inception Trailer", "The Matrix"));
    }

    #[test]
    fn test_contains_excluded_keywords() {
        assert!(contains_excluded_keywords("Movie Review"));
        assert!(contains_excluded_keywords("Reaction Video"));
        assert!(contains_excluded_keywords("Analysis"));
        assert!(!contains_excluded_keywords("Official Trailer"));
    }

    #[test]
    fn test_roman_to_int() {
        assert_eq!(roman_to_int("II"), Some(2));
        assert_eq!(roman_to_int("III"), Some(3));
        assert_eq!(roman_to_int("IV"), Some(4));
        assert_eq!(roman_to_int("XIX"), Some(19));
        assert_eq!(roman_to_int("I"), None); // 1 is not a valid sequel number
        assert_eq!(roman_to_int("XX"), None); // 20 is out of range
    }

    #[test]
    fn test_mentions_sequel_number() {
        assert!(mentions_sequel_number("REC 2 Trailer", "REC"));
        assert!(mentions_sequel_number("REC2 Behind the Scenes", "REC"));
        assert!(!mentions_sequel_number("REC Official Trailer", "REC"));
    }

    #[test]
    fn test_mentions_different_year() {
        assert!(mentions_different_year("REC 2009 Trailer", 2007));
        assert!(!mentions_different_year("REC 2007 Trailer", 2007));
        assert!(!mentions_different_year("REC Official Trailer", 2007));
    }

    #[test]
    fn test_extract_season_numbers() {
        // "Season N" format
        assert_eq!(
            extract_season_numbers("Breaking Bad Extras Season 3"),
            vec![3]
        );
        assert_eq!(
            extract_season_numbers("Season 1 Behind the Scenes"),
            vec![1]
        );
        assert_eq!(extract_season_numbers("season 03 extras"), vec![3]);

        // "Season (N)" format with parentheses
        assert_eq!(
            extract_season_numbers("Breaking Bad (2008) - Season (1) Extras - The Writer s Lab"),
            vec![1]
        );
        assert_eq!(
            extract_season_numbers("Season (2) Behind the Scenes"),
            vec![2]
        );

        // "SxxExx" format
        assert_eq!(extract_season_numbers("Breaking Bad S03E01"), vec![3]);
        assert_eq!(extract_season_numbers("S5 Deleted Scenes"), vec![5]);

        // Multiple seasons
        let mut result = extract_season_numbers("Season 1 and Season 3 recap");
        result.sort();
        assert_eq!(result, vec![1, 3]);

        // No season reference
        assert!(extract_season_numbers("Breaking Bad Cast Interview").is_empty());
        assert!(extract_season_numbers("Behind the Scenes").is_empty());

        // Season 0 is excluded (must be > 0)
        assert!(extract_season_numbers("Season 0 Specials").is_empty());
    }

    #[test]
    fn test_references_unavailable_season() {
        let available = vec![1, 2];

        // Should exclude: mentions season 3, only 1 and 2 on disk
        assert!(references_unavailable_season(
            "Bryan Cranston & Aaron Paul Answers Fan Questions | Breaking Bad Extras Season 3",
            &available
        ));

        // Should keep: mentions season 1, which is available
        assert!(!references_unavailable_season(
            "Breaking Bad Season 1 Behind the Scenes",
            &available
        ));

        // Should keep: no season reference at all (general content)
        assert!(!references_unavailable_season(
            "Breaking Bad Cast Interview",
            &available
        ));

        // Should exclude: S03 format, season 3 not available
        assert!(references_unavailable_season(
            "Breaking Bad S03 Deleted Scenes",
            &available
        ));

        // Should keep: mentions both season 1 and 3, season 1 is available
        assert!(!references_unavailable_season(
            "Season 1 and Season 3 comparison",
            &available
        ));
    }

    #[test]
    fn test_infer_category_from_title_trailer() {
        assert_eq!(
            infer_category_from_title("A TOUCH OF ZEN Trailer [1971]"),
            Some(ContentCategory::Trailer)
        );
        assert_eq!(
            infer_category_from_title("Official Teaser - Movie Name"),
            Some(ContentCategory::Trailer)
        );
    }

    #[test]
    fn test_infer_category_from_title_deleted_scene() {
        assert_eq!(
            infer_category_from_title("COBRA \"Deleted Scenes\" (1986) Stallone"),
            Some(ContentCategory::DeletedScene)
        );
    }

    #[test]
    fn test_infer_category_from_title_behind_the_scenes() {
        assert_eq!(
            infer_category_from_title("Coach Carter Movie - Behind the Scenes (2)"),
            Some(ContentCategory::BehindTheScenes)
        );
        assert_eq!(
            infer_category_from_title("Go Behind the Scenes of Money Monster (2016)"),
            Some(ContentCategory::BehindTheScenes)
        );
        assert_eq!(
            infer_category_from_title("Making of The Matrix"),
            Some(ContentCategory::BehindTheScenes)
        );
    }

    #[test]
    fn test_infer_category_from_title_interview() {
        assert_eq!(
            infer_category_from_title("George Clooney Interview - Money Monster"),
            Some(ContentCategory::Interview)
        );
        assert_eq!(
            infer_category_from_title("The Real Coach Carter Talks About The Movie"),
            Some(ContentCategory::Interview)
        );
        assert_eq!(
            infer_category_from_title("MONEY MONSTER - Press Conference - Cannes 2016"),
            Some(ContentCategory::Interview)
        );
    }

    #[test]
    fn test_infer_category_from_title_featurette() {
        assert_eq!(
            infer_category_from_title("Coach Carter Documentary - the real coach carter"),
            Some(ContentCategory::Featurette)
        );
        assert_eq!(
            infer_category_from_title("Cobra (1986) - Bonus Clip: Actor Brian Thompson"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_infer_category_from_title_none_when_ambiguous() {
        assert_eq!(
            infer_category_from_title("Coach Carter - Go to College"),
            None
        );
        assert_eq!(infer_category_from_title("a touch of zen (俠女)"), None);
    }

    #[test]
    fn test_infer_category_from_title_short() {
        // Compound phrases
        assert_eq!(
            infer_category_from_title("Pixar Short Film: Bao"),
            Some(ContentCategory::Short)
        );
        assert_eq!(
            infer_category_from_title("Disney Animated Short Collection"),
            Some(ContentCategory::Short)
        );
        // Bare "short" as standalone word
        assert_eq!(
            infer_category_from_title("A Short by the Director"),
            Some(ContentCategory::Short)
        );
        // "short" with trailing punctuation (e.g. "short:" or "short.")
        assert_eq!(
            infer_category_from_title("Pixar Short: Bao"),
            Some(ContentCategory::Short)
        );
        assert_eq!(
            infer_category_from_title("Director's Short."),
            Some(ContentCategory::Short)
        );
        // False positives must NOT match
        assert_eq!(infer_category_from_title("Shortcut to Hollywood"), None);
        assert_eq!(
            infer_category_from_title("Shortly after the premiere"),
            None
        );
    }

    #[test]
    fn test_infer_category_bts_trailer_priority() {
        // "Behind the Scenes" in title should win over "Trailer" substring
        assert_eq!(
            infer_category_from_title("Behind the Scenes Trailer for Movie X"),
            Some(ContentCategory::BehindTheScenes)
        );
    }
}
