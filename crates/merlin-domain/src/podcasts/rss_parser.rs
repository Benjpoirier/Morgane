use chrono::{DateTime, Utc};

use super::episode::{Episode, Podcast};

#[derive(Debug, Clone, thiserror::Error)]
pub enum RssParserError {
    #[error("URL de flux invalide")]
    InvalidUrl,
    #[error("impossible de recuperer le flux : {0}")]
    FetchFailed(String),
    #[error("impossible de lire ce flux RSS")]
    ParseFailed,
}

#[derive(Default)]
struct ParserState {
    feed_title: String,
    feed_image_url: Option<String>,
    episodes: Vec<Episode>,

    path: Vec<String>,
    current_text: String,

    item_guid: Option<String>,
    item_title: Option<String>,
    item_audio_url: Option<String>,
    item_image_url: Option<String>,
    item_published_at: Option<DateTime<Utc>>,
    item_duration: Option<String>,
}

impl ParserState {
    fn in_item(&self) -> bool {
        self.path.iter().any(|e| e == "item")
    }

    fn parent_element(&self) -> Option<&str> {
        if self.path.len() >= 2 {
            Some(self.path[self.path.len() - 2].as_str())
        } else {
            None
        }
    }

    fn handle_start(&mut self, element: &str, attributes: &[(String, String)]) {
        self.path.push(element.to_string());
        self.current_text.clear();

        if element == "item" {
            self.item_guid = None;
            self.item_title = None;
            self.item_audio_url = None;
            self.item_image_url = None;
            self.item_published_at = None;
            self.item_duration = None;
        }

        let attr = |name: &str| {
            attributes
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        };

        if element == "enclosure"
            && let Some(href) = attr("url")
        {
            let content_type = attr("type").unwrap_or_default();
            if content_type.contains("audio") || has_audio_extension(&href) {
                self.item_audio_url = Some(href);
            }
        }
        if element == "itunes:image"
            && let Some(href) = attr("href")
        {
            if self.in_item() {
                self.item_image_url = Some(href);
            } else {
                self.feed_image_url = Some(href);
            }
        }
    }

    fn handle_end(&mut self, element: &str) {
        let text = self.current_text.trim().to_string();
        self.current_text.clear();

        match element {
            "title" if !self.in_item() && self.parent_element() != Some("image") => {
                self.feed_title = text;
            }
            "url" if self.parent_element() == Some("image") && !self.in_item() => {
                if self.feed_image_url.is_none() && !text.is_empty() {
                    self.feed_image_url = Some(text);
                }
            }
            "title" => self.item_title = Some(text),
            "guid" => self.item_guid = Some(text),
            "pubDate" => self.item_published_at = parse_rfc822_date(&text),
            "itunes:duration" => self.item_duration = Some(text),
            "item" => {
                if let Some(audio_url) = self.item_audio_url.take() {
                    self.episodes.push(Episode {
                        guid: self.item_guid.take().unwrap_or_else(|| audio_url.clone()),
                        title: self
                            .item_title
                            .take()
                            .unwrap_or_else(|| "Episode sans titre".to_string()),
                        audio_url,
                        image_url: self
                            .item_image_url
                            .take()
                            .or_else(|| self.feed_image_url.clone()),
                        published_at: self.item_published_at.take(),
                        duration: self.item_duration.take(),
                    });
                }
            }
            _ => {}
        }

        self.path.pop();
    }
}

pub fn parse(data: &[u8], feed_url: &str) -> Option<Podcast> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_reader(data);
    let mut state = ParserState::default();
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let attributes = read_attributes(&e, reader.decoder())?;
                state.handle_start(&name, &attributes);
            }

            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let attributes = read_attributes(&e, reader.decoder())?;
                state.handle_start(&name, &attributes);
                state.handle_end(&name);
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                state.handle_end(&name);
            }
            Ok(Event::Text(e)) => {
                let Ok(text) = e.decode() else { return None };
                state.current_text.push_str(&text);
            }

            Ok(Event::GeneralRef(e)) => match e.resolve_char_ref() {
                Ok(Some(character)) => state.current_text.push(character),
                Ok(None) => {
                    let Ok(name) = e.decode() else { return None };
                    if let Some(character) = named_entity(name.as_ref()) {
                        state.current_text.push(character);
                    }
                }
                Err(_) => return None,
            },

            Ok(Event::CData(e)) => {
                state.current_text.push_str(&String::from_utf8_lossy(&e));
            }

            Ok(Event::Eof) => {
                if !state.path.is_empty() {
                    return None;
                }
                break;
            }
            Ok(_) => {}

            Err(_) => return None,
        }
        buffer.clear();
    }

    Some(Podcast {
        feed_url: feed_url.to_string(),
        title: if state.feed_title.is_empty() {
            feed_url.to_string()
        } else {
            state.feed_title
        },
        image_url: state.feed_image_url,
        episodes: state.episodes,
    })
}

fn named_entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{00a0}',
        "hellip" => '…',
        "middot" => '·',
        "bull" => '•',
        "laquo" => '«',
        "raquo" => '»',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "ldquo" => '\u{201c}',
        "rdquo" => '\u{201d}',
        "ndash" => '–',
        "mdash" => '—',
        "deg" => '°',
        "times" => '×',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "euro" => '€',
        "agrave" => 'à',
        "acirc" => 'â',
        "auml" => 'ä',
        "aelig" => 'æ',
        "ccedil" => 'ç',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "iacute" => 'í',
        "igrave" => 'ì',
        "icirc" => 'î',
        "iuml" => 'ï',
        "ntilde" => 'ñ',
        "oacute" => 'ó',
        "ograve" => 'ò',
        "ocirc" => 'ô',
        "ouml" => 'ö',
        "oelig" => 'œ',
        "uacute" => 'ú',
        "ugrave" => 'ù',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "Agrave" => 'À',
        "Ccedil" => 'Ç',
        "Eacute" => 'É',
        "Egrave" => 'È',
        _ => return None,
    })
}

fn read_attributes(
    element: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) -> Option<Vec<(String, String)>> {
    let mut attributes = Vec::new();
    for attr in element.attributes() {
        let attr = attr.ok()?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();

        #[allow(deprecated)]
        let value = attr
            .decode_and_unescape_value(decoder)
            .map(|v| v.into_owned())
            .unwrap_or_else(|_| String::from_utf8_lossy(attr.value.as_ref()).into_owned());
        attributes.push((key, value));
    }
    Some(attributes)
}

fn has_audio_extension(href: &str) -> bool {
    let lower = href.to_lowercase();
    [".mp3", ".m4a", ".wav", ".aac", ".ogg"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

fn parse_rfc822_date(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc2822(text)
        .map(|d| d.with_timezone(&Utc))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
  <channel>
    <title><![CDATA[Mystère à l'école]]></title>
    <image>
      <title>Image du flux</title>
      <url>https://example.com/channel-image.jpg</url>
    </image>
    <itunes:image href="https://example.com/itunes-image.jpg"/>
    <item>
      <title><![CDATA[Chapitre 1 : la rentrée]]></title>
      <guid isPermaLink="false">ep-guid-1</guid>
      <enclosure url="https://example.com/ep1.mp3" type="audio/mpeg" length="123"/>
      <itunes:image href="https://example.com/ep1.jpg"/>
      <pubDate>Wed, 02 Oct 2024 13:00:00 GMT</pubDate>
      <itunes:duration>12:34</itunes:duration>
    </item>
    <item>
      <title>Sans fichier audio</title>
      <guid>ep-guid-2</guid>
    </item>
    <item>
      <title>Enclosure par extension</title>
      <enclosure url="https://example.com/ep3.M4A" type="application/octet-stream"/>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn parses_feed_title_image_and_episodes() {
        let podcast = parse(SAMPLE_FEED.as_bytes(), "https://example.com/rss").unwrap();

        assert_eq!(podcast.title, "Mystère à l'école");

        assert_eq!(
            podcast.image_url.as_deref(),
            Some("https://example.com/itunes-image.jpg")
        );
        assert_eq!(
            podcast.episodes.len(),
            2,
            "l'item sans enclosure audio est ignoré"
        );

        let ep1 = &podcast.episodes[0];
        assert_eq!(ep1.guid, "ep-guid-1");
        assert_eq!(ep1.title, "Chapitre 1 : la rentrée");
        assert_eq!(ep1.audio_url, "https://example.com/ep1.mp3");
        assert_eq!(
            ep1.image_url.as_deref(),
            Some("https://example.com/ep1.jpg")
        );
        assert!(ep1.published_at.is_some());
        assert_eq!(ep1.duration.as_deref(), Some("12:34"));
    }

    #[test]
    fn enclosure_with_audio_extension_but_generic_type_is_accepted() {
        let podcast = parse(SAMPLE_FEED.as_bytes(), "https://example.com/rss").unwrap();
        assert_eq!(podcast.episodes[1].audio_url, "https://example.com/ep3.M4A");
    }

    #[test]
    fn missing_guid_falls_back_to_audio_url_and_missing_title_gets_default() {
        let feed = r#"<rss><channel><item>
            <enclosure url="https://example.com/a.mp3" type="audio/mpeg"/>
        </item></channel></rss>"#;
        let podcast = parse(feed.as_bytes(), "feed").unwrap();
        assert_eq!(podcast.episodes.len(), 1);
        assert_eq!(podcast.episodes[0].guid, "https://example.com/a.mp3");
        assert_eq!(podcast.episodes[0].title, "Episode sans titre");
    }

    #[test]
    fn episode_without_own_image_falls_back_to_feed_image() {
        let feed = r#"<rss xmlns:itunes="x"><channel>
            <itunes:image href="https://example.com/feed.jpg"/>
            <item><enclosure url="https://example.com/a.mp3" type="audio/mpeg"/></item>
        </channel></rss>"#;
        let podcast = parse(feed.as_bytes(), "feed").unwrap();
        assert_eq!(
            podcast.episodes[0].image_url.as_deref(),
            Some("https://example.com/feed.jpg")
        );
    }

    #[test]
    fn channel_image_url_tag_used_when_no_itunes_image() {
        let feed = r#"<rss><channel>
            <image><url>https://example.com/channel.jpg</url></image>
            <item><enclosure url="https://example.com/a.mp3" type="audio/mpeg"/></item>
        </channel></rss>"#;
        let podcast = parse(feed.as_bytes(), "feed").unwrap();
        assert_eq!(
            podcast.image_url.as_deref(),
            Some("https://example.com/channel.jpg")
        );
    }

    #[test]
    fn image_title_does_not_overwrite_feed_title() {
        let podcast = parse(SAMPLE_FEED.as_bytes(), "https://example.com/rss").unwrap();
        assert_eq!(
            podcast.title, "Mystère à l'école",
            "le <title> de <image> ne doit pas écraser celui du canal"
        );
    }

    #[test]
    fn empty_feed_title_falls_back_to_feed_url() {
        let feed = r#"<rss><channel></channel></rss>"#;
        let podcast = parse(feed.as_bytes(), "https://example.com/rss").unwrap();
        assert_eq!(podcast.title, "https://example.com/rss");
    }

    #[test]
    fn malformed_xml_returns_none() {
        assert!(parse(b"<rss><channel><item></rss>", "feed").is_none());
    }

    #[test]
    fn entities_in_text_are_resolved_like_xmlparser() {
        let feed = r#"<rss><channel><item>
            <guid>tom &amp; jerry &#233;pisode &lt;1&gt;</guid>
            <title>A &quot;B&quot; &apos;C&apos;</title>
            <enclosure url="https://example.com/a.mp3" type="audio/mpeg"/>
        </item></channel></rss>"#;
        let podcast = parse(feed.as_bytes(), "feed").unwrap();
        assert_eq!(podcast.episodes[0].guid, "tom & jerry épisode <1>");
        assert_eq!(podcast.episodes[0].title, "A \"B\" 'C'");
    }

    #[test]
    fn named_entities_are_resolved_and_unknown_are_tolerated() {
        let feed = r#"<rss><channel>
            <title>Les&nbsp;Odyss&eacute;es&hellip;</title>
            <item>
                <guid>a&nbsp;b &frobnicate; c</guid>
                <enclosure url="https://example.com/a.mp3" type="audio/mpeg"/>
            </item>
        </channel></rss>"#;
        let podcast = parse(feed.as_bytes(), "feed").unwrap();
        assert_eq!(podcast.title, "Les\u{00a0}Odyssées…");
        assert_eq!(podcast.episodes[0].guid, "a\u{00a0}b  c");

        let in_attribute = r#"<rss><channel><item>
            <enclosure url="https://example.com/a&nbsp;b.mp3" type="audio/mpeg"/>
        </item></channel></rss>"#;
        assert!(parse(in_attribute.as_bytes(), "feed").is_some());
    }

    #[test]
    fn truncated_feed_is_rejected() {
        assert!(parse(b"<rss><channel><item>", "feed").is_none());
    }

    #[test]
    fn rfc822_date_without_weekday_is_parsed() {
        assert!(parse_rfc822_date("02 Oct 2024 13:00:00 GMT").is_some());
        assert!(parse_rfc822_date("Wed, 02 Oct 2024 13:00:00 +0200").is_some());
        assert!(parse_rfc822_date("pas une date").is_none());
    }
}
