use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastSearchResult {
    pub title: String,
    pub feed_url: String,
    pub image_url: Option<String>,
    pub episode_count: Option<u32>,
    pub genre: Option<String>,
    pub author: Option<String>,
}

pub fn deduplicate(results: Vec<PodcastSearchResult>) -> Vec<PodcastSearchResult> {
    let mut best: Vec<PodcastSearchResult> = Vec::new();
    let mut position: HashMap<String, usize> = HashMap::new();
    for result in results {
        let key = dedup_key(&result);
        match position.get(&key) {
            Some(&index) => {
                if result.episode_count.unwrap_or(0) > best[index].episode_count.unwrap_or(0) {
                    best[index] = result;
                }
            }
            None => {
                position.insert(key, best.len());
                best.push(result);
            }
        }
    }
    best
}

fn dedup_key(result: &PodcastSearchResult) -> String {
    let normalize = |s: &str| s.trim().to_lowercase();
    format!(
        "{}|{}",
        normalize(&result.title),
        result.author.as_deref().map(normalize).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(
        title: &str,
        feed: &str,
        author: Option<&str>,
        episodes: Option<u32>,
    ) -> PodcastSearchResult {
        PodcastSearchResult {
            title: title.into(),
            feed_url: feed.into(),
            image_url: None,
            episode_count: episodes,
            genre: None,
            author: author.map(String::from),
        }
    }

    #[test]
    fn keeps_the_feed_with_the_most_episodes_for_the_same_show() {
        let deduped = deduplicate(vec![
            result(
                "Encore une histoire",
                "https://a/tronque",
                Some("Studio"),
                Some(5),
            ),
            result(
                "Encore une histoire",
                "https://a/complet",
                Some("Studio"),
                Some(180),
            ),
        ]);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].feed_url, "https://a/complet");
    }

    #[test]
    fn same_title_different_author_are_kept_separate() {
        let deduped = deduplicate(vec![
            result("Histoires", "https://a", Some("Auteur 1"), Some(10)),
            result("Histoires", "https://b", Some("Auteur 2"), Some(3)),
        ]);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn normalization_matches_case_and_whitespace() {
        let deduped = deduplicate(vec![
            result(
                "  Les Odyssées ",
                "https://a",
                Some("France Inter"),
                Some(4),
            ),
            result("les odyssées", "https://b", Some("france inter"), Some(120)),
        ]);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].feed_url, "https://b");
    }

    #[test]
    fn a_missing_episode_count_never_wins_over_a_known_one() {
        let deduped = deduplicate(vec![
            result("X", "https://known", Some("A"), Some(1)),
            result("X", "https://unknown", Some("A"), None),
        ]);
        assert_eq!(deduped[0].feed_url, "https://known");
    }

    #[test]
    fn preserves_first_seen_order() {
        let deduped = deduplicate(vec![
            result("B", "https://b", None, Some(1)),
            result("A", "https://a", None, Some(1)),
        ]);
        assert_eq!(
            deduped.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(),
            ["B", "A"]
        );
    }
}
