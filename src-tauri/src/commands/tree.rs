use std::time::Duration;

use tauri::{AppHandle, Emitter, State};

use merlin_domain::library::deterministic_uuid;
use merlin_domain::library::manual_category::ManualCategory;
use merlin_domain::library::repositories::SyncStateRepository;
use merlin_domain::sync::orphan_finder;
use merlin_infra::persistence::db;
use merlin_infra::persistence::sync_state_repository::SqliteSyncStateRepository;
use merlin_infra::podcasts::image_converter;
use merlin_infra::sync::device_tree_fetcher;

use crate::dto::TreeView;
use crate::events::{self, ThumbnailReadyPayload};
use crate::state::AppState;
use crate::tree_session::EditKind;

fn thumbnail_cache_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("merlinsync_category_thumbnails")
}

fn is_valid_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

fn view(state: &AppState) -> TreeView {
    state.tree.lock().expect("lock").view()
}

#[tauri::command]
pub async fn refresh_tree(
    host: String,
    port: u16,
    state: State<'_, AppState>,
) -> Result<TreeView, String> {
    let _guard = state.op_lock.clone().lock_owned().await;
    let live = device_tree_fetcher::fetch(&host, port, Duration::from_secs(10))
        .await
        .map_err(|e| e.to_string())?;
    let manual = SqliteSyncStateRepository::new(
        db::open(&state.db_path).map_err(|e| e.to_string())?,
        state.read_device_id(),
    )
    .manual_categories();
    let mut tree = state.tree.lock().expect("lock");
    tree.apply_refreshed(live, &manual);
    Ok(tree.view())
}

#[tauri::command]
pub fn rename_folder(uuid: String, new_title: String, state: State<AppState>) -> TreeView {
    state
        .tree
        .lock()
        .expect("lock")
        .rename_folder(&uuid, &new_title);
    view(&state)
}

#[tauri::command]
pub fn rename_sound(uuid: String, new_title: String, state: State<AppState>) -> TreeView {
    state
        .tree
        .lock()
        .expect("lock")
        .rename_sound(&uuid, &new_title);
    view(&state)
}

#[tauri::command]
pub fn rename_pending_group_preview(
    uuid: String,
    new_title: String,
    state: State<AppState>,
) -> TreeView {
    state
        .tree
        .lock()
        .expect("lock")
        .rename_pending_group_preview(&uuid, &new_title);
    view(&state)
}

#[tauri::command]
pub fn move_node(uuid: String, destination_uuid: String, state: State<AppState>) -> TreeView {
    state
        .tree
        .lock()
        .expect("lock")
        .move_node(&uuid, &destination_uuid);
    view(&state)
}

#[tauri::command]
pub fn delete_folder(uuid: String, state: State<AppState>) -> Result<TreeView, String> {
    let sound_uuids = state.tree.lock().expect("lock").delete_node(&uuid);
    if !sound_uuids.is_empty() {
        let mut repo = SqliteSyncStateRepository::new(
            db::open(&state.db_path).map_err(|e| e.to_string())?,
            state.read_device_id(),
        );
        for sound in &sound_uuids {
            repo.mark_pending_deletion(sound, true);
        }
    }
    Ok(view(&state))
}

#[tauri::command]
pub fn cancel_tree_edit(
    uuid: String,
    kind: String,
    state: State<AppState>,
) -> Result<TreeView, String> {
    let Some(kind) = EditKind::from_tag(&kind) else {
        return Err(format!("nature d'édition inconnue : {kind}"));
    };

    let mut repo = if kind == EditKind::Removed {
        Some(SqliteSyncStateRepository::new(
            db::open(&state.db_path).map_err(|e| e.to_string())?,
            state.read_device_id(),
        ))
    } else {
        None
    };
    let sounds = state.tree.lock().expect("lock").cancel_edit(&uuid, kind);
    if let Some(repo) = repo.as_mut() {
        for sound in &sounds {
            repo.mark_pending_deletion(sound, false);
        }
    }
    Ok(view(&state))
}

#[tauri::command]
pub fn add_manual_category(
    title: String,
    image_source: String,
    state: State<AppState>,
) -> Result<TreeView, String> {
    let uuid = deterministic_uuid::from_prefixed_name(&format!("merlinsync-category:{title}"));
    SqliteSyncStateRepository::new(
        db::open(&state.db_path).map_err(|e| e.to_string())?,
        state.read_device_id(),
    )
    .add_manual_category(ManualCategory {
        uuid: uuid.clone(),
        title: title.clone(),
        image_source,
    });
    state
        .tree
        .lock()
        .expect("lock")
        .add_manual_category(&uuid, &title);
    Ok(view(&state))
}

#[tauri::command]
pub fn remove_manual_category(uuid: String, state: State<AppState>) -> Result<TreeView, String> {
    let mut repo = SqliteSyncStateRepository::new(
        db::open(&state.db_path).map_err(|e| e.to_string())?,
        state.read_device_id(),
    );
    let orphaned: Vec<(String, String)> = repo
        .category_assignments()
        .values()
        .filter(|a| a.target_category_uuid == uuid)
        .map(|a| (a.feed_url.clone(), a.group_key.clone()))
        .collect();
    for (feed_url, group_key) in orphaned {
        repo.remove_category_assignment(&feed_url, &group_key);
    }
    repo.remove_manual_category(&uuid);
    state
        .tree
        .lock()
        .expect("lock")
        .remove_manual_category(&uuid);
    Ok(view(&state))
}

#[tauri::command]
pub fn toggle_orphan(uuid: String, state: State<AppState>) -> TreeView {
    state.tree.lock().expect("lock").toggle_orphan(&uuid);
    view(&state)
}

#[tauri::command]
pub fn toggle_all_orphans(state: State<AppState>) -> TreeView {
    state.tree.lock().expect("lock").toggle_all_orphans();
    view(&state)
}

#[tauri::command]
pub fn clear_pending_edits(state: State<AppState>) -> TreeView {
    state.tree.lock().expect("lock").clear_pending_edits();
    view(&state)
}

#[tauri::command]
pub fn clear_pending_orphan_deletions(state: State<AppState>) -> TreeView {
    state
        .tree
        .lock()
        .expect("lock")
        .clear_pending_orphan_deletions();
    view(&state)
}

#[tauri::command]
pub async fn search_orphans(
    host: String,
    port: u16,
    state: State<'_, AppState>,
) -> Result<TreeView, String> {
    let _guard = state.op_lock.clone().lock_owned().await;

    let tree_snapshot = state.tree.lock().expect("lock").folders.clone();
    let files = device_tree_fetcher::list_files(&host, port, Duration::from_secs(10))
        .await
        .map_err(|e| e.to_string())?;
    let orphans = orphan_finder::find_orphan_files(&files, &tree_snapshot);
    let mut tree = state.tree.lock().expect("lock");
    tree.apply_orphans(orphans);
    Ok(tree.view())
}

#[tauri::command]
pub async fn download_thumbnails(
    host: String,
    port: u16,
    uuids: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let cache_dir = thumbnail_cache_dir();
    let _ = std::fs::create_dir_all(&cache_dir);

    let mut to_download = Vec::new();
    for uuid in uuids {
        if !image_converter::is_valid_uuid(&uuid) {
            continue;
        }
        let path = cache_dir.join(format!("{uuid}.jpg"));
        if std::fs::read(&path)
            .map(|b| is_valid_jpeg(&b))
            .unwrap_or(false)
        {
            let _ = app.emit(
                events::THUMBNAIL_READY,
                ThumbnailReadyPayload {
                    uuid,
                    path: path.to_string_lossy().into_owned(),
                },
            );
        } else {
            to_download.push(uuid);
        }
    }
    if to_download.is_empty() {
        return Ok(());
    }

    let _guard = state.op_lock.clone().lock_owned().await;
    let downloaded = device_tree_fetcher::download_folder_images(
        &host,
        port,
        &to_download,
        Duration::from_secs(10),
    )
    .await;
    for (uuid, data) in downloaded {
        if !image_converter::is_valid_uuid(&uuid) {
            continue;
        }
        let path = cache_dir.join(format!("{uuid}.jpg"));
        if std::fs::write(&path, &data).is_ok() {
            let _ = app.emit(
                events::THUMBNAIL_READY,
                ThumbnailReadyPayload {
                    uuid,
                    path: path.to_string_lossy().into_owned(),
                },
            );
        }
    }
    Ok(())
}
