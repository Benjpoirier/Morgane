use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

const RELEASE_BASE: &str = "https://github.com/eugeneware/ffmpeg-static/releases/download";
const TAG: &str = "b6.1.1";

const ASSETS: &[(&str, &str, &str)] = &[
    (
        "darwin-arm64",
        "ffmpeg-darwin-arm64.gz",
        "8923876afa8db5585022d7860ec7e589af192f441c56793971276d450ed3bbfa",
    ),
    (
        "darwin-x64",
        "ffmpeg-darwin-x64.gz",
        "929b375c1182d956c51f7ac25e0b2b0411fb01f6f407aa15c9758efeb4242106",
    ),
    (
        "linux-x64",
        "ffmpeg-linux-x64.gz",
        "bfe8a8fc511530457b528c48d77b5737527b504a3797a9bc4866aeca69c2dffa",
    ),
    (
        "win32-x64",
        "ffmpeg-win32-x64.gz",
        "8883a3dffbd0a16cf4ef95206ea05283f78908dbfb118f73c83f4951dcc06d77",
    ),
];

static DOWNLOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Copy)]
pub enum SetupPhase {
    Downloading,
    Verifying,
    Extracting,
    Done,
}

#[derive(Debug, Clone, Copy)]
pub struct SetupProgress {
    pub phase: SetupPhase,
    pub bytes: u64,
    pub total: u64,
}

#[derive(Debug, Error)]
pub enum FfmpegSetupError {
    #[error("aucune build ffmpeg disponible pour cette plateforme")]
    UnsupportedPlatform,
    #[error("échec réseau : {0}")]
    Network(String),
    #[error("intégrité invalide (SHA-256) du binaire téléchargé")]
    Checksum,
    #[error("erreur disque : {0}")]
    Io(String),
}

fn io_err<E: std::fmt::Display>(e: E) -> FfmpegSetupError {
    FfmpegSetupError::Io(e.to_string())
}

fn net_err<E: std::fmt::Display>(e: E) -> FfmpegSetupError {
    FfmpegSetupError::Network(e.to_string())
}

pub fn bin_name() -> &'static str {
    if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

pub fn managed_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("merlinSync")
        .join("bin")
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub fn resolve() -> Option<PathBuf> {
    let managed = managed_dir().join(bin_name());
    if is_executable(&managed) {
        return Some(managed);
    }
    system_ffmpeg()
}

fn system_ffmpeg() -> Option<PathBuf> {
    let name = bin_name();
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let candidate = PathBuf::from(dir).join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn platform_key() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("darwin-arm64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("darwin-x64")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-x64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("win32-x64")
    } else {
        None
    }
}

fn current_asset() -> Option<(&'static str, &'static str)> {
    let key = platform_key()?;
    ASSETS
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, file, sha)| (*file, *sha))
}

pub async fn ensure<F>(progress: F) -> Result<PathBuf, FfmpegSetupError>
where
    F: Fn(SetupProgress) + Send,
{
    if let Some(path) = resolve() {
        return Ok(path);
    }

    let _guard = DOWNLOAD_LOCK.lock().await;
    if let Some(path) = resolve() {
        return Ok(path);
    }

    let (filename, expected) = current_asset().ok_or(FfmpegSetupError::UnsupportedPlatform)?;
    let dir = managed_dir();
    std::fs::create_dir_all(&dir).map_err(io_err)?;

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("part") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    let pid = std::process::id();
    let gz_path = dir.join(format!("ffmpeg.{pid}.gz.part"));
    let bin_tmp = dir.join(format!("ffmpeg.{pid}.part"));

    let outcome =
        download_and_install(filename, expected, &dir, &gz_path, &bin_tmp, &progress).await;

    let _ = std::fs::remove_file(&gz_path);
    if outcome.is_err() {
        let _ = std::fs::remove_file(&bin_tmp);
    }
    outcome
}

async fn download_and_install<F>(
    filename: &str,
    expected: &str,
    dir: &Path,
    gz_path: &Path,
    bin_tmp: &Path,
    progress: &F,
) -> Result<PathBuf, FfmpegSetupError>
where
    F: Fn(SetupProgress) + Send,
{
    let url = format!("{RELEASE_BASE}/{TAG}/{filename}");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(20 * 60))
        .user_agent(concat!("Morgane/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(net_err)?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(net_err)?
        .error_for_status()
        .map_err(net_err)?;

    let total = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(gz_path).await.map_err(io_err)?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(net_err)?;
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(io_err)?;
        downloaded += chunk.len() as u64;
        if downloaded - last_emit >= 256 * 1024 {
            last_emit = downloaded;
            progress(SetupProgress {
                phase: SetupPhase::Downloading,
                bytes: downloaded,
                total,
            });
        }
    }
    file.flush().await.map_err(io_err)?;
    progress(SetupProgress {
        phase: SetupPhase::Downloading,
        bytes: downloaded,
        total,
    });
    drop(file);

    progress(SetupProgress {
        phase: SetupPhase::Verifying,
        bytes: downloaded,
        total,
    });
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if !hex.eq_ignore_ascii_case(expected) {
        return Err(FfmpegSetupError::Checksum);
    }

    progress(SetupProgress {
        phase: SetupPhase::Extracting,
        bytes: downloaded,
        total,
    });
    let gz = gz_path.to_path_buf();
    let bin = bin_tmp.to_path_buf();
    tokio::task::spawn_blocking(move || gunzip(&gz, &bin))
        .await
        .map_err(io_err)?
        .map_err(io_err)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin_tmp, std::fs::Permissions::from_mode(0o755))
            .map_err(io_err)?;
    }

    let final_path = dir.join(bin_name());
    std::fs::rename(bin_tmp, &final_path).map_err(io_err)?;
    progress(SetupProgress {
        phase: SetupPhase::Done,
        bytes: downloaded,
        total,
    });
    Ok(final_path)
}

fn gunzip(src: &Path, dst: &Path) -> std::io::Result<()> {
    let input = std::fs::File::open(src)?;
    let mut decoder = flate2::read::GzDecoder::new(input);
    let mut output = std::fs::File::create(dst)?;
    std::io::copy(&mut decoder, &mut output)?;
    Ok(())
}
