use std::path::{Path, PathBuf};
use std::time::Duration;

use image::imageops::FilterType;
use uuid::Uuid;

pub const TARGET_IMAGE_SIZE: u32 = 128;
const JPEG_QUALITY: u8 = 90;

pub(crate) const DOWNLOAD_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("ffmpeg introuvable dans le PATH - installe-le (brew install ffmpeg)")]
    FfmpegNotFound,
    #[error("echec de conversion audio : {0}")]
    FfmpegFailed(String),
    #[error("echec de telechargement : {0}")]
    DownloadFailed(String),
    #[error("image invalide ou format non reconnu")]
    ImageDecodeFailed,

    #[error("nom distant invalide ({0}) - refuse par securite")]
    InvalidRemoteName(String),
}

pub fn is_valid_uuid(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok()
}

pub async fn download_and_convert_folder_image(
    source: &str,
    folder_uuid: &str,
    work_dir: &Path,
) -> Result<PathBuf, ConversionError> {
    if !is_valid_uuid(folder_uuid) {
        return Err(ConversionError::InvalidRemoteName(folder_uuid.to_string()));
    }
    let output = work_dir
        .join("converted")
        .join(format!("{folder_uuid}.jpg"));
    if output.exists() {
        return Ok(output);
    }
    let image_data = fetch_image_bytes(source).await?;
    convert_image_to_jpeg(&image_data, &output, None)?;
    Ok(output)
}

pub fn convert_local_image_to_jpeg(
    local_path: &Path,
    remote_base_name: &str,
    work_dir: &Path,
) -> Result<PathBuf, ConversionError> {
    if !is_valid_uuid(remote_base_name) {
        return Err(ConversionError::InvalidRemoteName(
            remote_base_name.to_string(),
        ));
    }
    let output = work_dir
        .join("converted")
        .join(format!("{remote_base_name}.jpg"));
    let image_data =
        std::fs::read(local_path).map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
    convert_image_to_jpeg(&image_data, &output, None)?;
    Ok(output)
}

pub async fn fetch_image_bytes(source: &str) -> Result<Vec<u8>, ConversionError> {
    if let Some(path) = source.strip_prefix("file://") {
        return std::fs::read(path).map_err(|e| ConversionError::DownloadFailed(e.to_string()));
    }
    if source.starts_with('/') {
        return std::fs::read(source).map_err(|e| ConversionError::DownloadFailed(e.to_string()));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(DOWNLOAD_USER_AGENT)
        .build()
        .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
    let response = client
        .get(source)
        .send()
        .await
        .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ConversionError::DownloadFailed(format!(
            "code HTTP {}",
            response.status().as_u16()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| ConversionError::DownloadFailed(e.to_string()))?;
    Ok(bytes.to_vec())
}

pub(crate) fn convert_image_to_jpeg(
    data: &[u8],
    output: &Path,
    badge: Option<&str>,
) -> Result<(), ConversionError> {
    let decoded = image::load_from_memory(data).map_err(|_| ConversionError::ImageDecodeFailed)?;
    let resized = decoded
        .resize_exact(TARGET_IMAGE_SIZE, TARGET_IMAGE_SIZE, FilterType::Lanczos3)
        .to_rgb8();

    let resized = match badge {
        Some(text) => super::badge::draw_badge(resized, text),
        None => resized,
    };

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|_| ConversionError::ImageDecodeFailed)?;
    }
    let file = std::fs::File::create(output).map_err(|_| ConversionError::ImageDecodeFailed)?;
    let mut writer = std::io::BufWriter::new(file);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, JPEG_QUALITY);
    resized
        .write_with_encoder(encoder)
        .map_err(|_| ConversionError::ImageDecodeFailed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_png() -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        let img = image::RgbImage::from_fn(64, 32, |x, _| image::Rgb([(x * 4) as u8, 0, 128]));
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buffer, image::ImageFormat::Png)
            .expect("png");
        buffer.into_inner()
    }

    #[test]
    fn output_is_baseline_jpeg_128x128() {
        let output = std::env::temp_dir().join("merlin-test-images/out-baseline.jpg");
        let _ = std::fs::remove_file(&output);
        convert_image_to_jpeg(&sample_png(), &output, None).expect("conversion");

        let bytes = std::fs::read(&output).expect("lecture");
        assert_eq!(&bytes[..2], &[0xFF, 0xD8], "signature JPEG");
        let has_sof0 = bytes.windows(2).any(|w| w == [0xFF, 0xC0]);
        let has_sof2 = bytes.windows(2).any(|w| w == [0xFF, 0xC2]);
        assert!(has_sof0, "SOF0 (baseline) attendu");
        assert!(!has_sof2, "SOF2 (progressif) interdit par le firmware");

        let decoded = image::load_from_memory(&bytes).expect("decode");
        assert_eq!((decoded.width(), decoded.height()), (128, 128));
    }

    #[test]
    fn invalid_folder_uuid_is_rejected_before_any_io() {
        let result = futures_executor(download_and_convert_folder_image(
            "https://example.com/x.jpg",
            "../../etc/passwd",
            &std::env::temp_dir(),
        ));
        assert!(matches!(result, Err(ConversionError::InvalidRemoteName(_))));
    }

    #[test]
    fn undecodable_image_fails_cleanly() {
        let output = std::env::temp_dir().join("merlin-test-images/out-bad.jpg");
        let result = convert_image_to_jpeg(b"pas une image", &output, None);
        assert!(matches!(result, Err(ConversionError::ImageDecodeFailed)));
    }

    fn futures_executor<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }
}
