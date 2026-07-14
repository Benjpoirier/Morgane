use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::category_assignment::PodcastCategoryAssignment;
use super::manual_category::ManualCategory;
use super::subscription::Subscription;
use super::synced_record::SyncedRecord;

pub fn group_override_key(feed_url: &str, group_key: &str) -> String {
    format!("{feed_url}|{group_key}")
}

pub trait SubscriptionRepository {
    fn all(&self) -> Vec<Subscription>;
    fn add(&mut self, subscription: Subscription);
    fn delete(&mut self, feed_url: &str);

    fn update_feed_metadata(
        &mut self,
        feed_url: &str,
        title: Option<&str>,
        feed_image_url: Option<&str>,
    );

    fn update_selected_episode_guids(&mut self, feed_url: &str, guids: &[String]);
}

pub trait SyncStateRepository {
    fn synced_records(&self) -> Vec<SyncedRecord>;

    fn record_synced(
        &mut self,
        episode_uuid: &str,
        title: &str,
        folder_title: &str,
        synced_at: DateTime<Utc>,
    );
    fn mark_pending_deletion(&mut self, episode_uuid: &str, pending: bool);
    fn delete_synced_records(&mut self, episode_uuids: &std::collections::HashSet<String>);

    fn episode_number_overrides(&self) -> HashMap<String, i64>;

    fn set_episode_number_override(&mut self, episode_guid: &str, number: Option<i64>);

    fn group_title_overrides(&self) -> HashMap<String, String>;

    fn set_group_title_override(
        &mut self,
        feed_url: &str,
        group_key: &str,
        custom_title: Option<&str>,
    );

    fn category_assignments(&self) -> HashMap<String, PodcastCategoryAssignment>;
    fn set_category_assignment(&mut self, assignment: PodcastCategoryAssignment);
    fn remove_category_assignment(&mut self, feed_url: &str, group_key: &str);

    fn episode_title_overrides(&self) -> HashMap<String, String>;

    fn set_episode_title_override(&mut self, episode_guid: &str, custom_title: Option<&str>);

    fn folder_image_overrides(&self) -> HashMap<String, String>;

    fn set_folder_image_override(&mut self, folder_uuid: &str, image_source: Option<&str>);

    fn episode_image_overrides(&self) -> HashMap<String, String>;
    fn set_episode_image_override(&mut self, episode_guid: &str, image_source: Option<&str>);

    fn manual_categories(&self) -> Vec<ManualCategory>;
    fn add_manual_category(&mut self, category: ManualCategory);
    fn remove_manual_category(&mut self, uuid: &str);

    fn uploaded_image_fingerprints(&self) -> HashMap<String, String>;
    fn set_uploaded_image_fingerprint(&mut self, remote_name: &str, fingerprint: &str);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_override_key_uses_pipe_separator() {
        assert_eq!(
            group_override_key("https://feed", "Chapitre"),
            "https://feed|Chapitre"
        );
        assert_eq!(group_override_key("https://feed", ""), "https://feed|");
    }
}
