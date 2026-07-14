use std::collections::HashSet;

use rusqlite::{Connection, params};

pub struct SqliteSeenRepository {
    connection: Connection,
}

impl SqliteSeenRepository {
    pub fn new(connection: Connection) -> Self {
        Self { connection }
    }

    fn seen_set(&self, feed_url: &str) -> Option<Vec<String>> {
        self.connection
            .query_row(
                "SELECT seen_guids FROM feed_seen WHERE feed_url = ?1",
                params![feed_url],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|json| serde_json::from_str(&json).unwrap_or_default())
    }

    fn write(&self, feed_url: &str, guids: &[String]) {
        let json = serde_json::to_string(guids).unwrap_or_else(|_| "[]".to_string());
        let _ = self.connection.execute(
            "INSERT INTO feed_seen (feed_url, seen_guids) VALUES (?1, ?2)
             ON CONFLICT(feed_url) DO UPDATE SET seen_guids = ?2",
            params![feed_url, json],
        );
    }

    pub fn new_episodes(&self, feed_url: &str, current: &[String]) -> Vec<String> {
        match self.seen_set(feed_url) {
            None => {
                self.write(feed_url, current);
                Vec::new()
            }
            Some(seen) => {
                let seen: HashSet<&str> = seen.iter().map(String::as_str).collect();
                current
                    .iter()
                    .filter(|guid| !seen.contains(guid.as_str()))
                    .cloned()
                    .collect()
            }
        }
    }

    pub fn mark_seen(&self, feed_url: &str, guids: &[String]) {
        let mut set = self.seen_set(feed_url).unwrap_or_default();
        let existing: HashSet<String> = set.iter().cloned().collect();
        for guid in guids {
            if !existing.contains(guid) {
                set.push(guid.clone());
            }
        }
        self.write(feed_url, &set);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::db;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn first_load_is_a_silent_baseline() {
        let repo = SqliteSeenRepository::new(db::open_in_memory().unwrap());
        let new = repo.new_episodes("feed", &strings(&["a", "b", "c"]));
        assert!(
            new.is_empty(),
            "le back-catalogue initial ne doit pas etre nouveau"
        );
    }

    #[test]
    fn later_episodes_are_new_until_marked() {
        let repo = SqliteSeenRepository::new(db::open_in_memory().unwrap());
        repo.new_episodes("feed", &strings(&["a", "b"]));

        assert_eq!(
            repo.new_episodes("feed", &strings(&["a", "b", "c"])),
            strings(&["c"])
        );

        assert_eq!(
            repo.new_episodes("feed", &strings(&["a", "b", "c"])),
            strings(&["c"])
        );
        repo.mark_seen("feed", &strings(&["c"]));
        assert!(
            repo.new_episodes("feed", &strings(&["a", "b", "c"]))
                .is_empty()
        );
    }

    #[test]
    fn feeds_are_independent() {
        let repo = SqliteSeenRepository::new(db::open_in_memory().unwrap());
        repo.new_episodes("f1", &strings(&["a"]));

        assert!(repo.new_episodes("f2", &strings(&["z"])).is_empty());
        assert_eq!(
            repo.new_episodes("f1", &strings(&["a", "b"])),
            strings(&["b"])
        );
    }
}
