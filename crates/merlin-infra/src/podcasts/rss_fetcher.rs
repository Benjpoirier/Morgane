use std::time::Duration;

use merlin_domain::podcasts::episode::Podcast;
use merlin_domain::podcasts::rss_parser::{self, RssParserError};

pub async fn fetch(feed_url: &str) -> Result<Podcast, RssParserError> {
    if !(feed_url.starts_with("http://") || feed_url.starts_with("https://")) {
        return Err(RssParserError::InvalidUrl);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| RssParserError::FetchFailed(e.to_string()))?;
    let response = client
        .get(feed_url)
        .send()
        .await
        .map_err(|e| RssParserError::FetchFailed(e.to_string()))?;
    if !response.status().is_success() {
        return Err(RssParserError::FetchFailed(format!(
            "code HTTP {}",
            response.status().as_u16()
        )));
    }
    let data = response
        .bytes()
        .await
        .map_err(|e| RssParserError::FetchFailed(e.to_string()))?;
    rss_parser::parse(&data, feed_url).ok_or(RssParserError::ParseFailed)
}
