use std::time::Duration;

use tauri::{AppHandle, Emitter, State};

use merlin_domain::sync::integrity_checker::{self, Issue, MissingFile};
use merlin_infra::podcasts::{audio_converter, image_converter};
use merlin_infra::sync::device_tree_fetcher;
use merlin_infra::sync::engine::{RepairFile, SyncCallbacks, SyncEngine};

use crate::dto::RepairFix;
use crate::events::{self, IntegrityProgressPayload};
use crate::state::AppState;

#[tauri::command]
pub async fn check_integrity(
    app: AppHandle,
    host: String,
    port: u16,
    state: State<'_, AppState>,
) -> Result<Vec<Issue>, String> {
    let _guard = state.op_lock.clone().lock_owned().await;
    let on_progress = |done: usize, total: usize| {
        let _ = app.emit(
            events::INTEGRITY_PROGRESS,
            IntegrityProgressPayload { done, total },
        );
    };
    let (tree, existing) = device_tree_fetcher::fetch_tree_and_check_integrity(
        &host,
        port,
        Duration::from_secs(10),
        Some(&on_progress),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(integrity_checker::check(&tree, &existing))
}

#[tauri::command]
pub async fn repair_integrity(
    host: String,
    port: u16,
    fixes: Vec<RepairFix>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _guard = state.op_lock.clone().lock_owned().await;
    let work_dir = std::env::temp_dir().join("merlinsync_integrity_repair");
    let mut repair_files = Vec::new();
    for fix in &fixes {
        let local_path = std::path::Path::new(&fix.path);
        match &fix.missing {
            MissingFile::Image { remote_name } => {
                let base = remote_name.trim_end_matches(".jpg");
                let converted =
                    image_converter::convert_local_image_to_jpeg(local_path, base, &work_dir)
                        .map_err(|e| e.to_string())?;
                repair_files.push(RepairFile {
                    local_path: converted,
                    remote_name: remote_name.clone(),
                });
            }
            MissingFile::Audio { base_uuid } => {
                let converted = audio_converter::convert_local_audio_to_mp3(
                    local_path,
                    base_uuid,
                    &work_dir,
                    &|_| {},
                )
                .await
                .map_err(|e| e.to_string())?;
                repair_files.push(RepairFile {
                    local_path: converted,
                    remote_name: format!("{base_uuid}.mp3"),
                });
            }
        }
    }
    let mut engine = SyncEngine::new(host, port);
    engine
        .repair_files(&repair_files, &SyncCallbacks::silent())
        .await
        .map_err(|e| e.to_string())
}
