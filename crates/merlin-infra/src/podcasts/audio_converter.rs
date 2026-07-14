use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tracing::{debug, info};

use merlin_domain::library::deterministic_uuid;

use super::image_converter::{
    ConversionError, DOWNLOAD_USER_AGENT, convert_image_to_jpeg, fetch_image_bytes, is_valid_uuid,
};

pub const TARGET_SAMPLE_RATE: u32 = 44_100;
pub const TARGET_CHANNELS: u32 = 2;
pub const TARGET_BITRATE: &str = "128k";

const DOWNLOAD_TOTAL_TIMEOUT: Duration = Duration::from_secs(10 * 60);

const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(45);

const FFMPEG_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionStep {
    DownloadingAudio,
    ConvertingAudio,
    DownloadingImage,
    ConvertingImage,
}

pub type ProgressFn<'a> = &'a (dyn Fn(ConversionStep, f64) + Send + Sync);

pub struct PreparedEpisode {
    pub audio: PathBuf,
    pub image: Option<PathBuf>,
    pub uuid: String,
}

pub fn episode_uuid(guid: &str) -> String {
    deterministic_uuid::from_guid_namespace(guid)
}

pub fn output_paths(guid: &str, work_dir: &Path) -> (PathBuf, PathBuf) {
    let id = episode_uuid(guid);
    let converted = work_dir.join("converted");
    (
        converted.join(format!("{id}.mp3")),
        converted.join(format!("{id}.jpg")),
    )
}

pub async fn download_and_convert(
    audio_url: &str,
    image_url: Option<&str>,
    guid: &str,
    work_dir: &Path,
    episode_number: Option<i64>,
    log: &(dyn Fn(&str) + Send + Sync),
    progress: Option<ProgressFn<'_>>,
) -> Result<PreparedEpisode, ConversionError> {
    let uuid = episode_uuid(guid);
    let (final_audio, final_image) = output_paths(guid, work_dir);

    if final_audio.exists() {
        log("Deja prepare, rien a refaire.");
        let image = final_image.exists().then_some(final_image);
        return Ok(PreparedEpisode {
            audio: final_audio,
            image,
            uuid,
        });
    }

    create_parent_dir(&final_audio)?;
    let raw_audio = work_dir.join("raw").join(format!("{uuid}.src"));
    create_parent_dir(&raw_audio)?;

    log("Telechargement de l'audio...");
    download(audio_url, &raw_audio, &|fraction| {
        if let Some(progress) = progress {
            progress(ConversionStep::DownloadingAudio, fraction);
        }
    })
    .await?;
    log("Conversion audio (ffmpeg)...");
    convert_audio_to_mp3(&raw_audio, &final_audio, &|fraction| {
        if let Some(progress) = progress {
            progress(ConversionStep::ConvertingAudio, fraction);
        }
    })
    .await?;
    let _ = std::fs::remove_file(&raw_audio);

    let mut image_path = None;
    if let Some(image_url) = image_url {
        let image_result: Result<(), ConversionError> = async {
            log("Telechargement de l'image...");
            if let Some(progress) = progress {
                progress(ConversionStep::DownloadingImage, 0.0);
            }
            let image_data = fetch_image_bytes(image_url).await?;
            if let Some(progress) = progress {
                progress(ConversionStep::ConvertingImage, 0.0);
            }
            let badge = episode_number.map(|n| format!("#{n}"));
            convert_image_to_jpeg(&image_data, &final_image, badge.as_deref())?;
            Ok(())
        }
        .await;
        match image_result {
            Ok(()) => image_path = Some(final_image),

            Err(error) => log(&format!("Image ignoree ({error})")),
        }
    }

    log("Episode pret.");
    Ok(PreparedEpisode {
        audio: final_audio,
        image: image_path,
        uuid,
    })
}

pub async fn convert_local_audio_to_mp3(
    local_path: &Path,
    remote_base_name: &str,
    work_dir: &Path,
    on_progress: &(dyn Fn(f64) + Send + Sync),
) -> Result<PathBuf, ConversionError> {
    if !is_valid_uuid(remote_base_name) {
        return Err(ConversionError::InvalidRemoteName(
            remote_base_name.to_string(),
        ));
    }
    let output = work_dir
        .join("converted")
        .join(format!("{remote_base_name}.mp3"));
    convert_audio_to_mp3(local_path, &output, on_progress).await?;
    Ok(output)
}

fn create_parent_dir(path: &Path) -> Result<(), ConversionError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
    }
    Ok(())
}

async fn download(
    url: &str,
    destination: &Path,
    on_progress: &(dyn Fn(f64) + Send + Sync),
) -> Result<(), ConversionError> {
    on_progress(0.0);
    if let Some(path) = url.strip_prefix("file://") {
        let _ = std::fs::remove_file(destination);
        std::fs::copy(path, destination)
            .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
        on_progress(1.0);
        return Ok(());
    }
    info!("Debut telechargement : {url}");
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TOTAL_TIMEOUT)
        .connect_timeout(Duration::from_secs(30))
        .user_agent(DOWNLOAD_USER_AGENT)
        .build()
        .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ConversionError::DownloadFailed(format!(
            "code HTTP {}",
            response.status().as_u16()
        )));
    }
    let total = response.content_length();
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, response.chunk())
        .await
        .map_err(|_| {
            ConversionError::DownloadFailed(
                "telechargement bloque (aucune donnee recue a temps)".to_string(),
            )
        })?
        .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?
    {
        bytes.extend_from_slice(&chunk);
        if let Some(total) = total
            && total > 0
        {
            on_progress((bytes.len() as f64 / total as f64).min(1.0));
        }
    }
    info!(
        "Telechargement termine : {url}, taille={}, debut={}",
        bytes.len(),
        bytes
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );

    let magic = &bytes[..bytes.len().min(16)];
    if looks_like_text(magic) {
        let preview = String::from_utf8_lossy(magic).into_owned();
        return Err(ConversionError::DownloadFailed(format!(
            "contenu recu invalide (probablement une page web, pas de l'audio) : \"{preview}\""
        )));
    }

    std::fs::write(destination, &bytes)
        .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
    Ok(())
}

fn looks_like_text(magic: &[u8]) -> bool {
    if magic.is_empty() {
        return false;
    }
    magic
        .iter()
        .all(|&byte| (0x20..=0x7E).contains(&byte) || byte == 0x0A || byte == 0x0D || byte == 0x09)
}

async fn convert_audio_to_mp3(
    input: &Path,
    output: &Path,
    on_progress: &(dyn Fn(f64) + Send + Sync),
) -> Result<(), ConversionError> {
    let Some(ffmpeg_path) = super::ffmpeg_provider::resolve() else {
        return Err(ConversionError::FfmpegNotFound);
    };
    create_parent_dir(output)?;

    let partial = output.with_extension("mp3.partial");
    let _ = std::fs::remove_file(&partial);

    on_progress(0.0);
    let mut child = tokio::process::Command::new(&ffmpeg_path)
        .args([
            "-y",
            "-i",
            &input.to_string_lossy(),
            "-ac",
            &TARGET_CHANNELS.to_string(),
            "-ar",
            &TARGET_SAMPLE_RATE.to_string(),
            "-b:a",
            TARGET_BITRATE,
            "-codec:a",
            "libmp3lame",
            "-f",
            "mp3",
            &partial.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ConversionError::FfmpegFailed(e.to_string()))?;

    info!(
        "ffmpeg demarre : {} -> {}",
        input.display(),
        output.display()
    );

    let mut stderr = child.stderr.take().expect("stderr piped");
    let mut tracker = FfmpegProgressTracker::new();

    let drain_and_wait = async {
        let mut chunk = [0u8; 4096];
        loop {
            match stderr.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&chunk[..n]).into_owned();
                    debug!("ffmpeg stderr: {text}");
                    if let Some(fraction) = tracker.append(&text) {
                        on_progress(fraction);
                    }
                }
            }
        }
        child.wait().await
    };
    let timeout_result = tokio::time::timeout(FFMPEG_TIMEOUT, drain_and_wait).await;
    let status = match timeout_result {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            let _ = std::fs::remove_file(&partial);
            return Err(ConversionError::FfmpegFailed(error.to_string()));
        }
        Err(_) => {
            let _ = child.start_kill();
            let _ = std::fs::remove_file(&partial);
            return Err(ConversionError::FfmpegFailed(
                "conversion ffmpeg trop longue (timeout depasse)".to_string(),
            ));
        }
    };
    info!("ffmpeg termine, code={:?}", status.code());
    if status.success() {
        std::fs::rename(&partial, output).map_err(|e| ConversionError::FfmpegFailed(e.to_string()))
    } else {
        let _ = std::fs::remove_file(&partial);
        let tail: String = tracker
            .error_text
            .chars()
            .rev()
            .take(2000)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        Err(ConversionError::FfmpegFailed(tail))
    }
}

struct FfmpegProgressTracker {
    recent_text: String,
    error_text: String,
    duration_seconds: Option<f64>,
}

const RECENT_TEXT_CAP: usize = 4096;

impl FfmpegProgressTracker {
    fn new() -> Self {
        Self {
            recent_text: String::new(),
            error_text: String::new(),
            duration_seconds: None,
        }
    }

    fn append(&mut self, chunk: &str) -> Option<f64> {
        if self.error_text.len() < RECENT_TEXT_CAP * 4 {
            self.error_text.push_str(chunk);
        }
        self.recent_text.push_str(chunk);
        if self.recent_text.len() > RECENT_TEXT_CAP {
            let excess = self.recent_text.len() - RECENT_TEXT_CAP;

            let cut = (excess..self.recent_text.len())
                .find(|&i| self.recent_text.is_char_boundary(i))
                .unwrap_or(0);
            self.recent_text.drain(..cut);
        }
        if self.duration_seconds.is_none() {
            self.duration_seconds = parse_timecode(&self.recent_text, "Duration: ", false);
        }
        let duration = self.duration_seconds?;
        let current = parse_timecode(&self.recent_text, "time=", true)?;
        if duration > 0.0 {
            Some((current / duration).min(1.0))
        } else {
            None
        }
    }
}

fn parse_timecode(text: &str, prefix: &str, from_end: bool) -> Option<f64> {
    let position = if from_end {
        text.rfind(prefix)
    } else {
        text.find(prefix)
    };
    let start = position? + prefix.len();
    let rest: String = text[start..].chars().take(12).collect();
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let hours: f64 = parts[0].parse().ok()?;
    let minutes: f64 = parts[1].parse().ok()?;
    let seconds_text: String = parts[2].chars().take(5).collect();
    let seconds: f64 = seconds_text.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_duration_then_time_progress() {
        let mut tracker = FfmpegProgressTracker::new();
        assert_eq!(
            tracker.append("Input #0, mp3\n  Duration: 00:10:00.00, start"),
            None
        );
        let fraction = tracker.append("size=1024 time=00:05:00.00 bitrate=128k");
        assert_eq!(fraction, Some(0.5));

        let fraction = tracker.append(" time=00:07:30.00 speed=40x");
        assert_eq!(fraction, Some(0.75));
    }

    #[test]
    fn progress_is_capped_at_one() {
        let mut tracker = FfmpegProgressTracker::new();
        tracker.append("Duration: 00:01:00.00");
        assert_eq!(tracker.append("time=00:02:00.00"), Some(1.0));
    }

    #[test]
    fn no_progress_without_duration() {
        let mut tracker = FfmpegProgressTracker::new();
        assert_eq!(tracker.append("time=00:05:00.00"), None);
    }

    #[test]
    fn timecode_parsing_is_correct() {
        assert_eq!(
            parse_timecode("Duration: 01:02:03.45, start", "Duration: ", false),
            Some(3723.45)
        );
        assert_eq!(parse_timecode("pas de timecode", "Duration: ", false), None);
    }

    #[test]
    fn html_response_is_detected_as_text() {
        assert!(looks_like_text(b"<!DOCTYPE html><ht"));
        assert!(looks_like_text(b"<html>\n<head>"));
        assert!(!looks_like_text(&[0xFF, 0xFB, 0x90, 0x00]));
        assert!(!looks_like_text(b"ID3\x04\x00"));
    }

    #[tokio::test]
    async fn real_ffmpeg_conversion_produces_mp3() {
        if super::super::ffmpeg_provider::resolve().is_none() {
            eprintln!("ffmpeg absent - test saute");
            return;
        }

        let samples: u32 = 8000;
        let data_len = samples * 2;
        let mut wav: Vec<u8> = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8000u32.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.extend(std::iter::repeat_n(0u8, data_len as usize));

        let dir = std::env::temp_dir().join("merlin-ffmpeg-test");
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.wav");
        let output = dir.join("out.mp3");
        let _ = std::fs::remove_file(&output);
        std::fs::write(&input, &wav).unwrap();

        convert_audio_to_mp3(&input, &output, &|_| {})
            .await
            .expect("conversion ffmpeg");

        let bytes = std::fs::read(&output).expect("mp3 produit");
        assert!(!bytes.is_empty());
        let is_mp3 = bytes.starts_with(b"ID3") || (bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0);
        assert!(is_mp3, "la sortie doit être un MP3 (ID3 ou trame MPEG)");
    }
}
