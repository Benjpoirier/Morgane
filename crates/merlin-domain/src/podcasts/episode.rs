use chrono::{DateTime, Utc};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub guid: String,
    pub title: String,
    pub audio_url: String,
    pub image_url: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Podcast {
    pub feed_url: String,
    pub title: String,
    pub image_url: Option<String>,
    pub episodes: Vec<Episode>,
}

pub mod episode_numbering {
    use super::*;

    static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        [
            r"(?i)chapitre\s*(\d+)",
            r"(?i)[ée]pisode\s*(\d+)",
            r"(?i)partie\s*(\d+)",
            r"(?i)n[°o]\s*(\d+)",
            r"(?i)#\s*(\d+)",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("motif de numérotation invalide"))
        .collect()
    });

    fn first_match(title: &str) -> Option<(std::ops::Range<usize>, String)> {
        for pattern in PATTERNS.iter() {
            if let Some(captures) = pattern.captures(title) {
                let full = captures.get(0).expect("match complet toujours présent");
                let number = captures.get(1).expect("groupe 1 toujours présent");
                return Some((full.range(), number.as_str().to_string()));
            }
        }
        None
    }

    pub fn guess_number(title: &str) -> Option<i64> {
        let (_, number) = first_match(title)?;
        number.parse().ok()
    }

    pub fn guess_group_title(title: &str) -> Option<String> {
        let (full_range, _) = first_match(title)?;
        let prefix = &title[..full_range.start];
        let trimmed = prefix.trim_matches(|c: char| c.is_whitespace() || " -:|,".contains(c));
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::episode_numbering::*;

    #[test]
    fn guesses_number_from_french_patterns() {
        assert_eq!(guess_number("Mystère à l'école - Chapitre 8"), Some(8));
        assert_eq!(guess_number("Épisode 12 : la suite"), Some(12));
        assert_eq!(
            guess_number("episode 3"),
            Some(3),
            "insensible à la casse et aux accents"
        );
        assert_eq!(guess_number("Partie 2"), Some(2));
        assert_eq!(guess_number("Aventures n°5"), Some(5));
        assert_eq!(guess_number("Aventures no 7"), Some(7));
        assert_eq!(guess_number("Saison 1 #4"), Some(4));
        assert_eq!(guess_number("Sans numero"), None);
    }

    #[test]
    fn pattern_order_wins_over_position_in_title() {
        assert_eq!(guess_number("partie 2 chapitre 3"), Some(3));
    }

    #[test]
    fn guesses_group_title_and_strips_trailing_separators() {
        assert_eq!(
            guess_group_title("Mystère à l'école - Chapitre 8"),
            Some("Mystère à l'école".to_string())
        );
        assert_eq!(
            guess_group_title("Les Aventures : Épisode 2"),
            Some("Les Aventures".to_string())
        );
        assert_eq!(
            guess_group_title("Chapitre 8"),
            None,
            "préfixe vide -> pas de groupe"
        );
        assert_eq!(guess_group_title("Sans numero"), None);
    }
}
