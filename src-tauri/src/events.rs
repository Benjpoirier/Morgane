use serde::Serialize;

pub const SYNC_PHASE: &str = "sync://phase";
pub const SYNC_LOG: &str = "sync://log";
pub const SYNC_STEP: &str = "sync://step";
pub const SYNC_EPISODE_UPLOADED: &str = "sync://episode-uploaded";
pub const SYNC_DELETIONS_COMPLETED: &str = "sync://deletions-completed";
pub const SYNC_TREE_EDITS_APPLIED: &str = "sync://tree-edits-applied";
pub const SYNC_ENDED: &str = "sync://ended";

pub const PREPARE_EPISODE_READY: &str = "prepare://episode-ready";
pub const PREPARE_EPISODE_FAILED: &str = "prepare://episode-failed";
pub const PREPARE_PROGRESS: &str = "prepare://progress";
pub const PREPARE_ENDED: &str = "prepare://ended";
pub const THUMBNAIL_READY: &str = "thumbnail://ready";
pub const INTEGRITY_PROGRESS: &str = "integrity://progress";
pub const FFMPEG_PROGRESS: &str = "ffmpeg://progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityProgressPayload {
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepPayload {
    pub label: Option<String>,
    pub fraction: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareFailedPayload {
    pub guid: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareProgressPayload {
    pub guid: String,
    pub fraction: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailReadyPayload {
    pub uuid: String,

    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegProgressPayload {
    pub phase: String,
    pub bytes: u64,
    pub total_bytes: u64,
}
