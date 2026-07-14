use std::collections::{HashMap, HashSet};

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, params};

use merlin_domain::library::category_assignment::PodcastCategoryAssignment;
use merlin_domain::library::manual_category::ManualCategory;
use merlin_domain::library::repositories::{SyncStateRepository, group_override_key};
use merlin_domain::library::synced_record::SyncedRecord;

pub struct SqliteSyncStateRepository {
    connection: Connection,

    device_id: String,
}

impl SqliteSyncStateRepository {
    pub fn new(connection: Connection, device_id: impl Into<String>) -> Self {
        Self {
            connection,
            device_id: device_id.into(),
        }
    }

    pub fn orphan_record_count(&self) -> usize {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM synced_records WHERE device_id = ''",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count as usize)
            .unwrap_or(0)
    }

    fn string_map(&self, query: &str) -> HashMap<String, String> {
        let Ok(mut statement) = self.connection.prepare(query) else {
            return HashMap::new();
        };
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }
}

impl SyncStateRepository for SqliteSyncStateRepository {
    fn synced_records(&self) -> Vec<SyncedRecord> {
        let Ok(mut statement) = self.connection.prepare(
            "SELECT episode_uuid, title, folder_title, synced_at, pending_deletion
             FROM synced_records WHERE device_id = ?1",
        ) else {
            return Vec::new();
        };
        statement
            .query_map(params![self.device_id], |row| {
                Ok(SyncedRecord {
                    episode_uuid: row.get(0)?,
                    title: row.get(1)?,
                    folder_title: row.get(2)?,
                    synced_at: Utc
                        .timestamp_opt(row.get::<_, i64>(3)?, 0)
                        .single()
                        .unwrap_or_else(Utc::now),
                    pending_deletion: row.get::<_, i64>(4)? != 0,
                })
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn record_synced(
        &mut self,
        episode_uuid: &str,
        title: &str,
        folder_title: &str,
        synced_at: DateTime<Utc>,
    ) {
        let _ = self.connection.execute(
            "INSERT INTO synced_records (device_id, episode_uuid, title, folder_title, synced_at, pending_deletion)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)
             ON CONFLICT(device_id, episode_uuid)
             DO UPDATE SET title = ?3, folder_title = ?4, synced_at = ?5",
            params![self.device_id, episode_uuid, title, folder_title, synced_at.timestamp()],
        );
    }

    fn mark_pending_deletion(&mut self, episode_uuid: &str, pending: bool) {
        let _ = self.connection.execute(
            "UPDATE synced_records SET pending_deletion = ?3 WHERE device_id = ?1 AND episode_uuid = ?2",
            params![self.device_id, episode_uuid, pending as i64],
        );
    }

    fn delete_synced_records(&mut self, episode_uuids: &HashSet<String>) {
        for uuid in episode_uuids {
            let _ = self.connection.execute(
                "DELETE FROM synced_records WHERE device_id = ?1 AND episode_uuid = ?2",
                params![self.device_id, uuid],
            );
        }
    }

    fn episode_number_overrides(&self) -> HashMap<String, i64> {
        let Ok(mut statement) = self
            .connection
            .prepare("SELECT episode_guid, number FROM episode_number_overrides")
        else {
            return HashMap::new();
        };
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn set_episode_number_override(&mut self, episode_guid: &str, number: Option<i64>) {
        match number {
            None => {
                let _ = self.connection.execute(
                    "DELETE FROM episode_number_overrides WHERE episode_guid = ?1",
                    params![episode_guid],
                );
            }
            Some(number) => {
                let _ = self.connection.execute(
                    "INSERT OR REPLACE INTO episode_number_overrides (episode_guid, number) VALUES (?1, ?2)",
                    params![episode_guid, number],
                );
            }
        }
    }

    fn group_title_overrides(&self) -> HashMap<String, String> {
        let Ok(mut statement) = self
            .connection
            .prepare("SELECT feed_url, group_key, custom_title FROM group_title_overrides")
        else {
            return HashMap::new();
        };
        statement
            .query_map([], |row| {
                Ok((
                    group_override_key(&row.get::<_, String>(0)?, &row.get::<_, String>(1)?),
                    row.get::<_, String>(2)?,
                ))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn set_group_title_override(
        &mut self,
        feed_url: &str,
        group_key: &str,
        custom_title: Option<&str>,
    ) {
        let _ = match custom_title {
            Some(title) => self.connection.execute(
                "INSERT OR REPLACE INTO group_title_overrides (feed_url, group_key, custom_title) VALUES (?1, ?2, ?3)",
                params![feed_url, group_key, title],
            ),
            None => self.connection.execute(
                "DELETE FROM group_title_overrides WHERE feed_url = ?1 AND group_key = ?2",
                params![feed_url, group_key],
            ),
        };
    }

    fn category_assignments(&self) -> HashMap<String, PodcastCategoryAssignment> {
        let Ok(mut statement) = self.connection.prepare(
            "SELECT feed_url, group_key, target_category_uuid, target_category_title FROM category_assignments",
        ) else {
            return HashMap::new();
        };
        statement
            .query_map([], |row| {
                let assignment = PodcastCategoryAssignment {
                    feed_url: row.get(0)?,
                    group_key: row.get(1)?,
                    target_category_uuid: row.get(2)?,
                    target_category_title: row.get(3)?,
                };
                Ok((
                    group_override_key(&assignment.feed_url, &assignment.group_key),
                    assignment,
                ))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn set_category_assignment(&mut self, assignment: PodcastCategoryAssignment) {
        let _ = self.connection.execute(
            "INSERT OR REPLACE INTO category_assignments
             (feed_url, group_key, target_category_uuid, target_category_title)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                assignment.feed_url,
                assignment.group_key,
                assignment.target_category_uuid,
                assignment.target_category_title,
            ],
        );
    }

    fn remove_category_assignment(&mut self, feed_url: &str, group_key: &str) {
        let _ = self.connection.execute(
            "DELETE FROM category_assignments WHERE feed_url = ?1 AND group_key = ?2",
            params![feed_url, group_key],
        );
    }

    fn episode_title_overrides(&self) -> HashMap<String, String> {
        self.string_map("SELECT episode_guid, custom_title FROM episode_title_overrides")
    }

    fn set_episode_title_override(&mut self, episode_guid: &str, custom_title: Option<&str>) {
        let _ = match custom_title {
            Some(title) => self.connection.execute(
                "INSERT OR REPLACE INTO episode_title_overrides (episode_guid, custom_title) VALUES (?1, ?2)",
                params![episode_guid, title],
            ),
            None => self.connection.execute(
                "DELETE FROM episode_title_overrides WHERE episode_guid = ?1",
                params![episode_guid],
            ),
        };
    }

    fn folder_image_overrides(&self) -> HashMap<String, String> {
        self.string_map("SELECT folder_uuid, image_source FROM folder_image_overrides")
    }

    fn set_folder_image_override(&mut self, folder_uuid: &str, image_source: Option<&str>) {
        match image_source {
            None => {
                let _ = self.connection.execute(
                    "DELETE FROM folder_image_overrides WHERE folder_uuid = ?1",
                    params![folder_uuid],
                );
            }
            Some(source) => {
                let _ = self.connection.execute(
                    "INSERT OR REPLACE INTO folder_image_overrides (folder_uuid, image_source) VALUES (?1, ?2)",
                    params![folder_uuid, source],
                );
            }
        }
    }

    fn episode_image_overrides(&self) -> HashMap<String, String> {
        self.string_map("SELECT episode_guid, image_source FROM episode_image_overrides")
    }

    fn set_episode_image_override(&mut self, episode_guid: &str, image_source: Option<&str>) {
        match image_source {
            None => {
                let _ = self.connection.execute(
                    "DELETE FROM episode_image_overrides WHERE episode_guid = ?1",
                    params![episode_guid],
                );
            }
            Some(source) => {
                let _ = self.connection.execute(
                    "INSERT OR REPLACE INTO episode_image_overrides (episode_guid, image_source) VALUES (?1, ?2)",
                    params![episode_guid, source],
                );
            }
        }
    }

    fn manual_categories(&self) -> Vec<ManualCategory> {
        let Ok(mut statement) = self
            .connection
            .prepare("SELECT uuid, title, image_source FROM manual_categories")
        else {
            return Vec::new();
        };
        statement
            .query_map([], |row| {
                Ok(ManualCategory {
                    uuid: row.get(0)?,
                    title: row.get(1)?,
                    image_source: row.get(2)?,
                })
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn add_manual_category(&mut self, category: ManualCategory) {
        let _ = self.connection.execute(
            "INSERT OR IGNORE INTO manual_categories (uuid, title, image_source) VALUES (?1, ?2, ?3)",
            params![category.uuid, category.title, category.image_source],
        );
    }

    fn remove_manual_category(&mut self, uuid: &str) {
        let _ = self.connection.execute(
            "DELETE FROM manual_categories WHERE uuid = ?1",
            params![uuid],
        );
    }

    fn uploaded_image_fingerprints(&self) -> HashMap<String, String> {
        let Ok(mut statement) = self.connection.prepare(
            "SELECT remote_name, fingerprint FROM uploaded_image_fingerprints WHERE device_id = ?1",
        ) else {
            return HashMap::new();
        };
        statement
            .query_map(params![self.device_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn set_uploaded_image_fingerprint(&mut self, remote_name: &str, fingerprint: &str) {
        let _ = self.connection.execute(
            "INSERT OR REPLACE INTO uploaded_image_fingerprints (device_id, remote_name, fingerprint) VALUES (?1, ?2, ?3)",
            params![self.device_id, remote_name, fingerprint],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::db;

    fn make_repo() -> SqliteSyncStateRepository {
        SqliteSyncStateRepository::new(db::open_in_memory().expect("db"), "test-device")
    }

    #[test]
    fn record_synced_then_upsert_overwrites_rather_than_duplicating() {
        let mut repo = make_repo();
        let first_date = Utc.timestamp_opt(1000, 0).unwrap();
        let second_date = Utc.timestamp_opt(2000, 0).unwrap();

        repo.record_synced("ep-1", "V1", "Dossier", first_date);
        repo.record_synced("ep-1", "V2", "Dossier", second_date);

        let records = repo.synced_records();
        assert_eq!(
            records.len(),
            1,
            "un même episodeUUID ne doit jamais se dupliquer"
        );
        assert_eq!(records[0].title, "V2");
        assert_eq!(records[0].synced_at, second_date);
    }

    #[test]
    fn mark_pending_deletion_toggles_flag() {
        let mut repo = make_repo();
        repo.record_synced("ep-1", "T", "D", Utc::now());

        repo.mark_pending_deletion("ep-1", true);
        assert!(repo.synced_records()[0].pending_deletion);

        repo.mark_pending_deletion("ep-1", false);
        assert!(!repo.synced_records()[0].pending_deletion);
    }

    #[test]
    fn delete_synced_records_removes_only_matching_uuids() {
        let mut repo = make_repo();
        repo.record_synced("ep-1", "A", "D", Utc::now());
        repo.record_synced("ep-2", "B", "D", Utc::now());

        repo.delete_synced_records(&HashSet::from(["ep-1".to_string()]));

        let uuids: Vec<String> = repo
            .synced_records()
            .into_iter()
            .map(|r| r.episode_uuid)
            .collect();
        assert_eq!(uuids, ["ep-2"]);
    }

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "merlin-multidev-{}-{tag}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn synced_records_are_scoped_per_device() {
        let path = temp_db("scope");
        SqliteSyncStateRepository::new(db::open(&path).unwrap(), "aa:aa").record_synced(
            "ep-1",
            "T",
            "F",
            Utc::now(),
        );

        assert_eq!(
            SqliteSyncStateRepository::new(db::open(&path).unwrap(), "aa:aa")
                .synced_records()
                .len(),
            1
        );
        assert!(
            SqliteSyncStateRepository::new(db::open(&path).unwrap(), "bb:bb")
                .synced_records()
                .is_empty(),
            "l'enceinte B ne doit pas voir les records de A"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]

    fn legacy_records_are_never_claimed_by_a_connecting_device() {
        let path = temp_db("claim");

        SqliteSyncStateRepository::new(db::open(&path).unwrap(), "").record_synced(
            "ep-1",
            "T",
            "F",
            Utc::now(),
        );

        let first = SqliteSyncStateRepository::new(db::open(&path).unwrap(), "aa:aa");
        assert_eq!(
            first.orphan_record_count(),
            1,
            "l'orphelin est bien signale"
        );
        assert!(first.synced_records().is_empty(), "aa:aa n'herite de rien");

        let second = SqliteSyncStateRepository::new(db::open(&path).unwrap(), "bb:bb");
        assert!(second.synced_records().is_empty(), "bb:bb non plus");
        assert_eq!(second.orphan_record_count(), 1, "l'orphelin reste orphelin");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn episode_number_override_upsert_then_none_deletes() {
        let mut repo = make_repo();

        repo.set_episode_number_override("guid-1", Some(5));
        assert_eq!(
            repo.episode_number_overrides(),
            HashMap::from([("guid-1".to_string(), 5)])
        );

        repo.set_episode_number_override("guid-1", Some(7));
        assert_eq!(
            repo.episode_number_overrides(),
            HashMap::from([("guid-1".to_string(), 7)]),
            "un second appel doit écraser, pas dupliquer"
        );

        repo.set_episode_number_override("guid-1", None);
        assert!(
            repo.episode_number_overrides().is_empty(),
            "None doit supprimer l'override"
        );
    }

    #[test]
    fn group_title_override_indexed_by_combined_key() {
        let mut repo = make_repo();
        repo.set_group_title_override("https://a.com", "Chapitre", Some("Mon Titre"));

        let key = group_override_key("https://a.com", "Chapitre");
        assert_eq!(
            repo.group_title_overrides().get(&key).map(String::as_str),
            Some("Mon Titre")
        );
    }

    #[test]
    fn none_clears_title_overrides() {
        let mut repo = make_repo();
        repo.set_group_title_override("https://a.com", "Chapitre", Some("T"));
        repo.set_episode_title_override("guid-1", Some("T"));

        repo.set_group_title_override("https://a.com", "Chapitre", None);
        repo.set_episode_title_override("guid-1", None);

        assert!(repo.group_title_overrides().is_empty());
        assert!(repo.episode_title_overrides().is_empty());
    }

    #[test]
    fn remove_category_assignment_deletes_it() {
        let mut repo = make_repo();
        repo.set_category_assignment(PodcastCategoryAssignment {
            feed_url: "https://a.com".into(),
            group_key: String::new(),
            target_category_uuid: "uuid-1".into(),
            target_category_title: "Histoires".into(),
        });

        repo.remove_category_assignment("https://a.com", "");

        assert!(repo.category_assignments().is_empty());
    }

    #[test]
    fn category_assignment_upsert_overwrites_existing() {
        let mut repo = make_repo();
        let first = PodcastCategoryAssignment {
            feed_url: "https://a.com".into(),
            group_key: String::new(),
            target_category_uuid: "uuid-1".into(),
            target_category_title: "Histoires".into(),
        };
        let second = PodcastCategoryAssignment {
            target_category_uuid: "uuid-2".into(),
            target_category_title: "Documentaires".into(),
            ..first.clone()
        };

        repo.set_category_assignment(first);
        repo.set_category_assignment(second);

        let key = group_override_key("https://a.com", "");
        let assignments = repo.category_assignments();
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[&key].target_category_uuid, "uuid-2");
    }

    #[test]
    fn episode_title_override_upsert() {
        let mut repo = make_repo();
        repo.set_episode_title_override("guid-1", Some("Titre Court"));
        repo.set_episode_title_override("guid-1", Some("Titre Encore Plus Court"));

        assert_eq!(
            repo.episode_title_overrides(),
            HashMap::from([("guid-1".to_string(), "Titre Encore Plus Court".to_string())])
        );
    }

    #[test]
    fn folder_image_override_upsert_then_none_deletes() {
        let mut repo = make_repo();

        repo.set_folder_image_override("folder-1", Some("https://a.com/img.jpg"));
        assert_eq!(
            repo.folder_image_overrides(),
            HashMap::from([("folder-1".to_string(), "https://a.com/img.jpg".to_string())])
        );

        repo.set_folder_image_override("folder-1", Some("file:///tmp/custom.jpg"));
        assert_eq!(
            repo.folder_image_overrides(),
            HashMap::from([("folder-1".to_string(), "file:///tmp/custom.jpg".to_string())]),
            "un second appel doit écraser, pas dupliquer"
        );

        repo.set_folder_image_override("folder-1", None);
        assert!(
            repo.folder_image_overrides().is_empty(),
            "None doit supprimer l'override"
        );
    }

    #[test]
    fn episode_image_override_upsert_then_none_deletes() {
        let mut repo = make_repo();

        repo.set_episode_image_override("guid-1", Some("https://a.com/ep.jpg"));
        assert_eq!(
            repo.episode_image_overrides(),
            HashMap::from([("guid-1".to_string(), "https://a.com/ep.jpg".to_string())])
        );

        repo.set_episode_image_override("guid-1", None);
        assert!(repo.episode_image_overrides().is_empty());
    }

    #[test]
    fn manual_category_add_then_list_returns_it() {
        let mut repo = make_repo();
        let category = ManualCategory {
            uuid: "manual-1".into(),
            title: "Contes".into(),
            image_source: "https://a.com/contes.jpg".into(),
        };
        repo.add_manual_category(category.clone());

        assert_eq!(repo.manual_categories(), vec![category]);
    }

    #[test]
    fn manual_category_add_twice_with_same_uuid_does_not_duplicate() {
        let mut repo = make_repo();
        repo.add_manual_category(ManualCategory {
            uuid: "manual-1".into(),
            title: "Contes".into(),
            image_source: "https://a.com/contes.jpg".into(),
        });
        repo.add_manual_category(ManualCategory {
            uuid: "manual-1".into(),
            title: "Autre titre".into(),
            image_source: "https://a.com/autre.jpg".into(),
        });

        assert_eq!(
            repo.manual_categories().len(),
            1,
            "un même uuid ne doit jamais se dupliquer"
        );
    }

    #[test]
    fn manual_category_remove() {
        let mut repo = make_repo();
        repo.add_manual_category(ManualCategory {
            uuid: "manual-1".into(),
            title: "Contes".into(),
            image_source: "https://a.com/contes.jpg".into(),
        });

        repo.remove_manual_category("manual-1");

        assert!(repo.manual_categories().is_empty());
    }
}
