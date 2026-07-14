#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastCategoryAssignment {
    pub feed_url: String,
    pub group_key: String,
    pub target_category_uuid: String,
    pub target_category_title: String,
}
