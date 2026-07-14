use std::time::{SystemTime, UNIX_EPOCH};

use merlin_domain::library::device::RegisteredDevice;
use rusqlite::{Connection, params};

const ACTIVE_KEY: &str = "active_device_mac";

pub struct SqliteDeviceRepository {
    connection: Connection,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl SqliteDeviceRepository {
    pub fn new(connection: Connection) -> Self {
        Self { connection }
    }

    pub fn register(&self, mac: &str, name: &str) -> bool {
        let existed = self
            .connection
            .query_row(
                "SELECT 1 FROM registered_devices WHERE mac = ?1",
                params![mac],
                |_| Ok(()),
            )
            .is_ok();
        let _ = self.connection.execute(
            "INSERT INTO registered_devices (mac, name, registered_at, last_connected_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(mac) DO UPDATE SET last_connected_at = ?3",
            params![mac, name, now()],
        );
        !existed
    }

    pub fn rename(&self, mac: &str, name: &str) {
        let _ = self.connection.execute(
            "UPDATE registered_devices SET name = ?2 WHERE mac = ?1",
            params![mac, name],
        );
    }

    pub fn remove(&self, mac: &str) {
        let _ = self.connection.execute(
            "DELETE FROM registered_devices WHERE mac = ?1",
            params![mac],
        );
        if self.active().as_deref() == Some(mac) {
            self.set_active(None);
        }
    }

    pub fn all(&self) -> Vec<RegisteredDevice> {
        let active = self.active();
        let Ok(mut statement) = self.connection.prepare(
            "SELECT mac, name, last_connected_at FROM registered_devices
             ORDER BY last_connected_at DESC, name ASC",
        ) else {
            return Vec::new();
        };
        statement
            .query_map([], |row| {
                let mac: String = row.get(0)?;
                Ok(RegisteredDevice {
                    is_active: active.as_deref() == Some(mac.as_str()),
                    mac,
                    name: row.get(1)?,
                    last_connected_at: row.get(2)?,
                })
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    pub fn active(&self) -> Option<String> {
        self.connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![ACTIVE_KEY],
                |row| row.get::<_, String>(0),
            )
            .ok()
    }

    pub fn set_active(&self, mac: Option<&str>) {
        match mac {
            Some(mac) => {
                let _ = self.connection.execute(
                    "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = ?2",
                    params![ACTIVE_KEY, mac],
                );
            }
            None => {
                let _ = self.connection.execute(
                    "DELETE FROM app_settings WHERE key = ?1",
                    params![ACTIVE_KEY],
                );
            }
        }
    }
}
