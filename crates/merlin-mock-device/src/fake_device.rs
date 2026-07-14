use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use merlin_protocol::commands::{self, send_file_status};
use merlin_protocol::crc32_mpeg2;
use merlin_protocol::firmware_error_catalog;

use merlin_domain::playlist::bin_encoder;
use merlin_domain::playlist::json_decoder::{self, DecodeError};
use merlin_domain::playlist::model::{PlaylistFolder, PlaylistNode};

use crate::store::MockDeviceStore;

struct PendingUpload {
    name: String,
    remaining: usize,
    collected: Vec<u8>,
    expected_sha256: Vec<u8>,
}

struct DeviceState {
    files: HashMap<String, Vec<u8>>,
    deleted_file_names: Vec<String>,
    receive_buffer: Vec<u8>,
    pending_upload: Option<PendingUpload>,

    update_playlist_status: u8,

    send_file_ready_status_override: Option<u8>,

    enforce_firmware_validation: bool,
    verbose: bool,

    store: Option<MockDeviceStore>,
}

impl DeviceState {
    fn log(&self, message: &str) {
        if self.verbose {
            println!("[mock] {message}");
        }
    }

    fn persist(&self, name: &str, content: &[u8]) {
        if let Some(store) = &self.store
            && let Err(e) = store.set(name, content)
        {
            self.log(&format!("persistance de {name} impossible : {e}"));
        }
    }

    fn remove_persisted(&self, name: &str) {
        if let Some(store) = &self.store
            && let Err(e) = store.delete(name)
        {
            self.log(&format!("suppression persistee de {name} impossible : {e}"));
        }
    }
}

pub struct FakeMerlinDevice {
    state: Arc<Mutex<DeviceState>>,
    accept_task: JoinHandle<()>,
    port: u16,
}

impl FakeMerlinDevice {
    pub async fn start(
        port: u16,
        initial_files: HashMap<String, Vec<u8>>,
        verbose: bool,
        store: Option<MockDeviceStore>,
    ) -> std::io::Result<Self> {
        let files = match &store {
            Some(store) => store
                .all()
                .map_err(|e| std::io::Error::other(e.to_string()))?,
            None => initial_files,
        };
        let state = Arc::new(Mutex::new(DeviceState {
            files,
            deleted_file_names: Vec::new(),
            receive_buffer: Vec::new(),
            pending_upload: None,
            update_playlist_status: 0,
            send_file_ready_status_override: None,
            enforce_firmware_validation: false,
            verbose,
            store,
        }));

        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        let actual_port = listener.local_addr()?.port();

        let accept_state = Arc::clone(&state);
        let accept_task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                {
                    let state = accept_state.lock().expect("lock");
                    state.log("connexion entrante");
                }
                let connection_state = Arc::clone(&accept_state);
                tokio::spawn(handle_connection(socket, connection_state));
            }
        });

        Ok(Self {
            state,
            accept_task,
            port: actual_port,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn stop(&self) {
        self.accept_task.abort();
    }

    pub fn set_update_playlist_status(&self, status: u8) {
        self.state.lock().expect("lock").update_playlist_status = status;
    }

    pub fn set_send_file_ready_status_override(&self, status: Option<u8>) {
        self.state
            .lock()
            .expect("lock")
            .send_file_ready_status_override = status;
    }

    pub fn set_enforce_firmware_validation(&self, enforce: bool) {
        self.state.lock().expect("lock").enforce_firmware_validation = enforce;
    }

    pub fn snapshot(&self) -> (HashMap<String, Vec<u8>>, Vec<String>) {
        let state = self.state.lock().expect("lock");
        (state.files.clone(), state.deleted_file_names.clone())
    }

    pub fn received_playlist_json(&self) -> Option<Vec<u8>> {
        self.state
            .lock()
            .expect("lock")
            .files
            .get("playlist.json")
            .cloned()
    }
}

impl Drop for FakeMerlinDevice {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

async fn handle_connection(mut socket: tokio::net::TcpStream, state: Arc<Mutex<DeviceState>>) {
    let mut chunk = vec![0u8; 65536];
    loop {
        match socket.read(&mut chunk).await {
            Ok(0) => return,
            Ok(n) => {
                let responses = {
                    let mut state = state.lock().expect("lock");
                    state.receive_buffer.extend_from_slice(&chunk[..n]);
                    drain(&mut state)
                };
                for response in responses {
                    if socket.write_all(&response).await.is_err() {
                        return;
                    }
                }
            }
            Err(e) => {
                state
                    .lock()
                    .expect("lock")
                    .log(&format!("connexion fermee ({e})"));
                return;
            }
        }
    }
}

fn drain(state: &mut DeviceState) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        if let Some(mut pending) = state.pending_upload.take() {
            if state.receive_buffer.is_empty() {
                state.pending_upload = Some(pending);
                return out;
            }
            let take = pending.remaining.min(state.receive_buffer.len());
            pending.collected.extend(state.receive_buffer.drain(..take));
            pending.remaining -= take;
            if pending.remaining == 0 {
                if state.enforce_firmware_validation && !pending.expected_sha256.is_empty() {
                    let actual = Sha256::digest(&pending.collected);
                    if actual.as_slice() != pending.expected_sha256.as_slice() {
                        state.log(&format!("SHA-256 incoherent pour {} - rejet", pending.name));
                        out.push(frame(&[
                            commands::OP_SEND_FILE,
                            send_file_status::HASH_MISMATCH,
                        ]));
                        continue;
                    }
                }
                state.log(&format!(
                    "recu : {} ({} octets)",
                    pending.name,
                    pending.collected.len()
                ));
                state.persist(&pending.name, &pending.collected);
                state.files.insert(pending.name, pending.collected);
                out.push(frame(&[commands::OP_SEND_FILE, send_file_status::SUCCESS]));
            } else {
                state.pending_upload = Some(pending);
                return out;
            }
            continue;
        }
        if state.receive_buffer.is_empty() {
            return out;
        }
        let length = state.receive_buffer[0] as usize;
        if state.receive_buffer.len() < 1 + length {
            return out;
        }
        let body: Vec<u8> = state.receive_buffer[1..1 + length].to_vec();
        state.receive_buffer.drain(..1 + length);
        handle_command(state, &body, &mut out);
    }
}

fn handle_command(state: &mut DeviceState, body: &[u8], out: &mut Vec<Vec<u8>>) {
    let Some(&opcode) = body.first() else { return };
    match opcode {
        commands::OP_CONNECT
        | commands::OP_MAC_ADDRESS
        | commands::OP_SESSION_TOKEN
        | commands::OP_TELEMETRY_0E
        | commands::OP_FIRMWARE_INFO
        | commands::OP_GET_HARDWARE_ID
        | commands::OP_TELEMETRY_1B
        | commands::OP_SET_DATE
        | commands::OP_SET_ENABLING_HOURS
        | commands::OP_END_SYNCHRONIZATION => out.push(frame(body)),

        commands::OP_DELETE_FILE => {
            if body.len() <= 5 {
                return;
            }
            let name = String::from_utf8_lossy(&body[1..body.len() - 4]).into_owned();
            state.log(&format!("suppression : {name}"));
            state.deleted_file_names.push(name.clone());
            state.files.remove(&name);
            state.remove_persisted(&name);
            out.push(frame(&[opcode, 0x00]));
        }

        commands::OP_DOWNLOAD_FILE => {
            if body.len() <= 5 {
                return;
            }
            let name = String::from_utf8_lossy(&body[1..body.len() - 4]).into_owned();
            if let Some(content) = state.files.get(&name).cloned() {
                state.log(&format!(
                    "telechargement : {name} ({} octets)",
                    content.len()
                ));
                out.push(raw_download_response(&name, &content));
            } else {
                state.log(&format!("telechargement : {name} introuvable"));
                out.push(frame(&[opcode, 0x01]));
            }
        }

        commands::OP_SEARCH_FILE => {
            if body.len() <= 6 {
                return;
            }
            let name = String::from_utf8_lossy(&body[2..body.len() - 4]).into_owned();
            let status = if state.files.contains_key(&name) {
                0x00
            } else {
                0x01
            };
            out.push(frame(&[opcode, status]));
        }

        commands::OP_SEND_FILE => {
            if body.len() <= 2 {
                return;
            }
            let name_len = body[1] as usize;
            if body.len() < 2 + name_len + 4 {
                return;
            }
            let name = String::from_utf8_lossy(&body[2..2 + name_len]).into_owned();
            let size_start = 2 + name_len;
            let size = u32::from_le_bytes(
                body[size_start..size_start + 4]
                    .try_into()
                    .expect("4 octets"),
            ) as usize;

            let sha256_start = size_start + 4;
            let expected_sha256 = if body.len() >= sha256_start + 32 {
                body[sha256_start..sha256_start + 32].to_vec()
            } else {
                Vec::new()
            };
            if let Some(override_status) = state.send_file_ready_status_override {
                state.log(&format!("envoi refuse (simule) : {name}"));
                out.push(frame(&[opcode, override_status]));
            } else {
                state.log(&format!(
                    "annonce d'envoi : {name} ({size} octets attendus)"
                ));
                state.pending_upload = Some(PendingUpload {
                    name,
                    remaining: size,
                    collected: Vec::new(),
                    expected_sha256,
                });
                out.push(frame(&[opcode, send_file_status::READY]));
            }
        }

        commands::OP_UPDATE_PLAYLIST => {
            if body.len() <= 5 {
                return;
            }
            if state.enforce_firmware_validation && state.update_playlist_status == 0 {
                let file_name = String::from_utf8_lossy(&body[1..body.len() - 4]).into_owned();
                let status = validate_and_apply_playlist(state, &file_name);
                state.log(&format!(
                    "mise a jour de la playlist (validation reelle, {file_name}) -> statut 0x{status:02x} ({})",
                    firmware_error_catalog::update_playlist_status(status)
                ));
                out.push(frame(&[opcode, status]));
            } else {
                let status = state.update_playlist_status;
                state.log(&format!("mise a jour de la playlist -> statut {status}"));
                out.push(frame(&[opcode, status]));
            }
        }

        commands::OP_GET_NUMBER_OF_FILES => {
            let count = state.files.len();
            state.log(&format!("nombre de fichiers demande -> {count}"));
            out.push(frame(&[
                opcode,
                (count & 0xFF) as u8,
                ((count >> 8) & 0xFF) as u8,
            ]));
        }

        commands::OP_GET_FILE_INFORMATION => {
            if body.len() <= 2 {
                return;
            }
            let index = (body[1] as usize) | ((body[2] as usize) << 8);
            let mut sorted_names: Vec<&String> = state.files.keys().collect();
            sorted_names.sort();
            if let Some(name) = sorted_names.get(index) {
                let name_bytes = name.as_bytes();
                let size = state.files[*name].len() as u32;
                let mut response = vec![opcode, 0x00, name_bytes.len() as u8];
                response.extend_from_slice(name_bytes);
                response.extend_from_slice(&size.to_le_bytes());
                out.push(frame(&response));
            } else {
                out.push(frame(&[opcode, 0x02]));
            }
        }

        _ => state.log(&format!("opcode non gere : 0x{opcode:x}")),
    }
}

fn validate_and_apply_playlist(state: &mut DeviceState, file_name: &str) -> u8 {
    if !file_name.ends_with(".json") {
        return 0x02;
    }
    let Some(json_bytes) = state.files.get(file_name) else {
        return 0x03;
    };

    let tree = match json_decoder::decode(json_bytes) {
        Ok(tree) => tree,
        Err(DecodeError::MalformedJson) => return 0x05,
        Err(DecodeError::RootIsNotAnArray) => return 0x07,
        Err(DecodeError::InvalidNode) => return 0x04,
    };
    if tree.is_empty() {
        return 0x08;
    }

    let all_folders = flatten_folders(&tree);
    let favorite_count = all_folders.iter().filter(|f| f.is_favorite).count();
    if favorite_count == 0 {
        return 0x13;
    }
    if favorite_count != 1 {
        return 0x14;
    }

    for folder in &all_folders {
        if !state.files.contains_key(&format!("{}.jpg", folder.uuid)) {
            return 0x11;
        }
    }
    let sound_uuids = flatten_sound_uuids(&tree);
    for uuid in &sound_uuids {
        if !state.files.contains_key(&format!("{uuid}.jpg")) {
            return 0x11;
        }
    }
    for uuid in &sound_uuids {
        if !state.files.contains_key(&format!("{uuid}.mp3"))
            && !state.files.contains_key(&format!("{uuid}.aac"))
        {
            return 0x12;
        }
    }

    let binary = bin_encoder::encode(&tree);
    state.persist("playlist.bin", &binary);
    state.files.insert("playlist.bin".to_string(), binary);
    0x00
}

fn flatten_folders(folders: &[PlaylistFolder]) -> Vec<&PlaylistFolder> {
    let mut result = Vec::new();
    fn visit<'a>(folder: &'a PlaylistFolder, result: &mut Vec<&'a PlaylistFolder>) {
        result.push(folder);
        for child in &folder.children {
            if let PlaylistNode::Folder(sub) = child {
                visit(sub, result);
            }
        }
    }
    for folder in folders {
        visit(folder, &mut result);
    }
    result
}

fn flatten_sound_uuids(folders: &[PlaylistFolder]) -> Vec<String> {
    let mut result = Vec::new();
    fn visit(folder: &PlaylistFolder, result: &mut Vec<String>) {
        for child in &folder.children {
            match child {
                PlaylistNode::Folder(sub) => visit(sub, result),
                PlaylistNode::Sound { uuid, .. } => result.push(uuid.clone()),
            }
        }
    }
    for folder in folders {
        visit(folder, &mut result);
    }
    result
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + body.len());
    data.push(body.len() as u8);
    data.extend_from_slice(body);
    data
}

fn raw_download_response(name: &str, content: &[u8]) -> Vec<u8> {
    let name_bytes = name.as_bytes();
    let mut body = vec![commands::OP_DOWNLOAD_FILE, 0x00, name_bytes.len() as u8];
    body.extend_from_slice(name_bytes);
    body.extend_from_slice(&(content.len() as u32).to_le_bytes());
    body.extend_from_slice(&Sha256::digest(content));
    let crc = crc32_mpeg2::checksum_le(&body);
    body.extend_from_slice(&crc);

    let mut response = vec![body.len() as u8];
    response.extend_from_slice(&body);
    response.extend_from_slice(content);
    response
}
