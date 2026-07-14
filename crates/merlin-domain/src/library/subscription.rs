use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub feed_url: String,
    pub title: String,
    pub kind: String,
    pub category: String,
    pub selected_episode_guids: Vec<String>,
    pub feed_image_url: Option<String>,
    pub direct_audio_url: Option<String>,
    pub direct_title: Option<String>,
    pub direct_image_url: Option<String>,
}

impl Subscription {
    pub fn new(feed_url: impl Into<String>) -> Self {
        Self {
            feed_url: feed_url.into(),
            title: String::new(),
            kind: "rss".to_string(),
            category: String::new(),
            selected_episode_guids: Vec::new(),
            feed_image_url: None,
            direct_audio_url: None,
            direct_title: None,
            direct_image_url: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.feed_url
    }

    pub fn new_direct_id() -> String {
        format!("direct:{}", Uuid::new_v4().to_string().to_uppercase())
    }
}
