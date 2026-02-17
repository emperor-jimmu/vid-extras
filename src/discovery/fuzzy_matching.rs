use log::debug;

/// Handles fuzzy title matching for series and extras
#[allow(dead_code)]
pub struct FuzzyMatcher;

#[allow(dead_code)]
impl FuzzyMatcher {
    /// Normalize a string for comparison (lowercase, remove special characters)
    pub fn normalize(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Calculate Levenshtein distance between two strings
    pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let len1 = s1_chars.len();
        let len2 = s2_chars.len();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for (i, row) in matrix.iter_mut().enumerate() {
            row[0] = i;
        }
        for (j, cell) in matrix[0].iter_mut().enumerate() {
            *cell = j;
        }

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                    0
                } else {
                    1
                };

                matrix[i][j] = std::cmp::min(
                    std::cmp::min(
                        matrix[i - 1][j] + 1, // deletion
                        matrix[i][j - 1] + 1, // insertion
                    ),
                    matrix[i - 1][j - 1] + cost, // substitution
                );
            }
        }

        matrix[len1][len2]
    }

    /// Calculate similarity score as a percentage (0-100)
    pub fn similarity_score(s1: &str, s2: &str) -> u8 {
        let max_len = std::cmp::max(s1.chars().count(), s2.chars().count());
        if max_len == 0 {
            return 100;
        }

        let distance = Self::levenshtein_distance(s1, s2);
        let similarity = ((max_len - distance) as f64 / max_len as f64) * 100.0;

        similarity.round() as u8
    }

    /// Check if two titles match with 80% similarity threshold
    pub fn titles_match(title1: &str, title2: &str) -> bool {
        let normalized1 = Self::normalize(title1);
        let normalized2 = Self::normalize(title2);

        let score = Self::similarity_score(&normalized1, &normalized2);
        debug!("Fuzzy match: '{}' vs '{}' -> {}%", title1, title2, score);

        score >= 80
    }

    /// Get similarity score for two titles (after normalization)
    pub fn get_similarity_score(title1: &str, title2: &str) -> u8 {
        let normalized1 = Self::normalize(title1);
        let normalized2 = Self::normalize(title2);

        let score = Self::similarity_score(&normalized1, &normalized2);
        debug!(
            "Similarity score: '{}' vs '{}' -> {}%",
            title1, title2, score
        );

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_normalize_lowercase() {
        let normalized = FuzzyMatcher::normalize("HELLO WORLD");
        assert_eq!(normalized, "hello world");
    }

    #[test]
    fn test_normalize_removes_special_chars() {
        let normalized = FuzzyMatcher::normalize("Hello-World!");
        assert_eq!(normalized, "helloworld");
    }

    #[test]
    fn test_normalize_multiple_spaces() {
        let normalized = FuzzyMatcher::normalize("Hello    World");
        assert_eq!(normalized, "hello world");
    }

    #[test]
    fn test_levenshtein_identical() {
        let distance = FuzzyMatcher::levenshtein_distance("hello", "hello");
        assert_eq!(distance, 0);
    }

    #[test]
    fn test_levenshtein_one_char_diff() {
        let distance = FuzzyMatcher::levenshtein_distance("hello", "hallo");
        assert_eq!(distance, 1);
    }

    #[test]
    fn test_levenshtein_empty_string() {
        let distance = FuzzyMatcher::levenshtein_distance("hello", "");
        assert_eq!(distance, 5);
    }

    #[test]
    fn test_similarity_score_identical() {
        let score = FuzzyMatcher::similarity_score("hello", "hello");
        assert_eq!(score, 100);
    }

    #[test]
    fn test_similarity_score_very_different() {
        let score = FuzzyMatcher::similarity_score("abc", "xyz");
        assert!(score < 50);
    }

    #[test]
    fn test_titles_match_exact() {
        assert!(FuzzyMatcher::titles_match("Breaking Bad", "Breaking Bad"));
    }

    #[test]
    fn test_titles_match_case_insensitive() {
        assert!(FuzzyMatcher::titles_match("Breaking Bad", "breaking bad"));
    }

    #[test]
    fn test_titles_match_with_special_chars() {
        assert!(FuzzyMatcher::titles_match("Breaking Bad", "Breaking-Bad!"));
    }

    #[test]
    fn test_titles_match_minor_typo() {
        assert!(FuzzyMatcher::titles_match("Breaking Bad", "Braking Bad"));
    }

    #[test]
    fn test_titles_no_match_very_different() {
        assert!(!FuzzyMatcher::titles_match(
            "Breaking Bad",
            "Game of Thrones"
        ));
    }

    #[test]
    fn test_get_similarity_score() {
        let score = FuzzyMatcher::get_similarity_score("Breaking Bad", "Breaking Bad");
        assert_eq!(score, 100);
    }

    #[test]
    fn test_get_similarity_score_with_typo() {
        let score = FuzzyMatcher::get_similarity_score("Breaking Bad", "Braking Bad");
        assert!(score >= 80);
    }

    // Property 17: Fuzzy Title Matching Threshold
    // Validates: Requirements 17.1, 17.2, 17.3, 17.4
    proptest! {
        #[test]
        fn prop_identical_strings_match(
            text in "[a-zA-Z0-9 ]{1,50}"
        ) {
            // Identical strings should always match
            assert!(FuzzyMatcher::titles_match(&text, &text));
            let score = FuzzyMatcher::get_similarity_score(&text, &text);
            prop_assert_eq!(score, 100);
        }

        #[test]
        fn prop_normalized_strings_are_lowercase(
            text in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let normalized = FuzzyMatcher::normalize(&text);
            let lowercase = normalized.to_lowercase();
            prop_assert_eq!(normalized, lowercase);
        }

        #[test]
        fn prop_similarity_score_range(
            text1 in "[a-zA-Z0-9 ]{1,50}",
            text2 in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let score = FuzzyMatcher::get_similarity_score(&text1, &text2);
            prop_assert!(score <= 100, "Score should be <= 100, got {}", score);
        }

        #[test]
        fn prop_threshold_80_percent(
            text in "[a-zA-Z0-9 ]{1,50}"
        ) {
            // Identical strings should match (>= 80%)
            assert!(FuzzyMatcher::titles_match(&text, &text));

            // Get the score to verify it's >= 80
            let score = FuzzyMatcher::get_similarity_score(&text, &text);
            prop_assert!(score >= 80, "Identical strings should have score >= 80, got {}", score);
        }

        #[test]
        fn prop_levenshtein_symmetry(
            text1 in "[a-zA-Z0-9]{1,20}",
            text2 in "[a-zA-Z0-9]{1,20}"
        ) {
            let dist1 = FuzzyMatcher::levenshtein_distance(&text1, &text2);
            let dist2 = FuzzyMatcher::levenshtein_distance(&text2, &text1);
            prop_assert_eq!(dist1, dist2, "Levenshtein distance should be symmetric");
        }

        #[test]
        fn prop_similarity_score_symmetry(
            text1 in "[a-zA-Z0-9 ]{1,20}",
            text2 in "[a-zA-Z0-9 ]{1,20}"
        ) {
            let score1 = FuzzyMatcher::get_similarity_score(&text1, &text2);
            let score2 = FuzzyMatcher::get_similarity_score(&text2, &text1);
            prop_assert_eq!(score1, score2, "Similarity score should be symmetric");
        }

        #[test]
        fn prop_match_result_consistency(
            text1 in "[a-zA-Z0-9 ]{1,20}",
            text2 in "[a-zA-Z0-9 ]{1,20}"
        ) {
            let matches = FuzzyMatcher::titles_match(&text1, &text2);
            let score = FuzzyMatcher::get_similarity_score(&text1, &text2);

            // If match returns true, score should be >= 80
            if matches {
                prop_assert!(score >= 80, "Match returned true but score is {}", score);
            } else {
                // If match returns false, score should be < 80
                prop_assert!(score < 80, "Match returned false but score is {}", score);
            }
        }
    }
}
