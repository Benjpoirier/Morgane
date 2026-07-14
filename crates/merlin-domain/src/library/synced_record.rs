use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedRecord {
    pub episode_uuid: String,
    pub title: String,
    pub folder_title: String,
    pub synced_at: DateTime<Utc>,
    pub pending_deletion: bool,
}
