use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::Utc;
use tokio_util::sync::CancellationToken;

use merlin_domain::library::deterministic_uuid;
use merlin_domain::library::repositories::{SyncStateRepository, group_override_key};
use merlin_domain::library::subscription::Subscription;
use merlin_domain::playlist::model::PlaylistFolder;
use merlin_domain::playlist::tree_edit::TreeEdit;
use merlin_domain::podcasts::episode::{Episode, episode_numbering};
use merlin_domain::sync::types::EpisodeToSync;
use merlin_infra::podcasts::audio_converter::{self, ConversionStep};
use merlin_infra::podcasts::image_converter::{self, ConversionError};
use merlin_infra::sync::engine::{SyncCallbacks, SyncEngine, SyncRequest};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SyncProgressPhase {
    Preparing {
        done: usize,
        total: usize,
    },
    Connecting,
    Sending {
        bytes_done: usize,
        bytes_total: usize,
    },
    Finished {
        count: usize,
    },
    Failed(String),
}

pub fn conversion_step_label(step: ConversionStep) -> &'static str {
    match step {
        ConversionStep::DownloadingAudio => "Telechargement de l'audio",
        ConversionStep::ConvertingAudio => "Conversion audio (ffmpeg)",
        ConversionStep::DownloadingImage => "Telechargement de l'image",
        ConversionStep::ConvertingImage => "Conversion de l'image",
    }
}

pub struct GroupInfo {
    pub uuid: String,
    pub title: String,
    pub group_key: String,
}

pub fn group_info(
    subscription: &Subscription,
    episode_title: &str,
    overrides: &HashMap<String, String>,
) -> Option<GroupInfo> {
    let group_key = episode_numbering::guess_group_title(episode_title)?;
    let display_title = overrides
        .get(&group_override_key(&subscription.feed_url, &group_key))
        .cloned()
        .unwrap_or_else(|| group_key.clone());
    let uuid = deterministic_uuid::from_prefixed_name(&format!(
        "merlinsync-group:{}:{group_key}",
        subscription.feed_url
    ));
    Some(GroupInfo {
        uuid,
        title: display_title,
        group_key,
    })
}

pub struct RunCallbacks<'a> {
    pub on_phase: &'a (dyn Fn(SyncProgressPhase) + Send + Sync),
    pub on_log: &'a (dyn Fn(&str) + Send + Sync),
    pub on_current_step: &'a (dyn Fn(Option<&str>, f64) + Send + Sync),
    pub on_episode_uploaded: &'a (dyn Fn(&str) + Send + Sync),

    pub deletions_completed: &'a (dyn Fn(&[String]) + Send + Sync),
    pub tree_edits_applied: &'a (dyn Fn() + Send + Sync),
}

pub struct PrepareCallbacks<'a> {
    pub on_log: &'a (dyn Fn(&str) + Send + Sync),

    pub on_progress: &'a (dyn Fn(usize, usize) + Send + Sync),

    pub on_episode_ready: &'a (dyn Fn(&str) + Send + Sync),

    pub on_episode_failed: &'a (dyn Fn(&str, &str) + Send + Sync),

    pub on_episode_progress: &'a (dyn Fn(&str, f64) + Send + Sync),
}

fn noop_str(_: &str) {}
fn noop_phase(_: SyncProgressPhase) {}
fn noop_step(_: Option<&str>, _: f64) {}
fn noop_deletions(_: &[String]) {}
fn noop_unit() {}

impl RunCallbacks<'static> {
    pub fn silent() -> Self {
        Self {
            on_phase: &noop_phase,
            on_log: &noop_str,
            on_current_step: &noop_step,
            on_episode_uploaded: &noop_str,
            deletions_completed: &noop_deletions,
            tree_edits_applied: &noop_unit,
        }
    }
}

pub struct SyncEpisodesUseCase<R: SyncStateRepository> {
    sync_state_repository: R,
    work_dir: PathBuf,
    cancel_token: CancellationToken,
}

impl<R: SyncStateRepository> SyncEpisodesUseCase<R> {
    pub fn new(sync_state_repository: R, work_dir: PathBuf) -> Self {
        Self {
            sync_state_repository,
            work_dir,
            cancel_token: CancellationToken::new(),
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub fn use_cancellation_token(&mut self, token: CancellationToken) {
        self.cancel_token = token;
    }

    pub fn repository(&self) -> &R {
        &self.sync_state_repository
    }

    fn folder_uuid_of(
        &self,
        subscription: &Subscription,
        raw_title: &str,
        group_title_overrides: &HashMap<String, String>,
    ) -> String {
        group_info(subscription, raw_title, group_title_overrides)
            .map(|g| g.uuid)
            .unwrap_or_else(|| {
                deterministic_uuid::from_prefixed_name(&format!(
                    "merlinsync-podcast:{}",
                    subscription.feed_url
                ))
            })
    }

    pub async fn prepare(
        &self,
        pairs: &[(Subscription, Episode)],
        callbacks: &PrepareCallbacks<'_>,
    ) {
        let is_cancelled = || self.cancel_token.is_cancelled();
        let group_title_overrides = self.sync_state_repository.group_title_overrides();
        let episode_number_overrides = self.sync_state_repository.episode_number_overrides();
        let episode_title_overrides = self.sync_state_repository.episode_title_overrides();
        let episode_image_overrides = self.sync_state_repository.episode_image_overrides();

        let total = pairs.len();
        for (index, (subscription, raw_episode)) in pairs.iter().enumerate() {
            if is_cancelled() {
                return;
            }
            let title = episode_title_overrides
                .get(&raw_episode.guid)
                .cloned()
                .unwrap_or_else(|| raw_episode.title.clone());
            let episode_number = episode_numbering::guess_number(&title)
                .or_else(|| episode_number_overrides.get(&raw_episode.guid).copied());
            (callbacks.on_log)(&format!("Preparation : {title}"));
            let log_indented = |message: &str| (callbacks.on_log)(&format!("  {message}"));
            let progress = |_step: ConversionStep, fraction: f64| {
                (callbacks.on_episode_progress)(&raw_episode.guid, fraction);
            };

            let has_override = episode_image_overrides
                .get(&raw_episode.guid)
                .is_some_and(|s| !s.is_empty());
            let own_image = if has_override {
                None
            } else {
                raw_episode.image_url.as_deref()
            };
            let prepared = tokio::select! {
                biased;
                _ = self.cancel_token.cancelled() => return,
                prepared = audio_converter::download_and_convert(
                    &raw_episode.audio_url,
                    own_image,
                    &raw_episode.guid,
                    &self.work_dir,
                    episode_number,
                    &log_indented,
                    Some(&progress),
                ) => prepared,
            };
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(error) => {
                    (callbacks.on_episode_failed)(&raw_episode.guid, &error.to_string());
                    (callbacks.on_progress)(index + 1, total);
                    continue;
                }
            };

            let folder_image_url = subscription
                .feed_image_url
                .clone()
                .filter(|s| !s.is_empty());
            match self
                .resolve_episode_image(
                    prepared.image.clone(),
                    &raw_episode.guid,
                    &prepared.uuid,
                    folder_image_url.as_deref(),
                    &log_indented,
                )
                .await
            {
                Ok(_) => (callbacks.on_episode_ready)(&raw_episode.guid),
                Err(error) => (callbacks.on_episode_failed)(&raw_episode.guid, &error.to_string()),
            }
            (callbacks.on_progress)(index + 1, total);
        }

        if is_cancelled() {
            return;
        }
        let mut folder_images: HashMap<String, String> = HashMap::new();
        for (subscription, raw_episode) in pairs {
            if let Some(url) = subscription
                .feed_image_url
                .clone()
                .filter(|s| !s.is_empty())
            {
                let uuid =
                    self.folder_uuid_of(subscription, &raw_episode.title, &group_title_overrides);
                folder_images.entry(uuid).or_insert(url);
            }
        }
        for category in self.sync_state_repository.manual_categories() {
            if !category.image_source.is_empty() {
                folder_images
                    .entry(category.uuid)
                    .or_insert(category.image_source);
            }
        }
        for (uuid, source) in self.sync_state_repository.folder_image_overrides() {
            folder_images.insert(uuid, source);
        }
        for (uuid, source) in folder_images {
            if is_cancelled() {
                return;
            }
            if let Err(error) =
                image_converter::download_and_convert_folder_image(&source, &uuid, &self.work_dir)
                    .await
            {
                (callbacks.on_log)(&format!(
                    "  visuel de dossier non prepare ({uuid}) : {error}"
                ));
            }
        }
    }

    pub fn validate_title_lengths(
        &self,
        pairs: &[(Subscription, Episode)],
        episode_title_overrides: &HashMap<String, String>,
    ) -> Vec<String> {
        let group_title_overrides = self.sync_state_repository.group_title_overrides();
        let category_assignments = self.sync_state_repository.category_assignments();
        let mut seen: HashSet<String> = HashSet::new();
        let mut overlong: Vec<String> = Vec::new();
        let mut check = |title: &str| {
            if title.len() > PlaylistFolder::MAX_TITLE_UTF8_BYTES && seen.insert(title.to_string())
            {
                overlong.push(title.to_string());
            }
        };
        for (subscription, raw_episode) in pairs {
            let episode_title = episode_title_overrides
                .get(&raw_episode.guid)
                .unwrap_or(&raw_episode.title);
            check(episode_title);

            let group = group_info(subscription, &raw_episode.title, &group_title_overrides);
            let folder_title = group.as_ref().map(|g| g.title.clone()).unwrap_or_else(|| {
                if subscription.title.is_empty() {
                    subscription.feed_url.clone()
                } else {
                    subscription.title.clone()
                }
            });
            check(&folder_title);
            let assignment_key = group_override_key(
                &subscription.feed_url,
                group.as_ref().map(|g| g.group_key.as_str()).unwrap_or(""),
            );
            if let Some(assignment) = category_assignments.get(&assignment_key) {
                check(&assignment.target_category_title);
            }
        }
        overlong
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &mut self,
        pairs: &[(Subscription, Episode)],
        host: &str,
        port: u16,
        already_synced_episode_uuids: &HashSet<String>,
        files_to_delete: &HashMap<String, Vec<String>>,
        tree_edits: Vec<TreeEdit>,
        callbacks: &RunCallbacks<'_>,
    ) {
        let on_phase = callbacks.on_phase;
        let on_log = callbacks.on_log;
        let on_current_step = callbacks.on_current_step;
        let is_cancelled = || self.cancel_token.is_cancelled();

        let folder_image_overrides = self.sync_state_repository.folder_image_overrides();
        if pairs.is_empty()
            && files_to_delete.is_empty()
            && tree_edits.is_empty()
            && folder_image_overrides.is_empty()
        {
            on_phase(SyncProgressPhase::Failed(
                "aucun changement a synchroniser".to_string(),
            ));
            return;
        }

        let episode_title_overrides = self.sync_state_repository.episode_title_overrides();
        let group_title_overrides = self.sync_state_repository.group_title_overrides();
        let category_assignments = self.sync_state_repository.category_assignments();
        let episode_number_overrides = self.sync_state_repository.episode_number_overrides();
        let episode_image_overrides = self.sync_state_repository.episode_image_overrides();

        let overlong_titles = self.validate_title_lengths(pairs, &episode_title_overrides);
        if !overlong_titles.is_empty() {
            let list = overlong_titles
                .iter()
                .map(|t| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(", ");
            on_phase(SyncProgressPhase::Failed(format!(
                "Titre(s) trop long(s) pour l'enceinte (30 caracteres max) : {list}. Raccourcis-le(s) avant de synchroniser."
            )));
            return;
        }

        let mut to_sync: Vec<EpisodeToSync> = Vec::new();
        on_phase(SyncProgressPhase::Preparing {
            done: 0,
            total: pairs.len(),
        });
        for (index, (subscription, raw_episode)) in pairs.iter().enumerate() {
            if is_cancelled() {
                return;
            }
            let episode = match episode_title_overrides.get(&raw_episode.guid) {
                Some(custom_title) => Episode {
                    title: custom_title.clone(),
                    ..raw_episode.clone()
                },
                None => raw_episode.clone(),
            };
            let uuid = audio_converter::episode_uuid(&episode.guid);

            if files_to_delete.contains_key(&uuid) {
                on_phase(SyncProgressPhase::Preparing {
                    done: index + 1,
                    total: pairs.len(),
                });
                continue;
            }

            let group = group_info(subscription, &raw_episode.title, &group_title_overrides);
            let folder_uuid = group.as_ref().map(|g| g.uuid.clone()).unwrap_or_else(|| {
                deterministic_uuid::from_prefixed_name(&format!(
                    "merlinsync-podcast:{}",
                    subscription.feed_url
                ))
            });
            let folder_title = group.as_ref().map(|g| g.title.clone()).unwrap_or_else(|| {
                if subscription.title.is_empty() {
                    subscription.feed_url.clone()
                } else {
                    subscription.title.clone()
                }
            });

            let assignment_key = group_override_key(
                &subscription.feed_url,
                group.as_ref().map(|g| g.group_key.as_str()).unwrap_or(""),
            );
            let category_assignment = category_assignments.get(&assignment_key);
            let category_title = category_assignment
                .map(|a| a.target_category_title.clone())
                .unwrap_or_default();
            let category_uuid = category_assignment.map(|a| a.target_category_uuid.clone());

            if category_assignment.is_none() && !already_synced_episode_uuids.contains(&uuid) {
                on_log(&format!(
                    "\"{}\" pas envoye : glisser son groupe vers une categorie dans \"Sur l'enceinte\" avant de synchroniser.",
                    episode.title
                ));
                on_phase(SyncProgressPhase::Preparing {
                    done: index + 1,
                    total: pairs.len(),
                });
                continue;
            }
            let episode_number = episode_numbering::guess_number(&episode.title)
                .or_else(|| episode_number_overrides.get(&episode.guid).copied());
            let folder_image_url = subscription
                .feed_image_url
                .clone()
                .filter(|s| !s.is_empty());
            if already_synced_episode_uuids.contains(&uuid) {
                to_sync.push(EpisodeToSync {
                    folder_uuid,
                    folder_title,
                    episode_uuid: uuid,
                    episode_title: episode.title.clone(),

                    audio_path: self.work_dir.clone(),
                    image_path: None,
                    category_title,
                    category_uuid,
                    folder_image_url,
                    already_uploaded: true,
                    order: episode_number,
                });
                on_phase(SyncProgressPhase::Preparing {
                    done: index + 1,
                    total: pairs.len(),
                });
                continue;
            }
            on_log(&format!("Preparation : {}", episode.title));
            let log_indented = |message: &str| on_log(&format!("  {message}"));
            let progress = |step: ConversionStep, fraction: f64| {
                on_current_step(Some(conversion_step_label(step)), fraction);
            };

            let own_image = if episode_image_overrides
                .get(&episode.guid)
                .is_some_and(|s| !s.is_empty())
            {
                None
            } else {
                episode.image_url.as_deref()
            };
            let prepared = tokio::select! {
                biased;
                _ = self.cancel_token.cancelled() => return,
                prepared = audio_converter::download_and_convert(
                    &episode.audio_url,
                    own_image,
                    &episode.guid,
                    &self.work_dir,
                    episode_number,
                    &log_indented,
                    Some(&progress),
                ) => prepared,
            };
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(error) => {
                    if is_cancelled() {
                        return;
                    }
                    on_phase(SyncProgressPhase::Failed(format!(
                        "echec de preparation de \"{}\" : {error}",
                        episode.title
                    )));
                    return;
                }
            };
            let resolved_image = match self
                .resolve_episode_image(
                    prepared.image.clone(),
                    &episode.guid,
                    &prepared.uuid,
                    folder_image_url.as_deref(),
                    on_log,
                )
                .await
            {
                Ok(image) => image,
                Err(error) => {
                    on_phase(SyncProgressPhase::Failed(format!(
                        "echec de preparation de \"{}\" : {error}",
                        episode.title
                    )));
                    return;
                }
            };
            to_sync.push(EpisodeToSync {
                folder_uuid,
                folder_title,
                episode_uuid: prepared.uuid,
                episode_title: episode.title.clone(),
                audio_path: prepared.audio,
                image_path: resolved_image,
                category_title,
                category_uuid,
                folder_image_url,
                already_uploaded: false,
                order: episode_number,
            });
            on_phase(SyncProgressPhase::Preparing {
                done: index + 1,
                total: pairs.len(),
            });
        }
        if is_cancelled() {
            return;
        }

        let mut folder_rank: HashMap<String, usize> = HashMap::new();
        for episode in &to_sync {
            let next_rank = folder_rank.len();
            folder_rank
                .entry(episode.folder_uuid.clone())
                .or_insert(next_rank);
        }
        let mut indexed: Vec<(usize, EpisodeToSync)> = to_sync.into_iter().enumerate().collect();
        indexed.sort_by_key(|(index, episode)| {
            (
                folder_rank[&episode.folder_uuid],
                episode.order.is_none(),
                episode.order.unwrap_or(0),
                *index,
            )
        });
        let to_sync: Vec<EpisodeToSync> = indexed.into_iter().map(|(_, episode)| episode).collect();

        on_current_step(None, 0.0);
        on_phase(SyncProgressPhase::Connecting);
        let mut engine = SyncEngine::new(host, port);
        engine.set_cancellation_token(self.cancel_token.clone());

        let request = SyncRequest {
            episodes: to_sync,
            files_to_delete: files_to_delete.values().flatten().cloned().collect(),
            tree_edits,
            folder_image_overrides,
            manual_categories: self.sync_state_repository.manual_categories(),
            image_fingerprints: self.sync_state_repository.uploaded_image_fingerprints(),
            work_dir: self.work_dir.clone(),
        };
        let tree_edits_provided = !request.tree_edits.is_empty();

        let engine_callbacks = SyncCallbacks {
            log: on_log,
            on_progress: Some(&|done, total| {
                on_phase(SyncProgressPhase::Sending {
                    bytes_done: done,
                    bytes_total: total,
                })
            }),
            on_episode_uploaded: Some(callbacks.on_episode_uploaded),
            on_file_progress: Some(&|label, sent, total| {
                let fraction = if total > 0 {
                    sent as f64 / total as f64
                } else {
                    0.0
                };
                on_current_step(Some(label), fraction);
            }),
        };
        match engine.sync(&request, &engine_callbacks).await {
            Ok(synced) => {
                let now = Utc::now();
                for record in &synced {
                    self.sync_state_repository.record_synced(
                        &record.episode_uuid,
                        &record.title,
                        &record.folder_title,
                        now,
                    );
                }

                for (remote_name, fingerprint) in engine.uploaded_image_fingerprints() {
                    self.sync_state_repository
                        .set_uploaded_image_fingerprint(remote_name, fingerprint);
                }
                if !files_to_delete.is_empty() {
                    let deleted_uuids: HashSet<String> = files_to_delete.keys().cloned().collect();
                    self.sync_state_repository
                        .delete_synced_records(&deleted_uuids);
                    let mut deleted: Vec<String> = deleted_uuids.into_iter().collect();
                    deleted.sort();
                    (callbacks.deletions_completed)(&deleted);
                }
                if tree_edits_provided {
                    (callbacks.tree_edits_applied)();
                }
                on_phase(SyncProgressPhase::Finished {
                    count: synced.len(),
                });
            }
            Err(error) => {
                if is_cancelled() {
                    return;
                }
                on_phase(SyncProgressPhase::Failed(error.to_string()));
            }
        }
    }

    async fn resolve_episode_image(
        &self,
        existing: Option<PathBuf>,
        episode_guid: &str,
        episode_uuid: &str,
        folder_image_url: Option<&str>,
        on_log: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<Option<PathBuf>, ConversionError> {
        if let Some(override_source) = self
            .sync_state_repository
            .episode_image_overrides()
            .get(episode_guid)
            .filter(|s| !s.is_empty())
        {
            return image_converter::download_and_convert_folder_image(
                override_source,
                episode_uuid,
                &self.work_dir,
            )
            .await
            .map(Some);
        }
        if let Some(existing) = existing {
            return Ok(Some(existing));
        }
        let Some(folder_image_url) = folder_image_url else {
            return Ok(None);
        };
        on_log("  Pas d'image propre pour cet episode - reutilisation de l'image du podcast");
        image_converter::download_and_convert_folder_image(
            folder_image_url,
            episode_uuid,
            &self.work_dir,
        )
        .await
        .map(Some)
    }
}
