use tauri::State;

use merlin_domain::library::category_assignment::PodcastCategoryAssignment;
use merlin_domain::library::repositories::SyncStateRepository;
use merlin_infra::persistence::db;
use merlin_infra::persistence::sync_state_repository::SqliteSyncStateRepository;
use merlin_infra::podcasts::audio_converter;

use crate::dto::SyncStateSnapshot;
use crate::state::AppState;

fn repo(state: &AppState) -> Result<SqliteSyncStateRepository, String> {
    db::open(&state.db_path)
        .map(|connection| SqliteSyncStateRepository::new(connection, state.read_device_id()))
        .map_err(|e| format!("base locale inaccessible : {e}"))
}

fn invalidate_image_cache(state: &AppState, uuid: &str) {
    let path = state.work_dir.join("converted").join(format!("{uuid}.jpg"));
    let _ = std::fs::remove_file(path);
}

#[tauri::command]
pub fn get_sync_state(state: State<AppState>) -> Result<SyncStateSnapshot, String> {
    let repo = repo(&state)?;
    Ok(SyncStateSnapshot {
        synced_records: repo.synced_records(),
        episode_title_overrides: repo.episode_title_overrides(),
        episode_number_overrides: repo.episode_number_overrides(),
        group_title_overrides: repo.group_title_overrides(),
        category_assignments: repo.category_assignments(),
        folder_image_overrides: repo.folder_image_overrides(),
        episode_image_overrides: repo.episode_image_overrides(),
        manual_categories: repo.manual_categories(),
    })
}

#[tauri::command]
pub fn mark_pending_deletion(
    episode_uuid: String,
    pending: bool,
    state: State<AppState>,
) -> Result<(), String> {
    repo(&state)?.mark_pending_deletion(&episode_uuid, pending);
    Ok(())
}

#[tauri::command]
pub fn set_episode_title_override(
    guid: String,
    title: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    let title = title.filter(|t| !t.is_empty());
    repo(&state)?.set_episode_title_override(&guid, title.as_deref());
    Ok(())
}

#[tauri::command]
pub fn set_episode_number_override(
    guid: String,
    number: Option<i64>,
    state: State<AppState>,
) -> Result<(), String> {
    repo(&state)?.set_episode_number_override(&guid, number);
    Ok(())
}

#[tauri::command]
pub fn set_episode_image_override(
    guid: String,
    source: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    repo(&state)?.set_episode_image_override(&guid, source.as_deref());
    invalidate_image_cache(&state, &audio_converter::episode_uuid(&guid));
    Ok(())
}

#[tauri::command]
pub fn set_group_title_override(
    feed_url: String,
    group_key: String,
    title: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    let title = title.filter(|t| !t.is_empty());
    repo(&state)?.set_group_title_override(&feed_url, &group_key, title.as_deref());
    Ok(())
}

#[tauri::command]
pub fn set_category_assignment(
    assignment: PodcastCategoryAssignment,
    state: State<AppState>,
) -> Result<(), String> {
    repo(&state)?.set_category_assignment(assignment);
    Ok(())
}

#[tauri::command]
pub fn remove_category_assignment(
    feed_url: String,
    group_key: String,
    state: State<AppState>,
) -> Result<(), String> {
    repo(&state)?.remove_category_assignment(&feed_url, &group_key);
    Ok(())
}

#[tauri::command]
pub fn set_folder_image_override(
    folder_uuid: String,
    source: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    repo(&state)?.set_folder_image_override(&folder_uuid, source.as_deref());
    invalidate_image_cache(&state, &folder_uuid);
    Ok(())
}
