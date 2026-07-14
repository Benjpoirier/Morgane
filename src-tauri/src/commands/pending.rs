use std::collections::{HashMap, HashSet};

use tauri::State;

use merlin_application::sync_episodes_use_case::group_info;
use merlin_domain::library::deterministic_uuid;
use merlin_domain::library::repositories::SyncStateRepository;
use merlin_domain::podcasts::episode::{Episode, episode_numbering};
use merlin_infra::persistence::db;
use merlin_infra::persistence::sync_state_repository::SqliteSyncStateRepository;
use merlin_infra::podcasts::audio_converter;

use crate::dto::{PendingGroup, SelectedPair};
use crate::state::AppState;

#[tauri::command]
pub fn guess_numbers(titles: Vec<String>) -> Vec<Option<i64>> {
    titles
        .iter()
        .map(|t| episode_numbering::guess_number(t))
        .collect()
}

#[tauri::command]
pub fn episode_uuids(guids: Vec<String>) -> Vec<String> {
    guids
        .iter()
        .map(|g| audio_converter::episode_uuid(g))
        .collect()
}

#[tauri::command]
pub fn compute_pending_groups(
    pairs: Vec<SelectedPair>,
    already_synced: Vec<String>,
    state: State<AppState>,
) -> Result<Vec<PendingGroup>, String> {
    let repo = SqliteSyncStateRepository::new(
        db::open(&state.db_path).map_err(|e| e.to_string())?,
        state.read_device_id(),
    );
    let group_title_overrides = repo.group_title_overrides();
    let episode_title_overrides = repo.episode_title_overrides();
    let already: HashSet<String> = already_synced.into_iter().collect();

    let mut order: Vec<String> = Vec::new();
    let mut episodes_by_uuid: HashMap<String, Vec<Episode>> = HashMap::new();

    let mut info_by_uuid: HashMap<String, (String, String, String, Option<String>)> =
        HashMap::new();

    for pair in &pairs {
        let subscription = &pair.subscription;
        let raw_episode = &pair.episode;
        let episode_uuid = audio_converter::episode_uuid(&raw_episode.guid);
        if already.contains(&episode_uuid) {
            continue;
        }

        let group = group_info(subscription, &raw_episode.title, &group_title_overrides);
        let episode = match episode_title_overrides.get(&raw_episode.guid) {
            Some(custom) => Episode {
                title: custom.clone(),
                ..raw_episode.clone()
            },
            None => raw_episode.clone(),
        };
        let uuid = group.as_ref().map(|g| g.uuid.clone()).unwrap_or_else(|| {
            deterministic_uuid::from_prefixed_name(&format!(
                "merlinsync-podcast:{}",
                subscription.feed_url
            ))
        });
        if !episodes_by_uuid.contains_key(&uuid) {
            order.push(uuid.clone());
            let title = group.as_ref().map(|g| g.title.clone()).unwrap_or_else(|| {
                if subscription.title.is_empty() {
                    subscription.feed_url.clone()
                } else {
                    subscription.title.clone()
                }
            });
            info_by_uuid.insert(
                uuid.clone(),
                (
                    subscription.feed_url.clone(),
                    group
                        .as_ref()
                        .map(|g| g.group_key.clone())
                        .unwrap_or_default(),
                    title,
                    subscription.feed_image_url.clone(),
                ),
            );
        }
        episodes_by_uuid.entry(uuid).or_default().push(episode);
    }

    Ok(order
        .into_iter()
        .filter_map(|uuid| {
            let (feed_url, group_key, title, feed_image_url) = info_by_uuid.remove(&uuid)?;
            let episodes = episodes_by_uuid.remove(&uuid).unwrap_or_default();
            Some(PendingGroup {
                feed_url,
                group_key,
                uuid,
                title,
                feed_image_url,
                episodes,
            })
        })
        .collect())
}
