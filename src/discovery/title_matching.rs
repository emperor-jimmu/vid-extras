// Title matching and filtering logic for YouTube video discovery

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

/// Check if video title mentions other movies from the collection (with normalization)
pub fn mentions_collection_movies(video_title: &str, collection_titles: &[String]) -> bool {
    if collection_titles.is_empty() {
        return false;
    }

    let normalized_video = normalize_title(video_title);
    let normalized_video_no_spaces = normalized_video.replace(' ', "");

    // Check if any normalized collection movie title appears in the normalized video title
    // We check both with and without spaces to handle cases like "[Rec]3" vs "REC 3"
    collection_titles.iter().any(|title| {
        let normalized_collection = normalize_title(title);
        let normalized_collection_no_spaces = normalized_collection.replace(' ', "");

        // Check both versions to handle spacing variations
        normalized_video.contains(&normalized_collection)
            || normalized_video_no_spaces.contains(&normalized_collection_no_spaces)
    })
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
/// Extract season numbers mentioned in a video title.
///
/// Matches patterns like "Season 3", "season 03", "S03", "S05E02", "S3".
/// Returns a list of unique season numbers found.
pub fn extract_season_numbers(title: &str) -> Vec<u8> {
    use regex::Regex;
    use std::sync::LazyLock;

    static SEASON_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        vec![
            // "Season 3", "season 03", "Season 12"
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
    fn test_mentions_collection_movies() {
        let collection = vec!["The Matrix Reloaded".to_string()];
        assert!(mentions_collection_movies(
            "The Matrix Reloaded Trailer",
            &collection
        ));
        assert!(!mentions_collection_movies(
            "The Matrix Original Trailer",
            &collection
        ));
    }
}
