use tauri::{AppHandle, Emitter};

use merlin_infra::podcasts::ffmpeg_provider::{self, SetupPhase, SetupProgress};

use crate::events::{self, FfmpegProgressPayload};

#[tauri::command]
pub fn ffmpeg_ready() -> bool {
    ffmpeg_provider::resolve().is_some()
}

#[tauri::command]
pub async fn download_ffmpeg(app: AppHandle) -> Result<(), String> {
    let emitter = app.clone();
    ffmpeg_provider::ensure(move |p: SetupProgress| {
        let phase = match p.phase {
            SetupPhase::Downloading => "downloading",
            SetupPhase::Verifying => "verifying",
            SetupPhase::Extracting => "extracting",
            SetupPhase::Done => "done",
        };
        let _ = emitter.emit(
            events::FFMPEG_PROGRESS,
            FfmpegProgressPayload {
                phase: phase.to_string(),
                bytes: p.bytes,
                total_bytes: p.total,
            },
        );
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}
