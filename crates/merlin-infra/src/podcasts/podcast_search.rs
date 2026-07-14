use std::collections::HashMap;
use std::time::Duration;

use merlin_domain::podcasts::search::{PodcastSearchResult, deduplicate};
use serde_json::Value;
use tokio::task::JoinSet;

const ENDPOINT: &str = "https://itunes.apple.com/search";

const KIDS_CHART: &str = "https://itunes.apple.com/fr/rss/toppodcasts/genre=1305/limit=40/json";

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

pub async fn has_internet() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    else {
        return false;
    };
    client
        .head("https://itunes.apple.com/")
        .send()
        .await
        .is_ok()
}

async fn resolve_ranked(
    client: &reqwest::Client,
    items: Vec<Value>,
    rank_of: impl Fn(&Value) -> Option<usize>,
) -> Vec<PodcastSearchResult> {
    let mut tasks = JoinSet::new();
    for item in items {
        if let Some(rank) = rank_of(&item) {
            let client = client.clone();
            tasks.spawn(async move { (rank, resolve_item(&client, &item).await) });
        }
    }
    let mut indexed: Vec<(usize, PodcastSearchResult)> = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        if let Ok((rank, Some(result))) = joined {
            indexed.push((rank, result));
        }
    }
    indexed.sort_by_key(|(rank, _)| *rank);
    deduplicate(indexed.into_iter().map(|(_, result)| result).collect())
}

pub async fn popular_kids() -> Result<Vec<PodcastSearchResult>, String> {
    let client = build_client()?;
    let chart_body = client
        .get(KIDS_CHART)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let chart: Value = serde_json::from_slice(&chart_body).map_err(|e| e.to_string())?;
    let ids: Vec<String> = chart
        .get("feed")
        .and_then(|f| f.get("entry"))
        .and_then(|e| e.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| {
                    e.get("id")?
                        .get("attributes")?
                        .get("im:id")?
                        .as_str()
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rank: HashMap<String, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();

    let url = reqwest::Url::parse_with_params(
        "https://itunes.apple.com/lookup",
        &[
            ("id", ids.join(",").as_str()),
            ("entity", "podcast"),
            ("country", "FR"),
        ],
    )
    .map_err(|e| e.to_string())?;
    let body = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let items: Vec<Value> = json
        .get("results")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(resolve_ranked(&client, items, |item| {
        item.get("collectionId")
            .and_then(|v| v.as_u64())
            .and_then(|id| rank.get(&id.to_string()).copied())
    })
    .await)
}

pub async fn search(query: &str) -> Result<Vec<PodcastSearchResult>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let url = reqwest::Url::parse_with_params(
        ENDPOINT,
        &[
            ("media", "podcast"),
            ("country", "FR"),
            ("limit", "25"),
            ("term", query),
        ],
    )
    .map_err(|e| e.to_string())?;
    let client = build_client()?;
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("code HTTP {}", response.status().as_u16()));
    }
    let body = response.bytes().await.map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let items: Vec<Value> = json
        .get("results")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();

    let mut tasks = JoinSet::new();
    for (index, item) in items.into_iter().enumerate() {
        let client = client.clone();
        tasks.spawn(async move { (index, resolve_item(&client, &item).await) });
    }
    let mut indexed: Vec<(usize, PodcastSearchResult)> = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        if let Ok((index, Some(result))) = joined {
            indexed.push((index, result));
        }
    }
    indexed.sort_by_key(|(index, _)| *index);
    let results = indexed.into_iter().map(|(_, result)| result).collect();
    Ok(deduplicate(results))
}

async fn resolve_item(client: &reqwest::Client, item: &Value) -> Option<PodcastSearchResult> {
    let string = |key: &str| item.get(key).and_then(|v| v.as_str()).map(String::from);
    let title = string("collectionName").unwrap_or_else(|| "Sans titre".to_string());
    let author = string("artistName");

    let feed_url = match item.get("feedUrl").and_then(|v| v.as_str()) {
        Some(feed) => feed.to_string(),
        None => {
            let station = radiofrance_station(author.as_deref()?)?;
            resolve_radiofrance_feed(client, station, &title).await?
        }
    };

    Some(PodcastSearchResult {
        title,
        feed_url,
        image_url: string("artworkUrl600").or_else(|| string("artworkUrl100")),
        episode_count: item
            .get("trackCount")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32),
        genre: string("primaryGenreName"),
        author,
    })
}

fn radiofrance_station(author: &str) -> Option<&'static str> {
    let author = author.to_lowercase();
    if author.contains("france inter") {
        Some("franceinter")
    } else if author.contains("france culture") {
        Some("franceculture")
    } else if author.contains("france musique") {
        Some("francemusique")
    } else if author.contains("franceinfo") || author.contains("france info") {
        Some("franceinfo")
    } else if author.contains("france bleu") {
        Some("francebleu")
    } else if author.contains("mouv") {
        Some("mouv")
    } else {
        None
    }
}

async fn resolve_radiofrance_feed(
    client: &reqwest::Client,
    station: &str,
    title: &str,
) -> Option<String> {
    let page = format!(
        "https://www.radiofrance.fr/{station}/podcasts/{}",
        slugify(title)
    );
    let response = client
        .get(&page)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let html = response.text().await.ok()?;
    extract_radiofrance_feed(&html)
}

fn extract_radiofrance_feed(html: &str) -> Option<String> {
    html.split(['"', '\'', ' ', '<', '>', '(', ')'])
        .find(|token| {
            token.starts_with("http")
                && token.contains("radiofrance-podcast.net")
                && token.ends_with(".xml")
        })
        .map(String::from)
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for ch in title.chars() {
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        let ascii: &str = match lower {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => "a",
            'ç' => "c",
            'è' | 'é' | 'ê' | 'ë' => "e",
            'ì' | 'í' | 'î' | 'ï' => "i",
            'ñ' => "n",
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => "o",
            'ù' | 'ú' | 'û' | 'ü' => "u",
            'ý' | 'ÿ' => "y",
            'œ' => "oe",
            'æ' => "ae",
            c if c.is_ascii_alphanumeric() => {
                if pending_dash && !slug.is_empty() {
                    slug.push('-');
                }
                slug.push(c);
                pending_dash = false;
                continue;
            }
            _ => {
                pending_dash = true;
                continue;
            }
        };
        if pending_dash && !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(ascii);
        pending_dash = false;
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_matches_radiofrance_urls() {
        assert_eq!(
            slugify("Les aventures du professeur Caillou"),
            "les-aventures-du-professeur-caillou"
        );
        assert_eq!(slugify("Les Odyssées"), "les-odyssees");
        assert_eq!(slugify("Une histoire et… Oli"), "une-histoire-et-oli");
    }

    #[test]
    fn station_maps_known_authors_only() {
        assert_eq!(radiofrance_station("France Inter"), Some("franceinter"));
        assert_eq!(radiofrance_station("franceinfo"), Some("franceinfo"));
        assert_eq!(radiofrance_station("Acast"), None);
    }

    #[test]
    fn extracts_feed_from_page_markup() {
        let html = r#"<link href="https://radiofrance-podcast.net/podcast09/podcast_abc.xml"/>"#;
        assert_eq!(
            extract_radiofrance_feed(html).as_deref(),
            Some("https://radiofrance-podcast.net/podcast09/podcast_abc.xml"),
        );
        assert_eq!(extract_radiofrance_feed("<p>rien</p>"), None);
    }
}
