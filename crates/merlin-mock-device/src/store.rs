use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("impossible d'ouvrir le store : {0}")]
    OpenFailed(String),
    #[error("requete SQLite echouee : {0}")]
    ExecutionFailed(String),
}

pub struct MockDeviceStore {
    db: Connection,
}

impl MockDeviceStore {
    pub fn new(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::OpenFailed(e.to_string()))?;
        }
        let db = Connection::open(path).map_err(|e| StoreError::OpenFailed(e.to_string()))?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS files (name TEXT PRIMARY KEY, content BLOB NOT NULL)",
            [],
        )
        .map_err(|e| StoreError::ExecutionFailed(e.to_string()))?;
        Ok(Self { db })
    }

    pub fn all(&self) -> Result<HashMap<String, Vec<u8>>, StoreError> {
        let mut statement = self
            .db
            .prepare("SELECT name, content FROM files")
            .map_err(|e| StoreError::ExecutionFailed(e.to_string()))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|e| StoreError::ExecutionFailed(e.to_string()))?;
        let mut result = HashMap::new();
        for row in rows {
            let (name, content) = row.map_err(|e| StoreError::ExecutionFailed(e.to_string()))?;
            result.insert(name, content);
        }
        Ok(result)
    }

    pub fn set(&self, name: &str, content: &[u8]) -> Result<(), StoreError> {
        self.db
            .execute(
                "INSERT OR REPLACE INTO files (name, content) VALUES (?1, ?2)",
                rusqlite::params![name, content],
            )
            .map_err(|e| StoreError::ExecutionFailed(e.to_string()))?;
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<(), StoreError> {
        self.db
            .execute("DELETE FROM files WHERE name = ?1", rusqlite::params![name])
            .map_err(|e| StoreError::ExecutionFailed(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store_path() -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!("MockDeviceStoreTests-{}", uuid_like()))
            .join("store.sqlite3")
    }

    fn uuid_like() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        format!(
            "{}-{:p}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            &()
        )
    }

    #[test]
    fn empty_store_returns_no_files() {
        let store = MockDeviceStore::new(&temp_store_path()).unwrap();
        assert!(store.all().unwrap().is_empty());
    }

    #[test]
    fn set_then_all_returns_stored_content() {
        let store = MockDeviceStore::new(&temp_store_path()).unwrap();
        store.set("playlist.json", &[1, 2, 3]).unwrap();
        store.set("abc.mp3", &[0xFF, 0x00]).unwrap();

        let all = store.all().unwrap();
        assert_eq!(all["playlist.json"], [1, 2, 3]);
        assert_eq!(all["abc.mp3"], [0xFF, 0x00]);
    }

    #[test]
    fn set_overwrites_existing_entry() {
        let store = MockDeviceStore::new(&temp_store_path()).unwrap();
        store.set("playlist.json", &[1]).unwrap();
        store.set("playlist.json", &[2, 2]).unwrap();

        assert_eq!(store.all().unwrap()["playlist.json"], [2, 2]);
    }

    #[test]
    fn delete_removes_entry() {
        let store = MockDeviceStore::new(&temp_store_path()).unwrap();
        store.set("abc.mp3", &[1]).unwrap();
        store.delete("abc.mp3").unwrap();

        assert!(!store.all().unwrap().contains_key("abc.mp3"));
    }

    #[test]
    fn content_persists_across_reopen() {
        let path = temp_store_path();
        {
            let store = MockDeviceStore::new(&path).unwrap();
            store.set("playlist.bin", &[9, 8, 7]).unwrap();
        }
        let reopened = MockDeviceStore::new(&path).unwrap();
        assert_eq!(reopened.all().unwrap()["playlist.bin"], [9, 8, 7]);
    }
}
