use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use merlin_domain::library::category_assignment::PodcastCategoryAssignment;
use merlin_domain::library::manual_category::ManualCategory;
use merlin_domain::library::subscription::Subscription;
use merlin_domain::library::synced_record::SyncedRecord;
use merlin_domain::playlist::model::PlaylistFolder;
use merlin_domain::playlist::tree_edit::TreeEdit;
use merlin_domain::podcasts::episode::Episode;
use merlin_domain::sync::integrity_checker::MissingFile;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub connected: bool,
    pub latency_ms: Option<f64>,
    pub message: Option<String>,

    pub busy: bool,

    pub device_mac: Option<String>,

    pub device_name: Option<String>,

    pub newly_registered: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedPair {
    pub subscription: Subscription,
    pub episode: Episode,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeView {
    pub folders: Vec<PlaylistFolder>,
    pub pending_edits: Vec<TreeEdit>,

    pub edit_details: Vec<EditDetail>,
    pub pending_orphan_deletions: Vec<String>,

    pub thumbnail_uuids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditDetail {
    pub uuid: String,
    pub kind: String,
    pub title: String,
    pub new_title: Option<String>,
    pub dest_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairFix {
    pub missing: MissingFile,

    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncLaunchDto {
    pub pairs: Vec<SelectedPair>,
    pub host: String,
    pub port: u16,
    pub already_synced: Vec<String>,

    pub files_to_delete: std::collections::HashMap<String, Vec<String>>,
    pub tree_edits: Vec<TreeEdit>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingGroup {
    pub feed_url: String,
    pub group_key: String,
    pub uuid: String,
    pub title: String,
    pub feed_image_url: Option<String>,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStateSnapshot {
    pub synced_records: Vec<SyncedRecord>,
    pub episode_title_overrides: HashMap<String, String>,
    pub episode_number_overrides: HashMap<String, i64>,
    pub group_title_overrides: HashMap<String, String>,
    pub category_assignments: HashMap<String, PodcastCategoryAssignment>,
    pub folder_image_overrides: HashMap<String, String>,
    pub episode_image_overrides: HashMap<String, String>,
    pub manual_categories: Vec<ManualCategory>,
}
