use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use merlin_domain::library::category_assignment::PodcastCategoryAssignment;
use merlin_domain::library::manual_category::ManualCategory;
use merlin_domain::library::repositories::{SyncStateRepository, group_override_key};
use merlin_domain::library::synced_record::SyncedRecord;

#[derive(Default)]
pub struct FakeSyncStateRepository {
    pub synced: HashMap<String, SyncedRecord>,
    pub episode_numbers: HashMap<String, i64>,
    pub group_titles: HashMap<String, String>,
    pub assignments: HashMap<String, PodcastCategoryAssignment>,
    pub episode_titles: HashMap<String, String>,
    pub folder_images: HashMap<String, String>,
    pub episode_images: HashMap<String, String>,
    pub manual_categories_storage: HashMap<String, ManualCategory>,
    pub image_fingerprints: HashMap<String, String>,
}

impl SyncStateRepository for FakeSyncStateRepository {
    fn synced_records(&self) -> Vec<SyncedRecord> {
        self.synced.values().cloned().collect()
    }

    fn record_synced(
        &mut self,
        episode_uuid: &str,
        title: &str,
        folder_title: &str,
        synced_at: DateTime<Utc>,
    ) {
        self.synced.insert(
            episode_uuid.to_string(),
            SyncedRecord {
                episode_uuid: episode_uuid.to_string(),
                title: title.to_string(),
                folder_title: folder_title.to_string(),
                synced_at,
                pending_deletion: false,
            },
        );
    }

    fn mark_pending_deletion(&mut self, episode_uuid: &str, pending: bool) {
        if let Some(record) = self.synced.get_mut(episode_uuid) {
            record.pending_deletion = pending;
        }
    }

    fn delete_synced_records(&mut self, episode_uuids: &HashSet<String>) {
        for uuid in episode_uuids {
            self.synced.remove(uuid);
        }
    }

    fn episode_number_overrides(&self) -> HashMap<String, i64> {
        self.episode_numbers.clone()
    }

    fn set_episode_number_override(&mut self, episode_guid: &str, number: Option<i64>) {
        match number {
            Some(number) => self
                .episode_numbers
                .insert(episode_guid.to_string(), number),
            None => self.episode_numbers.remove(episode_guid),
        };
    }

    fn group_title_overrides(&self) -> HashMap<String, String> {
        self.group_titles.clone()
    }

    fn set_group_title_override(
        &mut self,
        feed_url: &str,
        group_key: &str,
        custom_title: Option<&str>,
    ) {
        let key = group_override_key(feed_url, group_key);
        match custom_title {
            Some(title) => self.group_titles.insert(key, title.to_string()),
            None => self.group_titles.remove(&key),
        };
    }

    fn category_assignments(&self) -> HashMap<String, PodcastCategoryAssignment> {
        self.assignments.clone()
    }

    fn set_category_assignment(&mut self, assignment: PodcastCategoryAssignment) {
        self.assignments.insert(
            group_override_key(&assignment.feed_url, &assignment.group_key),
            assignment,
        );
    }

    fn remove_category_assignment(&mut self, feed_url: &str, group_key: &str) {
        self.assignments
            .remove(&group_override_key(feed_url, group_key));
    }

    fn episode_title_overrides(&self) -> HashMap<String, String> {
        self.episode_titles.clone()
    }

    fn set_episode_title_override(&mut self, episode_guid: &str, custom_title: Option<&str>) {
        match custom_title {
            Some(title) => self
                .episode_titles
                .insert(episode_guid.to_string(), title.to_string()),
            None => self.episode_titles.remove(episode_guid),
        };
    }

    fn folder_image_overrides(&self) -> HashMap<String, String> {
        self.folder_images.clone()
    }

    fn set_folder_image_override(&mut self, folder_uuid: &str, image_source: Option<&str>) {
        match image_source {
            Some(source) => self
                .folder_images
                .insert(folder_uuid.to_string(), source.to_string()),
            None => self.folder_images.remove(folder_uuid),
        };
    }

    fn episode_image_overrides(&self) -> HashMap<String, String> {
        self.episode_images.clone()
    }

    fn set_episode_image_override(&mut self, episode_guid: &str, image_source: Option<&str>) {
        match image_source {
            Some(source) => self
                .episode_images
                .insert(episode_guid.to_string(), source.to_string()),
            None => self.episode_images.remove(episode_guid),
        };
    }

    fn manual_categories(&self) -> Vec<ManualCategory> {
        self.manual_categories_storage.values().cloned().collect()
    }

    fn add_manual_category(&mut self, category: ManualCategory) {
        self.manual_categories_storage
            .entry(category.uuid.clone())
            .or_insert(category);
    }

    fn remove_manual_category(&mut self, uuid: &str) {
        self.manual_categories_storage.remove(uuid);
    }

    fn uploaded_image_fingerprints(&self) -> HashMap<String, String> {
        self.image_fingerprints.clone()
    }

    fn set_uploaded_image_fingerprint(&mut self, remote_name: &str, fingerprint: &str) {
        self.image_fingerprints
            .insert(remote_name.to_string(), fingerprint.to_string());
    }
}
