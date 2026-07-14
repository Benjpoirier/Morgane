use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use merlin_domain::library::manual_category::ManualCategory;
use merlin_domain::playlist::bin_parser;
use merlin_domain::playlist::builder;
use merlin_domain::playlist::model::{PlaylistFolder, PlaylistNode};
use merlin_domain::playlist::tree_edit::{self, TreeEdit};
use merlin_domain::sync::merge_planner::{self, FolderImageNeeded};
use merlin_domain::sync::types::{EpisodeToSync, SyncError, SyncedEpisode};
use merlin_protocol::client::{DownloadError, Frame, MerlinClient};
use merlin_protocol::commands::{self, send_file_status};
use merlin_protocol::crc32_mpeg2;

use crate::podcasts::image_converter;

use super::error::EngineError;
use super::session::{self, LogFn};

const BUNDLED_CATEGORY_IMAGE: &[u8] = include_bytes!("../../assets/podcast_category.jpg");

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const READY_TIMEOUT: Duration = Duration::from_secs(15);

const PROGRESS_THRESHOLD: usize = 65536;

pub type FileProgressFn<'a> = &'a (dyn Fn(&str, usize, usize) + Send + Sync);

pub struct SyncCallbacks<'a> {
    pub log: LogFn<'a>,
    pub on_progress: Option<&'a (dyn Fn(usize, usize) + Send + Sync)>,
    pub on_episode_uploaded: Option<&'a (dyn Fn(&str) + Send + Sync)>,
    pub on_file_progress: Option<FileProgressFn<'a>>,
}

impl SyncCallbacks<'static> {
    pub fn silent() -> Self {
        Self {
            log: &session::noop_log,
            on_progress: None,
            on_episode_uploaded: None,
            on_file_progress: None,
        }
    }
}

#[derive(Default)]
pub struct SyncRequest {
    pub episodes: Vec<EpisodeToSync>,

    pub files_to_delete: Vec<String>,
    pub tree_edits: Vec<TreeEdit>,
    pub folder_image_overrides: HashMap<String, String>,
    pub manual_categories: Vec<ManualCategory>,

    pub work_dir: PathBuf,

    pub image_fingerprints: HashMap<String, String>,
}

pub struct RepairFile {
    pub local_path: PathBuf,
    pub remote_name: String,
}

pub struct SyncEngine {
    host: String,
    port: u16,
    cancel_token: CancellationToken,
    total_bytes: usize,
    bytes_done: usize,

    uploaded_episode_files_this_attempt: Vec<String>,

    uploaded_image_fingerprints: Vec<(String, String)>,

    has_committed_new_manifest: bool,
}

impl SyncEngine {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            cancel_token: CancellationToken::new(),
            total_bytes: 0,
            bytes_done: 0,
            uploaded_episode_files_this_attempt: Vec::new(),
            uploaded_image_fingerprints: Vec::new(),
            has_committed_new_manifest: false,
        }
    }

    pub fn uploaded_image_fingerprints(&self) -> &[(String, String)] {
        &self.uploaded_image_fingerprints
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub fn set_cancellation_token(&mut self, token: CancellationToken) {
        self.cancel_token = token;
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn bytes_done(&self) -> usize {
        self.bytes_done
    }

    pub async fn sync(
        &mut self,
        request: &SyncRequest,
        callbacks: &SyncCallbacks<'_>,
    ) -> Result<Vec<SyncedEpisode>, EngineError> {
        if request.episodes.is_empty()
            && request.files_to_delete.is_empty()
            && request.tree_edits.is_empty()
            && request.manual_categories.is_empty()
            && request.folder_image_overrides.is_empty()
        {
            return Err(SyncError::NoEpisodes.into());
        }

        self.total_bytes = request
            .episodes
            .iter()
            .filter(|ep| !ep.already_uploaded)
            .map(|ep| {
                file_size(&ep.audio_path) + ep.image_path.as_deref().map(file_size).unwrap_or(0)
            })
            .sum();
        self.bytes_done = 0;
        self.uploaded_episode_files_this_attempt.clear();
        self.uploaded_image_fingerprints.clear();
        self.has_committed_new_manifest = false;

        let mut client = MerlinClient::new(self.host.clone(), self.port);
        let token = self.cancel_token.clone();
        let result = tokio::select! {
            biased;
            _ = token.cancelled() => Err(EngineError::Cancelled),
            result = self.perform_sync(&mut client, request, callbacks) => result,
        };
        match result {
            Ok(synced) => {
                client.close().await;
                Ok(synced)
            }
            Err(EngineError::Cancelled) => {
                client.close().await;
                Err(EngineError::Cancelled)
            }
            Err(error) => {
                self.clean_up_after_failure(&mut client, callbacks.log)
                    .await;
                client.close().await;
                Err(error)
            }
        }
    }

    pub async fn repair_files(
        &mut self,
        files: &[RepairFile],
        callbacks: &SyncCallbacks<'_>,
    ) -> Result<(), EngineError> {
        let mut client = MerlinClient::new(self.host.clone(), self.port);
        client.connect(Duration::from_secs(10));
        let result = async {
            session::handshake(&mut client, Duration::from_secs(10), callbacks.log).await?;
            for file in files {
                self.upload_if_needed(
                    &mut client,
                    &file.remote_name,
                    &file.local_path,
                    callbacks,
                    "",
                )
                .await?;
            }
            Ok(())
        }
        .await;
        client.close().await;
        result
    }

    async fn clean_up_after_failure(&mut self, client: &mut MerlinClient, log: LogFn<'_>) {
        if self.has_committed_new_manifest {
            log(
                "Nettoyage ignore : le nouveau manifeste a deja ete accepte par l'enceinte, le modifier maintenant romprait le contenu actif plutot que de nettoyer une tentative avortee.",
            );
            return;
        }
        if self.uploaded_episode_files_this_attempt.is_empty() {
            return;
        }
        log(&format!(
            "Echec de la synchro : nettoyage de {} fichier(s) deja envoye(s) pour eviter des orphelins...",
            self.uploaded_episode_files_this_attempt.len()
        ));
        let is_delete = |frames: &[Frame]| {
            frames
                .iter()
                .any(|f| f.opcode() == Some(commands::OP_DELETE_FILE))
        };
        for remote_name in &self.uploaded_episode_files_this_attempt {
            match client
                .send_frame_command(
                    &commands::delete_file(remote_name),
                    Duration::from_secs(15),
                    Some(&is_delete),
                )
                .await
            {
                Ok(frames) => {
                    let status = frames
                        .iter()
                        .find(|f| f.opcode() == Some(commands::OP_DELETE_FILE) && f.body.len() > 1)
                        .map(|f| f.body[1]);
                    if status != Some(0) {
                        log(&format!(
                            "Nettoyage de {remote_name} : reponse inattendue (statut {})",
                            status
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "?".to_string())
                        ));
                    }
                }
                Err(error) => log(&format!(
                    "Nettoyage de {remote_name} impossible ({error}) - a retirer manuellement via \"Fichiers retrouves\" si besoin"
                )),
            }
        }
    }

    async fn perform_sync(
        &mut self,
        client: &mut MerlinClient,
        request: &SyncRequest,
        callbacks: &SyncCallbacks<'_>,
    ) -> Result<Vec<SyncedEpisode>, EngineError> {
        let log = callbacks.log;
        client.connect(Duration::from_secs(10));
        session::handshake(client, Duration::from_secs(10), log).await?;

        let mut root_folders: Vec<PlaylistFolder>;
        match client.download_file("playlist.bin", COMMAND_TIMEOUT).await {
            Ok(playlist_bin_data) => {
                root_folders = bin_parser::parse(&playlist_bin_data);
                log(&format!(
                    "playlist.bin recuperee de l'enceinte ({} octets, {} entrees racine)",
                    playlist_bin_data.len(),
                    root_folders.len()
                ));

                if !playlist_bin_data.is_empty()
                    && !playlist_bin_data
                        .len()
                        .is_multiple_of(bin_parser::ACTUAL_RECORD_SIZE)
                {
                    let error = SyncError::CorruptPlaylistBin {
                        byte_count: playlist_bin_data.len(),
                    };
                    log(&format!(
                        "{error} - synchronisation annulee plutot que de risquer d'ecraser le contenu existant."
                    ));
                    return Err(error.into());
                }
            }
            Err(DownloadError::NotFound(_)) => {
                log(
                    "playlist.bin absente de l'enceinte (premiere synchro ou carte SD neuve) - depart d'une playlist vide",
                );
                root_folders = Vec::new();
            }
            Err(error) => {
                log(&format!(
                    "Echec du telechargement de playlist.bin ({error}) - synchronisation annulee plutot que de risquer d'ecraser le contenu existant sans l'avoir reellement lu."
                ));
                return Err(error.into());
            }
        }

        let mut device_sound_uuids: HashSet<String> = HashSet::new();
        collect_sound_uuids(&root_folders, &mut device_sound_uuids);

        for category in &request.manual_categories {
            if !root_folders.iter().any(|f| f.uuid == category.uuid) {
                root_folders.push(PlaylistFolder::new(&category.uuid, &category.title));
            }
        }

        if !request.tree_edits.is_empty() {
            for message in tree_edit::apply(&request.tree_edits, &mut root_folders) {
                log(&message);
            }
        }

        if !request.files_to_delete.is_empty() {
            let deleted_episode_uuids: HashSet<String> = request
                .files_to_delete
                .iter()
                .map(|name| strip_extension(name).to_string())
                .collect();
            for uuid in &deleted_episode_uuids {
                while tree_edit::remove_node(uuid, &mut root_folders).is_some() {}
            }
        }

        let plan = merge_planner::plan(
            &request.episodes,
            root_folders,
            &request.folder_image_overrides,
            &request.manual_categories,
        );
        let root_folders = plan.root_folders;
        for warning in &plan.warnings {
            log(warning);
        }

        for needed in &plan.folder_images_needed {
            if needed.use_bundled_fallback {
                self.upload_bundled_folder_image(client, needed, &request.image_fingerprints, log)
                    .await?;
            } else {
                self.upload_folder_image_if_needed(
                    client,
                    needed,
                    &request.work_dir,
                    &request.image_fingerprints,
                    log,
                )
                .await?;
            }
        }

        for episode in &request.episodes {
            let on_device =
                episode.already_uploaded || device_sound_uuids.contains(&episode.episode_uuid);
            if on_device {
                log(&format!(
                    "{} deja sur l'enceinte, mise a jour du visuel si change",
                    episode.episode_title
                ));
            } else {
                log(&format!("Envoi : {}", episode.episode_title));
                let audio_remote_name = remote_name(&episode.episode_uuid, &episode.audio_path);
                if self
                    .upload_if_needed(
                        client,
                        &audio_remote_name,
                        &episode.audio_path,
                        callbacks,
                        &episode.episode_title,
                    )
                    .await?
                {
                    self.uploaded_episode_files_this_attempt
                        .push(audio_remote_name);
                }
            }

            if let Some(image_path) = &episode.image_path {
                let image_remote_name = remote_name(&episode.episode_uuid, image_path);
                let data = std::fs::read(image_path)?;
                let uploaded = self
                    .upload_image_if_changed(
                        client,
                        &image_remote_name,
                        &data,
                        &request.image_fingerprints,
                        log,
                    )
                    .await?;
                if !on_device {
                    self.bytes_done += data.len();
                    if let Some(on_progress) = callbacks.on_progress {
                        on_progress(self.bytes_done, self.total_bytes);
                    }
                    if uploaded {
                        self.uploaded_episode_files_this_attempt
                            .push(image_remote_name);
                    }
                }
            }

            if !on_device && let Some(on_episode_uploaded) = callbacks.on_episode_uploaded {
                on_episode_uploaded(&episode.episode_uuid);
            }
        }

        let synced = plan.synced;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let playlist_json = builder::build_json(&root_folders, now);

        log("Mise a jour de la playlist...");
        let playlist_bytes = playlist_json.as_bytes();

        let quiet = SyncCallbacks {
            log: callbacks.log,
            on_progress: None,
            on_episode_uploaded: None,
            on_file_progress: None,
        };
        self.upload_file_bytes(client, "playlist.json", playlist_bytes, &quiet, "")
            .await?;

        self.has_committed_new_manifest = true;
        let update_playlist_frames = session::send(
            client,
            "updatePlaylist",
            &commands::update_playlist("playlist.json"),
            COMMAND_TIMEOUT,
        )
        .await?;
        let Some(status) = update_playlist_frames
            .iter()
            .find(|f| f.opcode() == Some(commands::OP_UPDATE_PLAYLIST))
            .and_then(|f| f.body.get(1).copied())
        else {
            return Err(SyncError::UpdatePlaylistNoResponse.into());
        };
        if status != 0 {
            self.has_committed_new_manifest = false;
            return Err(SyncError::UpdatePlaylistRejected(
                commands::update_playlist_status_description(status),
            )
            .into());
        }

        for remote_name in &request.files_to_delete {
            log(&format!("Suppression : {remote_name}"));
            let frames = session::send(
                client,
                "deleteFile",
                &commands::delete_file(remote_name),
                COMMAND_TIMEOUT,
            )
            .await?;
            let status = frames
                .iter()
                .find(|f| f.opcode() == Some(commands::OP_DELETE_FILE) && f.body.len() > 1)
                .map(|f| f.body[1]);
            if status != Some(0) {
                log(&format!(
                    "Suppression de {remote_name} : reponse inattendue (statut {})",
                    status
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "?".to_string())
                ));
            }
        }

        session::send(
            client,
            "setDate",
            &commands::set_date(None),
            COMMAND_TIMEOUT,
        )
        .await?;
        session::send(
            client,
            "setEnablingHours",
            &commands::set_enabling_hours_disabled(),
            COMMAND_TIMEOUT,
        )
        .await?;
        session::send(
            client,
            "endSynchronization",
            &commands::end_synchronization(),
            COMMAND_TIMEOUT,
        )
        .await?;

        Ok(synced)
    }

    async fn upload_folder_image_if_needed(
        &mut self,
        client: &mut MerlinClient,
        needed: &FolderImageNeeded,
        work_dir: &Path,
        known_fingerprints: &HashMap<String, String>,
        log: LogFn<'_>,
    ) -> Result<(), EngineError> {
        let result: Result<(), EngineError> = async {
            let jpg: PathBuf;
            if let Some(image_url) = &needed.image_url {
                jpg = image_converter::download_and_convert_folder_image(
                    image_url,
                    &needed.folder_uuid,
                    work_dir,
                )
                .await
                .map_err(|e| EngineError::Io(e.to_string()))?;
            } else if let Some(fallback) = &needed.fallback_local_image_path {
                log("Pas d'image propre pour ce dossier - reutilisation de l'image d'un episode enfant");
                jpg = fallback.clone();
            } else {

                log("Pas d'image ni de repli pour ce dossier - image generique embarquee");
                return self
                    .upload_image_if_changed(
                        client,
                        &format!("{}.jpg", needed.folder_uuid),
                        BUNDLED_CATEGORY_IMAGE,
                        known_fingerprints,
                        log,
                    )
                    .await
                    .map(|_| ());
            }
            let data = std::fs::read(&jpg)?;
            self.upload_image_if_changed(
                client,
                &format!("{}.jpg", needed.folder_uuid),
                &data,
                known_fingerprints,
                log,
            )
            .await
            .map(|_| ())
        }
        .await;
        result.map_err(|error| {
            SyncError::FolderImageUploadFailed {
                folder_title: needed.folder_title.clone(),
                underlying: error.to_string(),
            }
            .into()
        })
    }

    async fn upload_bundled_folder_image(
        &mut self,
        client: &mut MerlinClient,
        needed: &FolderImageNeeded,
        known_fingerprints: &HashMap<String, String>,
        log: LogFn<'_>,
    ) -> Result<(), EngineError> {
        let remote = format!("{}.jpg", needed.folder_uuid);
        let result = async {
            self.upload_image_if_changed(
                client,
                &remote,
                BUNDLED_CATEGORY_IMAGE,
                known_fingerprints,
                log,
            )
            .await
            .map(|_| ())
        }
        .await;
        result.map_err(|error: EngineError| {
            SyncError::FolderImageUploadFailed {
                folder_title: needed.folder_title.clone(),
                underlying: error.to_string(),
            }
            .into()
        })
    }

    async fn upload_image_if_changed(
        &mut self,
        client: &mut MerlinClient,
        remote_name: &str,
        data: &[u8],
        known_fingerprints: &HashMap<String, String>,
        log: LogFn<'_>,
    ) -> Result<bool, EngineError> {
        let fingerprint = format!("{:08x}", crc32_mpeg2::checksum(data));
        if known_fingerprints.get(remote_name) == Some(&fingerprint) {
            return Ok(false);
        }
        self.upload_file_bytes(
            client,
            remote_name,
            data,
            &SyncCallbacks {
                log,
                on_progress: None,
                on_episode_uploaded: None,
                on_file_progress: None,
            },
            "",
        )
        .await?;
        self.uploaded_image_fingerprints
            .push((remote_name.to_string(), fingerprint));
        Ok(true)
    }

    async fn upload_if_needed(
        &mut self,
        client: &mut MerlinClient,
        remote_name: &str,
        local_path: &Path,
        callbacks: &SyncCallbacks<'_>,
        file_label: &str,
    ) -> Result<bool, EngineError> {
        let data = std::fs::read(local_path)?;

        let did_upload = if self.file_not_found_on_device(client, remote_name).await? {
            self.upload_file_bytes(client, remote_name, &data, callbacks, file_label)
                .await?;
            true
        } else {
            (callbacks.log)(&format!("{remote_name} deja present, transfert saute"));
            false
        };
        self.bytes_done += file_size(local_path);
        if let Some(on_progress) = callbacks.on_progress {
            on_progress(self.bytes_done, self.total_bytes);
        }
        Ok(did_upload)
    }

    async fn file_not_found_on_device(
        &mut self,
        client: &mut MerlinClient,
        remote_name: &str,
    ) -> Result<bool, EngineError> {
        let is_search = |frames: &[Frame]| {
            frames
                .iter()
                .any(|f| f.opcode() == Some(commands::OP_SEARCH_FILE))
        };
        let frames = client
            .send_frame_command(
                &commands::search_file(remote_name),
                COMMAND_TIMEOUT,
                Some(&is_search),
            )
            .await?;
        let Some(frame) = frames
            .iter()
            .find(|f| f.opcode() == Some(commands::OP_SEARCH_FILE))
        else {
            return Err(SyncError::FileSearchFailed(remote_name.to_string()).into());
        };
        Ok(frame.body.len() >= 2 && frame.body[1] == 1)
    }

    async fn upload_file_bytes(
        &mut self,
        client: &mut MerlinClient,
        remote_name: &str,
        data: &[u8],
        callbacks: &SyncCallbacks<'_>,
        file_label: &str,
    ) -> Result<(), EngineError> {
        let announce = commands::send_file_announce(remote_name, data);
        client.send_frame(&announce).await?;
        let is_send_file = |frames: &[Frame]| {
            frames
                .iter()
                .any(|f| f.opcode() == Some(commands::OP_SEND_FILE))
        };
        let ready_frames = client
            .read_frames(READY_TIMEOUT, Some(&is_send_file))
            .await?;
        let Some(ready_frame) = ready_frames
            .iter()
            .find(|f| f.opcode() == Some(commands::OP_SEND_FILE))
        else {
            return Err(SyncError::NoResponse(format!(
                "le signal \"pret\" de sendFile pour {remote_name}"
            ))
            .into());
        };
        let ready_status = if ready_frame.body.len() > 1 {
            ready_frame.body[1]
        } else {
            0xFF
        };
        if ready_status != send_file_status::READY {
            return Err(SyncError::SendFileRejected(
                remote_name.to_string(),
                send_file_status::description(ready_status),
            )
            .into());
        }

        let timeout = Duration::from_secs_f64((data.len() as f64 / 2000.0).clamp(60.0, 1800.0));
        let base_bytes_done = self.bytes_done;
        let current_total_bytes = self.total_bytes;
        let mut last_reported = 0usize;
        let mut progress = |sent: usize, total: usize| {
            if sent - last_reported < PROGRESS_THRESHOLD && sent != total {
                return;
            }
            last_reported = sent;
            if let Some(on_progress) = callbacks.on_progress {
                on_progress(base_bytes_done + sent, current_total_bytes);
            }
            if let Some(on_file_progress) = callbacks.on_file_progress {
                on_file_progress(file_label, sent, total);
            }
        };
        client.send_bulk(data, timeout, Some(&mut progress)).await?;
        let final_frames = client
            .read_frames(READY_TIMEOUT, Some(&is_send_file))
            .await?;
        let Some(final_frame) = final_frames
            .iter()
            .find(|f| f.opcode() == Some(commands::OP_SEND_FILE))
        else {
            return Err(SyncError::NoResponse(format!(
                "la confirmation finale d'upload de {remote_name}"
            ))
            .into());
        };
        let final_status = if final_frame.body.len() > 1 {
            final_frame.body[1]
        } else {
            0xFF
        };
        if final_status != send_file_status::SUCCESS {
            return Err(SyncError::SendFileRejected(
                remote_name.to_string(),
                send_file_status::description(final_status),
            )
            .into());
        }
        Ok(())
    }
}

fn file_size(path: &Path) -> usize {
    std::fs::metadata(path)
        .map(|m| m.len() as usize)
        .unwrap_or(0)
}

fn remote_name(episode_uuid: &str, local_path: &Path) -> String {
    let extension = local_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    format!("{episode_uuid}.{extension}")
}

fn strip_extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(index) => &name[..index],
        None => name,
    }
}

fn collect_sound_uuids(folders: &[PlaylistFolder], out: &mut HashSet<String>) {
    for folder in folders {
        for child in &folder.children {
            match child {
                PlaylistNode::Sound { uuid, .. } => {
                    out.insert(uuid.clone());
                }
                PlaylistNode::Folder(subfolder) => {
                    collect_sound_uuids(std::slice::from_ref(subfolder), out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_sound_uuids_walks_nested_folders() {
        let mut root = PlaylistFolder::new("cat", "Categorie");
        root.children.push(PlaylistNode::Sound {
            uuid: "sound-1".into(),
            title: "A".into(),
        });
        let mut group = PlaylistFolder::new("grp", "Groupe");
        group.children.push(PlaylistNode::Sound {
            uuid: "sound-2".into(),
            title: "B".into(),
        });
        root.children.push(PlaylistNode::Folder(group));

        let mut out = HashSet::new();
        collect_sound_uuids(std::slice::from_ref(&root), &mut out);

        assert_eq!(
            out,
            HashSet::from(["sound-1".to_string(), "sound-2".to_string()])
        );
    }
}
