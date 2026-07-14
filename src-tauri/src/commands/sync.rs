use std::collections::HashSet;

use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

use merlin_application::sync_episodes_use_case::{
    PrepareCallbacks, RunCallbacks, SyncEpisodesUseCase, SyncProgressPhase,
};
use merlin_domain::library::subscription::Subscription;
use merlin_domain::podcasts::episode::Episode;
use merlin_infra::persistence::db;
use merlin_infra::persistence::sync_state_repository::SqliteSyncStateRepository;
use merlin_infra::podcasts::audio_converter;

use crate::dto::{SelectedPair, SyncLaunchDto};
use crate::events::{self, PrepareFailedPayload, PrepareProgressPayload, StepPayload};
use crate::state::AppState;

#[tauri::command]
pub async fn start_sync(
    launch: SyncLaunchDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state
        .sync_running
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return Err("une synchronisation est déjà en cours".to_string());
    }
    let token = CancellationToken::new();
    *state.sync_cancel.lock().expect("lock") = Some(token.clone());
    let op_lock = state.op_lock.clone();
    let sync_cancel = state.sync_cancel.clone();
    let sync_running = state.sync_running.clone();
    let db_path = state.db_path.clone();
    let work_dir = state.work_dir.clone();

    let device_id = match state.require_device_id() {
        Ok(id) => id,
        Err(message) => {
            state
                .sync_running
                .store(false, std::sync::atomic::Ordering::SeqCst);
            *state.sync_cancel.lock().expect("lock") = None;
            return Err(message);
        }
    };

    tauri::async_runtime::spawn(async move {
        let _guard = op_lock.lock_owned().await;
        let blocking_app = app.clone();
        let result = tokio::task::spawn_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime de synchro");
            runtime.block_on(run_sync(
                blocking_app,
                launch,
                db_path,
                work_dir,
                device_id,
                token,
            ));
        })
        .await;
        if result.is_err() {
            let _ = app.emit(
                events::SYNC_PHASE,
                SyncProgressPhase::Failed(
                    "la tache de synchronisation s'est interrompue de facon inattendue".into(),
                ),
            );
        }

        *sync_cancel.lock().expect("lock") = None;
        sync_running.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = app.emit(events::SYNC_ENDED, ());
    });
    Ok(())
}

async fn run_sync(
    app: AppHandle,
    launch: SyncLaunchDto,
    db_path: std::path::PathBuf,
    work_dir: std::path::PathBuf,
    device_id: String,
    token: CancellationToken,
) {
    let connection = match db::open(&db_path) {
        Ok(connection) => connection,
        Err(error) => {
            let _ = app.emit(
                events::SYNC_PHASE,
                SyncProgressPhase::Failed(format!("base locale inaccessible : {error}")),
            );
            return;
        }
    };
    let repository = SqliteSyncStateRepository::new(connection, device_id);
    let mut use_case = SyncEpisodesUseCase::new(repository, work_dir);
    use_case.use_cancellation_token(token);

    let pairs: Vec<(Subscription, Episode)> = launch
        .pairs
        .into_iter()
        .map(|p| (p.subscription, p.episode))
        .collect();
    let already: HashSet<String> = launch.already_synced.into_iter().collect();

    let on_phase = |phase: SyncProgressPhase| {
        let _ = app.emit(events::SYNC_PHASE, phase);
    };
    let on_log = |message: &str| {
        let _ = app.emit(events::SYNC_LOG, message);
    };
    let on_current_step = |label: Option<&str>, fraction: f64| {
        let _ = app.emit(
            events::SYNC_STEP,
            StepPayload {
                label: label.map(String::from),
                fraction,
            },
        );
    };
    let on_episode_uploaded = |uuid: &str| {
        let _ = app.emit(events::SYNC_EPISODE_UPLOADED, uuid);
    };
    let deletions_completed = |uuids: &[String]| {
        let _ = app.emit(events::SYNC_DELETIONS_COMPLETED, uuids);
    };
    let tree_edits_applied = || {
        let _ = app.emit(events::SYNC_TREE_EDITS_APPLIED, ());
    };
    let callbacks = RunCallbacks {
        on_phase: &on_phase,
        on_log: &on_log,
        on_current_step: &on_current_step,
        on_episode_uploaded: &on_episode_uploaded,
        deletions_completed: &deletions_completed,
        tree_edits_applied: &tree_edits_applied,
    };
    use_case
        .run(
            &pairs,
            &launch.host,
            launch.port,
            &already,
            &launch.files_to_delete,
            launch.tree_edits,
            &callbacks,
        )
        .await;
}

#[tauri::command]
pub fn cancel_sync(state: State<AppState>) {
    if let Some(token) = state.sync_cancel.lock().expect("lock").as_ref() {
        token.cancel();
    }
}

#[tauri::command]
pub fn prepared_guids(guids: Vec<String>, state: State<AppState>) -> Vec<String> {
    let converted = state.work_dir.join("converted");
    guids
        .into_iter()
        .filter(|guid| {
            let uuid = audio_converter::episode_uuid(guid);
            converted.join(format!("{uuid}.mp3")).exists()
        })
        .collect()
}

#[tauri::command]
pub async fn prepare_selection(
    pairs: Vec<SelectedPair>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state
        .prepare_running
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return Err("une preparation est deja en cours".to_string());
    }

    let op_lock = state.op_lock.clone();
    let db_path = state.db_path.clone();
    let work_dir = state.work_dir.clone();
    let device_id = state.read_device_id();
    let prepare_running = state.prepare_running.clone();

    tauri::async_runtime::spawn(async move {
        let _guard = op_lock.lock_owned().await;
        let task_app = app.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime de preparation");
            runtime.block_on(run_prepare(task_app, pairs, db_path, work_dir, device_id));
        })
        .await;

        prepare_running.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = app.emit(events::PREPARE_ENDED, ());
    });
    Ok(())
}

async fn run_prepare(
    app: AppHandle,
    pairs: Vec<SelectedPair>,
    db_path: std::path::PathBuf,
    work_dir: std::path::PathBuf,
    device_id: String,
) {
    let connection = match db::open(&db_path) {
        Ok(connection) => connection,
        Err(error) => {
            for pair in &pairs {
                let _ = app.emit(
                    events::PREPARE_EPISODE_FAILED,
                    PrepareFailedPayload {
                        guid: pair.episode.guid.clone(),
                        error: format!("base locale inaccessible : {error}"),
                    },
                );
            }
            return;
        }
    };
    let repository = SqliteSyncStateRepository::new(connection, device_id);
    let use_case = SyncEpisodesUseCase::new(repository, work_dir);
    let pairs: Vec<(Subscription, Episode)> = pairs
        .into_iter()
        .map(|p| (p.subscription, p.episode))
        .collect();

    let on_log = |message: &str| {
        let _ = app.emit(events::SYNC_LOG, message);
    };
    let on_progress = |_done: usize, _total: usize| {};
    let on_episode_ready = |guid: &str| {
        let _ = app.emit(events::PREPARE_EPISODE_READY, guid);
    };
    let on_episode_failed = |guid: &str, error: &str| {
        let _ = app.emit(
            events::PREPARE_EPISODE_FAILED,
            PrepareFailedPayload {
                guid: guid.to_string(),
                error: error.to_string(),
            },
        );
    };
    let on_episode_progress = |guid: &str, fraction: f64| {
        let _ = app.emit(
            events::PREPARE_PROGRESS,
            PrepareProgressPayload {
                guid: guid.to_string(),
                fraction,
            },
        );
    };
    let callbacks = PrepareCallbacks {
        on_log: &on_log,
        on_progress: &on_progress,
        on_episode_ready: &on_episode_ready,
        on_episode_failed: &on_episode_failed,
        on_episode_progress: &on_episode_progress,
    };
    use_case.prepare(&pairs, &callbacks).await;
}
