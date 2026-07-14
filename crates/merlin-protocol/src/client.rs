use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use crate::commands;
use crate::crc32_mpeg2;
use crate::firmware_error_catalog;

#[derive(Debug, Clone, thiserror::Error)]
pub enum MerlinConnectionError {
    #[error("non connecte")]
    NotConnected,
    #[error("connexion a l'enceinte impossible : {0}")]
    ConnectionFailed(String),
    #[error("connexion a l'enceinte perdue en envoyant : {0}")]
    SendFailed(String),
    #[error("corps de plus de 255 octets - le framing 1-octet-longueur ne s'applique pas ici")]
    BodyTooLong,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DownloadError {
    #[error("fichier introuvable sur l'enceinte : {0}")]
    NotFound(String),
    #[error("timeout en telechargeant {0}")]
    Timeout(String),
    #[error("{0} : contenu corrompu en transit (CRC de fin de flux invalide)")]
    Corrupted(String),
    #[error(transparent)]
    Connection(#[from] MerlinConnectionError),
}

pub type SatisfiedPredicate<'a> = &'a (dyn Fn(&[Frame]) -> bool + Send + Sync);

pub type DownloadProgressFn<'a> = &'a (dyn Fn(usize, usize) + Send + Sync);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub length: u8,
    pub body: Vec<u8>,
}

impl Frame {
    pub fn opcode(&self) -> Option<u8> {
        self.body.first().copied()
    }
}

struct Shared {
    buffer: Mutex<Vec<u8>>,
    receiver_error: Mutex<Option<String>>,

    scrub_enabled: std::sync::atomic::AtomicBool,
}

struct ScrubSuspension(Arc<Shared>);

impl Drop for ScrubSuspension {
    fn drop(&mut self) {
        self.0
            .scrub_enabled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

struct Established {
    write: OwnedWriteHalf,
    shared: Arc<Shared>,
    receiver: JoinHandle<()>,
}

pub struct MerlinClient {
    host: String,
    port: u16,

    connect_timeout: Duration,

    prepared: bool,
    established: Option<Established>,
}

const POLL_INTERVAL: Duration = Duration::from_millis(15);

const BULK_CHUNK_SIZE: usize = 1400;

const HEADER_TAIL_LEN: usize = 4 + 32 + 4;

impl MerlinClient {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            connect_timeout: Duration::from_secs(10),
            prepared: false,
            established: None,
        }
    }

    pub fn connect(&mut self, timeout: Duration) {
        if self.prepared {
            return;
        }
        self.connect_timeout = timeout;
        self.prepared = true;
        info!("connect() prepare vers {}:{}", self.host, self.port);
    }

    pub async fn close(&mut self) {
        info!("close()");
        if let Some(established) = self.established.take() {
            established.receiver.abort();
            let _ = established.receiver.await;
        }
        self.prepared = false;
    }

    async fn establish(&mut self) -> Result<(), MerlinConnectionError> {
        let stream = tokio::time::timeout(
            self.connect_timeout,
            TcpStream::connect((self.host.as_str(), self.port)),
        )
        .await
        .map_err(|_| {
            MerlinConnectionError::ConnectionFailed(format!(
                "timeout apres {}s",
                self.connect_timeout.as_secs_f64()
            ))
        })?
        .map_err(|e| MerlinConnectionError::ConnectionFailed(e.to_string()))?;

        stream
            .set_nodelay(true)
            .map_err(|e| MerlinConnectionError::ConnectionFailed(e.to_string()))?;

        let (read, write) = stream.into_split();
        let shared = Arc::new(Shared {
            buffer: Mutex::new(Vec::new()),
            receiver_error: Mutex::new(None),
            scrub_enabled: std::sync::atomic::AtomicBool::new(true),
        });
        let receiver = tokio::spawn(receiver_loop(read, Arc::clone(&shared)));
        self.established = Some(Established {
            write,
            shared,
            receiver,
        });
        Ok(())
    }

    fn shared(&self) -> Option<&Arc<Shared>> {
        self.established.as_ref().map(|e| &e.shared)
    }

    fn receiver_error(&self) -> Option<String> {
        self.shared()
            .and_then(|s| s.receiver_error.lock().expect("lock").clone())
    }

    pub async fn send_frame(&mut self, body: &[u8]) -> Result<(), MerlinConnectionError> {
        if !self.prepared {
            return Err(MerlinConnectionError::NotConnected);
        }
        if body.len() > 255 {
            return Err(MerlinConnectionError::BodyTooLong);
        }
        let opcode_hex = body
            .first()
            .map(|b| format!("0x{b:02x}"))
            .unwrap_or_else(|| "?".to_string());
        if self.established.is_none() {
            self.establish()
                .await
                .map_err(|e| MerlinConnectionError::SendFailed(e.to_string()))?;
            info!("connexion etablie (1er envoi, opcode={opcode_hex})");
        }

        if let Some(established) = &self.established {
            let mut buffer = established.shared.buffer.lock().expect("lock");
            if !buffer.is_empty() {
                debug!(
                    "purge de {} octet(s) residuels avant une nouvelle commande",
                    buffer.len()
                );
                buffer.clear();
            }
        }
        let mut data = Vec::with_capacity(1 + body.len());
        data.push(body.len() as u8);
        data.extend_from_slice(body);
        debug!(
            "envoi {} octet(s) (opcode={opcode_hex}) : {}",
            data.len(),
            hex_dump(&data)
        );

        let established = self.established.as_mut().expect("établie juste au-dessus");
        established.write.write_all(&data).await.map_err(|e| {
            error!("send_frame echec (opcode={opcode_hex}) : {e}");
            MerlinConnectionError::SendFailed(e.to_string())
        })
    }

    fn parse_buffered_frames(buffer: &[u8]) -> Vec<Frame> {
        let mut frames = Vec::new();
        let mut i = 0;
        while i < buffer.len() {
            let length = buffer[i] as usize;
            if i + 1 + length > buffer.len() {
                break;
            }
            frames.push(Frame {
                length: length as u8,
                body: buffer[i + 1..i + 1 + length].to_vec(),
            });
            i += 1 + length;
        }
        frames
    }

    pub async fn read_frames(
        &mut self,
        timeout: Duration,
        satisfied: Option<SatisfiedPredicate<'_>>,
    ) -> Result<Vec<Frame>, MerlinConnectionError> {
        if !self.prepared {
            return Err(MerlinConnectionError::NotConnected);
        }
        let Some(shared) = self.shared().cloned() else {
            return Err(MerlinConnectionError::NotConnected);
        };

        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            if let Some(receiver_error) = self.receiver_error() {
                return Err(MerlinConnectionError::SendFailed(receiver_error));
            }
            let ready = Self::parse_buffered_frames(&shared.buffer.lock().expect("lock"));
            let is_satisfied = match satisfied {
                Some(predicate) => predicate(&ready),
                None => !ready.is_empty(),
            };
            if is_satisfied {
                break;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        let mut buffer = shared.buffer.lock().expect("lock");
        let frames = Self::parse_buffered_frames(&buffer);
        let consumed: usize = frames.iter().map(|f| 1 + f.length as usize).sum();
        buffer.drain(..consumed);
        Ok(frames)
    }

    pub async fn send_frame_command(
        &mut self,
        body: &[u8],
        response_timeout: Duration,
        satisfied: Option<SatisfiedPredicate<'_>>,
    ) -> Result<Vec<Frame>, MerlinConnectionError> {
        self.send_frame(body).await?;
        self.read_frames(response_timeout, satisfied).await
    }

    pub async fn download_file(
        &mut self,
        name: &str,
        timeout: Duration,
    ) -> Result<Vec<u8>, DownloadError> {
        self.download_file_with_progress(name, timeout, None).await
    }

    pub async fn download_file_with_progress(
        &mut self,
        name: &str,
        timeout: Duration,
        on_progress: Option<DownloadProgressFn<'_>>,
    ) -> Result<Vec<u8>, DownloadError> {
        self.send_frame(&commands::download_file(name)).await?;
        let shared = self
            .shared()
            .cloned()
            .ok_or(MerlinConnectionError::NotConnected)?;

        shared
            .scrub_enabled
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let _scrub_guard = ScrubSuspension(Arc::clone(&shared));

        let name_bytes = name.as_bytes();
        let mut marker = vec![commands::OP_DOWNLOAD_FILE, 0x00, name_bytes.len() as u8];
        marker.extend_from_slice(name_bytes);
        let deadline = tokio::time::Instant::now() + timeout;

        let find_marker = |buffer: &[u8]| -> Option<usize> {
            buffer
                .windows(marker.len())
                .position(|window| window == marker)
        };

        let mut marker_start: Option<usize> = None;
        while tokio::time::Instant::now() < deadline {
            if let Some(receiver_error) = self.receiver_error() {
                return Err(MerlinConnectionError::SendFailed(receiver_error).into());
            }
            {
                let mut buffer = shared.buffer.lock().expect("lock");
                marker_start = find_marker(&buffer);
                if marker_start.is_some() {
                    break;
                }

                if buffer.len() >= 2 && buffer.len() <= 8 {
                    let frames = Self::parse_buffered_frames(&buffer);
                    if frames.iter().any(|f| {
                        f.opcode() == Some(commands::OP_DOWNLOAD_FILE)
                            && f.body.len() > 1
                            && f.body[1] != 0
                    }) {
                        buffer.clear();
                        return Err(DownloadError::NotFound(name.to_string()));
                    }
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        let Some(marker_pos) = marker_start else {
            return Err(DownloadError::Timeout(name.to_string()));
        };

        let size_field_start = marker_pos + marker.len();

        let Some(header) =
            wait_and_copy(&shared, size_field_start, HEADER_TAIL_LEN, timeout, None).await
        else {
            return Err(DownloadError::Timeout(name.to_string()));
        };
        let (size_bytes, rest) = header.split_at(4);
        let (sha256, frame_crc) = rest.split_at(32);
        let size = u32::from_le_bytes(size_bytes.try_into().expect("4 octets exactement")) as usize;

        let expected_crc = crc32_mpeg2::checksum_parts(&[&marker, &header[..36]]).to_le_bytes();
        if expected_crc != frame_crc {
            error!(
                "{name} : CRC d'en-tete invalide (attendu {}, recu {})",
                hex_dump(&expected_crc),
                hex_dump(frame_crc)
            );
            return Err(DownloadError::Corrupted(name.to_string()));
        }
        let content_start = size_field_start + HEADER_TAIL_LEN;

        let relay;
        let on_growth: Option<&(dyn Fn(usize) + Send + Sync)> = match on_progress {
            Some(on_progress) => {
                relay = move |buffered: usize| {
                    on_progress(buffered.saturating_sub(content_start).min(size), size);
                };
                Some(&relay)
            }
            None => None,
        };

        let Some(content) = wait_and_copy(&shared, content_start, size, timeout, on_growth).await
        else {
            return Err(DownloadError::Timeout(name.to_string()));
        };

        if let Some(on_progress) = on_progress {
            on_progress(size, size);
        }

        {
            let mut buffer = shared.buffer.lock().expect("lock");
            let to_remove = (content_start + size).min(buffer.len());
            buffer.drain(..to_remove);
        }

        let digest = Sha256::digest(&content);
        if digest.as_slice() != sha256 {
            error!(
                "{name} : SHA-256 invalide (annonce {}, calcule {})",
                hex_dump(sha256),
                hex_dump(&digest)
            );
            return Err(DownloadError::Corrupted(name.to_string()));
        }
        debug!("{name} : SHA-256 verifie ({size} octets)");
        Ok(content)
    }

    pub async fn send_bulk(
        &mut self,
        data: &[u8],
        timeout: Duration,
        mut on_progress: Option<&mut (dyn FnMut(usize, usize) + Send)>,
    ) -> Result<(), MerlinConnectionError> {
        let Some(established) = self.established.as_mut() else {
            return Err(MerlinConnectionError::NotConnected);
        };
        let total = data.len();
        let overall_start = tokio::time::Instant::now();
        info!(
            "send_bulk debut : {total} octets, timeout={}s",
            timeout.as_secs_f64()
        );

        let write = &mut established.write;
        let result = tokio::time::timeout(timeout, async {
            let mut sent = 0;
            while sent < total {
                let end = (sent + BULK_CHUNK_SIZE).min(total);
                write.write_all(&data[sent..end]).await?;
                sent = end;
                if let Some(progress) = on_progress.as_deref_mut() {
                    progress(sent, total);
                }
            }
            Ok::<(), std::io::Error>(())
        })
        .await;

        let elapsed = overall_start.elapsed();
        match result {
            Ok(Ok(())) => {
                let avg_kbps = if elapsed.as_secs_f64() > 0.0 {
                    total as f64 / 1024.0 / elapsed.as_secs_f64()
                } else {
                    0.0
                };
                info!(
                    "send_bulk fini : {total} octets en {:.1}s ({avg_kbps:.1} Ko/s moyen)",
                    elapsed.as_secs_f64()
                );
                Ok(())
            }
            Ok(Err(e)) => {
                error!("send_bulk echec apres {:.1}s : {e}", elapsed.as_secs_f64());
                Err(MerlinConnectionError::SendFailed(format!(
                    "{e} (apres {}s)",
                    elapsed.as_secs()
                )))
            }
            Err(_) => {
                error!("send_bulk timeout apres {:.1}s", elapsed.as_secs_f64());
                Err(MerlinConnectionError::SendFailed(format!(
                    "timeout apres {}s (apres {}s)",
                    timeout.as_secs_f64(),
                    elapsed.as_secs()
                )))
            }
        }
    }
}

async fn wait_and_copy(
    shared: &Shared,
    start: usize,
    len: usize,
    idle_timeout: Duration,
    on_growth: Option<&(dyn Fn(usize) + Send + Sync)>,
) -> Option<Vec<u8>> {
    let mut seen_len = 0usize;
    let mut last_progress = tokio::time::Instant::now();
    loop {
        let grew = {
            let buffer = shared.buffer.lock().expect("lock");
            if buffer.len() >= start + len {
                return Some(buffer[start..start + len].to_vec());
            }
            if buffer.len() > seen_len {
                seen_len = buffer.len();
                last_progress = tokio::time::Instant::now();
                true
            } else {
                false
            }
        };
        if grew && let Some(on_growth) = on_growth {
            on_growth(seen_len);
        }
        if last_progress.elapsed() >= idle_timeout {
            return None;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn receiver_loop(mut read: tokio::net::tcp::OwnedReadHalf, shared: Arc<Shared>) {
    let mut chunk = vec![0u8; 65536];
    loop {
        match read.read(&mut chunk).await {
            Ok(0) => {
                *shared.receiver_error.lock().expect("lock") =
                    Some("connexion fermee par l'enceinte".to_string());
                return;
            }
            Ok(n) => {
                debug!("recu {n} octet(s) : {}", hex_dump(&chunk[..n]));
                let mut buffer = shared.buffer.lock().expect("lock");
                buffer.extend_from_slice(&chunk[..n]);
                if shared
                    .scrub_enabled
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    log_framing_errors_in_buffer(&mut buffer);
                }
            }
            Err(e) => {
                error!("receiver_loop: erreur de reception : {e}");
                *shared.receiver_error.lock().expect("lock") = Some(e.to_string());
                return;
            }
        }
    }
}

fn log_framing_errors_in_buffer(buffer: &mut Vec<u8>) {
    let mut i = 0;
    let mut kept = Vec::with_capacity(buffer.len());
    while i < buffer.len() {
        let length = buffer[i] as usize;
        if i + 1 + length > buffer.len() {
            kept.extend_from_slice(&buffer[i..]);
            break;
        }
        let frame_end = i + 1 + length;
        let body = &buffer[i + 1..frame_end];
        if length == 6 && body.first() == Some(&0xFF) {
            let status = if body.len() > 1 { body[1] } else { 0xFF };
            let meaning = firmware_error_catalog::framing_error_status(status);
            error!(
                "erreur de framing/CRC de l'enceinte : [0xFF][0x{status:02x}] ({meaning}) - trame precedente rejetee"
            );
        } else {
            kept.extend_from_slice(&buffer[i..frame_end]);
        }
        i = frame_end;
    }
    *buffer = kept;
}

fn hex_dump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn spawn_echo_server() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];
                    loop {
                        match socket.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => {
                                if socket.write_all(&buf[..n]).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                });
            }
        });
        port
    }

    #[tokio::test]
    async fn connect_send_frame_and_read_frames_round_trip() {
        let port = spawn_echo_server().await;

        let mut client = MerlinClient::new("127.0.0.1", port);
        client.connect(Duration::from_secs(5));

        let frames = client
            .send_frame_command(&commands::connect(), Duration::from_secs(5), None)
            .await
            .expect("commande");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].body, commands::connect());

        client.close().await;
    }

    #[tokio::test]
    async fn multiple_sequential_commands_all_succeed() {
        let port = spawn_echo_server().await;

        let mut client = MerlinClient::new("127.0.0.1", port);
        client.connect(Duration::from_secs(5));

        for opcode in [0x02u8, 0x04, 0x03, 0x0E, 0x05, 0x1E, 0x1B] {
            let frames = client
                .send_frame_command(&[opcode, 0xAA, 0xBB], Duration::from_secs(5), None)
                .await
                .unwrap_or_else(|e| panic!("echec sur opcode 0x{opcode:02x} : {e}"));
            assert_eq!(frames[0].body, [opcode, 0xAA, 0xBB]);
        }

        client.close().await;
    }

    #[tokio::test]
    async fn send_frame_fails_bounded_when_nothing_listens() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            listener.local_addr().expect("addr").port()
        };
        let mut client = MerlinClient::new("127.0.0.1", port);
        client.connect(Duration::from_secs(3));

        let start = std::time::Instant::now();
        let result = client
            .send_frame_command(&commands::connect(), Duration::from_secs(3), None)
            .await;
        assert!(
            result.is_err(),
            "attendu : échec (rien n'écoute sur ce port)"
        );
        assert!(start.elapsed() < Duration::from_secs(4));
        client.close().await;
    }

    fn empty_shared() -> Arc<Shared> {
        Arc::new(Shared {
            buffer: Mutex::new(Vec::new()),
            receiver_error: Mutex::new(None),
            scrub_enabled: std::sync::atomic::AtomicBool::new(true),
        })
    }

    #[tokio::test]
    async fn wait_and_copy_survives_a_slow_but_progressing_transfer() {
        let shared = empty_shared();
        let writer = Arc::clone(&shared);

        tokio::spawn(async move {
            for _ in 0..20 {
                tokio::time::sleep(Duration::from_millis(30)).await;
                writer
                    .buffer
                    .lock()
                    .expect("lock")
                    .extend_from_slice(&[0xAB; 10]);
            }
        });

        let start = tokio::time::Instant::now();
        let content = wait_and_copy(&shared, 0, 200, Duration::from_millis(100), None).await;
        assert_eq!(
            content,
            Some(vec![0xAB; 200]),
            "un flux qui progresse ne doit pas expirer"
        );
        assert!(
            start.elapsed() >= Duration::from_millis(500),
            "le test doit bien durer plus que le délai d'inactivité"
        );
    }

    enum Tamper {
        None,

        Content,

        HeaderCrc,
    }

    async fn spawn_download_server(name: &str, content: Vec<u8>, tamper: Tamper) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let name = name.to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request = vec![0u8; 1024];
            let _ = socket.read(&mut request).await;

            let mut body = vec![commands::OP_DOWNLOAD_FILE, 0x00, name.len() as u8];
            body.extend_from_slice(name.as_bytes());
            body.extend_from_slice(&(content.len() as u32).to_le_bytes());
            body.extend_from_slice(&Sha256::digest(&content));
            let mut crc = crc32_mpeg2::checksum_le(&body);
            if matches!(tamper, Tamper::HeaderCrc) {
                crc[0] ^= 0xFF;
            }
            body.extend_from_slice(&crc);

            let mut response = vec![body.len() as u8];
            response.extend_from_slice(&body);
            let mut streamed = content.clone();
            if matches!(tamper, Tamper::Content) {
                streamed[0] ^= 0xFF;
            }
            response.extend_from_slice(&streamed);

            let _ = socket.write_all(&response).await;

            tokio::time::sleep(Duration::from_secs(10)).await;
        });
        port
    }

    #[tokio::test]
    async fn download_reads_the_content_after_the_header_frame() {
        let content: Vec<u8> = (0..=255u8).cycle().take(3000).collect();
        let port = spawn_download_server("song.mp3", content.clone(), Tamper::None).await;
        let mut client = MerlinClient::new("127.0.0.1", port);
        client.connect(Duration::from_secs(5));

        let got = client
            .download_file("song.mp3", Duration::from_secs(5))
            .await;
        assert_eq!(got.expect("téléchargement"), content);
        client.close().await;
    }

    #[tokio::test]
    async fn download_rejects_a_content_that_does_not_match_the_announced_sha256() {
        let content: Vec<u8> = (0..=255u8).cycle().take(3000).collect();
        let port = spawn_download_server("song.mp3", content, Tamper::Content).await;
        let mut client = MerlinClient::new("127.0.0.1", port);
        client.connect(Duration::from_secs(5));

        let error = client
            .download_file("song.mp3", Duration::from_secs(5))
            .await;
        assert!(
            matches!(error, Err(DownloadError::Corrupted(ref n)) if n == "song.mp3"),
            "attendu Corrupted, obtenu {error:?}",
        );
        client.close().await;
    }

    #[tokio::test]
    async fn download_rejects_a_header_frame_whose_crc_is_wrong() {
        let content: Vec<u8> = (0..=255u8).cycle().take(300).collect();
        let port = spawn_download_server("cover.jpg", content, Tamper::HeaderCrc).await;
        let mut client = MerlinClient::new("127.0.0.1", port);
        client.connect(Duration::from_secs(5));

        let error = client
            .download_file("cover.jpg", Duration::from_secs(5))
            .await;
        assert!(
            matches!(error, Err(DownloadError::Corrupted(ref n)) if n == "cover.jpg"),
            "attendu Corrupted, obtenu {error:?}",
        );
        client.close().await;
    }

    #[tokio::test]
    async fn download_parses_the_sleep_cfg_response() {
        let captured: Vec<u8> = vec![
            0x34, 0x0d, 0x00, 0x09, 0x73, 0x6c, 0x65, 0x65, 0x70, 0x2e, 0x63, 0x66, 0x67, 0x08,
            0x00, 0x00, 0x00, 0xaf, 0x55, 0x70, 0xf5, 0xa1, 0x81, 0x0b, 0x7a, 0xf7, 0x8c, 0xaf,
            0x4b, 0xc7, 0x0a, 0x66, 0x0f, 0x0d, 0xf5, 0x1e, 0x42, 0xba, 0xf9, 0x1d, 0x4d, 0xe5,
            0xb2, 0x32, 0x8d, 0xe0, 0xe8, 0x3d, 0xfc, 0x78, 0xaa, 0x70, 0x9b, 0, 0, 0, 0, 0, 0, 0,
            0,
        ];
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request = vec![0u8; 1024];
            let _ = socket.read(&mut request).await;
            let _ = socket.write_all(&captured).await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        let mut client = MerlinClient::new("127.0.0.1", port);
        client.connect(Duration::from_secs(5));
        let got = client
            .download_file("sleep.cfg", Duration::from_secs(5))
            .await;
        assert_eq!(got.expect("téléchargement"), vec![0u8; 8]);
        client.close().await;
    }

    #[tokio::test]
    async fn wait_and_copy_reports_growth_while_the_transfer_runs() {
        let shared = empty_shared();
        let writer = Arc::clone(&shared);
        tokio::spawn(async move {
            for _ in 0..5 {
                tokio::time::sleep(Duration::from_millis(30)).await;
                writer
                    .buffer
                    .lock()
                    .expect("lock")
                    .extend_from_slice(&[0xAB; 10]);
            }
        });

        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&seen);
        let on_growth = move |len: usize| recorder.lock().expect("lock").push(len);

        let content =
            wait_and_copy(&shared, 0, 50, Duration::from_millis(100), Some(&on_growth)).await;
        assert_eq!(content, Some(vec![0xAB; 50]));

        let seen = seen.lock().expect("lock").clone();
        assert!(
            seen.len() >= 2,
            "progression attendue en cours de route, vu : {seen:?}"
        );
        assert!(
            seen.windows(2).all(|w| w[0] < w[1]),
            "doit croître : {seen:?}"
        );
        assert!(
            seen.iter().all(|&len| len <= 50),
            "jamais au-delà du total : {seen:?}"
        );
    }

    #[tokio::test]
    async fn wait_and_copy_times_out_on_a_stalled_transfer() {
        let shared = empty_shared();
        shared
            .buffer
            .lock()
            .expect("lock")
            .extend_from_slice(&[0xAB; 10]);

        let start = tokio::time::Instant::now();

        let content = wait_and_copy(&shared, 0, 200, Duration::from_millis(100), None).await;
        assert_eq!(content, None, "un flux immobile doit expirer");
        assert!(start.elapsed() < Duration::from_secs(1), "et expirer vite");
    }

    #[test]
    fn framing_errors_are_logged_and_removed_from_buffer() {
        let mut buffer = vec![6, 0xFF, 0x01, 0xAA, 0xBB, 0xCC, 0xDD, 2, 0x06, 0x00];
        log_framing_errors_in_buffer(&mut buffer);
        assert_eq!(buffer, vec![2, 0x06, 0x00]);
    }

    #[test]
    fn incomplete_trailing_frame_is_kept_in_buffer() {
        let mut buffer = vec![6, 0xFF, 0x01, 0xAA, 0xBB, 0xCC, 0xDD, 5, 0x06];
        log_framing_errors_in_buffer(&mut buffer);
        assert_eq!(buffer, vec![5, 0x06]);
    }
}
