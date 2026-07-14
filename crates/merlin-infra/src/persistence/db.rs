use std::path::{Path, PathBuf};

use rusqlite::Connection;

pub fn default_database_path() -> PathBuf {
    dirs::data_dir()
        .expect("répertoire de données utilisateur introuvable")
        .join("merlinSync/merlinsync.sqlite3")
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let connection = Connection::open(path)?;
    ensure_schema(&connection)?;
    migrate(&connection)?;
    Ok(connection)
}

pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let connection = Connection::open_in_memory()?;
    ensure_schema(&connection)?;
    migrate(&connection)?;
    Ok(connection)
}

fn migrate(connection: &Connection) -> rusqlite::Result<()> {
    let has_device_id = connection
        .prepare("PRAGMA table_info(synced_records)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|name| name == "device_id");
    if !has_device_id {
        connection.execute_batch(
            "BEGIN;
            CREATE TABLE synced_records_v2 (
                device_id TEXT NOT NULL,
                episode_uuid TEXT NOT NULL,
                title TEXT NOT NULL,
                folder_title TEXT NOT NULL,
                synced_at INTEGER NOT NULL,
                pending_deletion INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (device_id, episode_uuid)
            );
            INSERT INTO synced_records_v2
                SELECT '', episode_uuid, title, folder_title, synced_at, pending_deletion
                FROM synced_records;
            DROP TABLE synced_records;
            ALTER TABLE synced_records_v2 RENAME TO synced_records;
            COMMIT;",
        )?;
    }

    let has_sub_device = connection
        .prepare("PRAGMA table_info(subscriptions)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|name| name == "device_id");
    if !has_sub_device {
        connection.execute_batch(
            "BEGIN;
            CREATE TABLE subscriptions_v2 (
                device_id TEXT NOT NULL DEFAULT '',
                feed_url TEXT NOT NULL,
                title TEXT NOT NULL,
                kind TEXT NOT NULL,
                category TEXT NOT NULL,
                selected_episode_guids TEXT NOT NULL DEFAULT '[]',
                feed_image_url TEXT,
                direct_audio_url TEXT,
                direct_title TEXT,
                direct_image_url TEXT,
                PRIMARY KEY (device_id, feed_url)
            );
            INSERT INTO subscriptions_v2
                SELECT '', feed_url, title, kind, category, selected_episode_guids,
                       feed_image_url, direct_audio_url, direct_title, direct_image_url
                FROM subscriptions;
            DROP TABLE subscriptions;
            ALTER TABLE subscriptions_v2 RENAME TO subscriptions;
            COMMIT;",
        )?;
    }
    Ok(())
}

fn ensure_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS subscriptions (
            device_id TEXT NOT NULL DEFAULT '',
            feed_url TEXT NOT NULL,
            title TEXT NOT NULL,
            kind TEXT NOT NULL,
            category TEXT NOT NULL,
            selected_episode_guids TEXT NOT NULL DEFAULT '[]',
            feed_image_url TEXT,
            direct_audio_url TEXT,
            direct_title TEXT,
            direct_image_url TEXT,
            PRIMARY KEY (device_id, feed_url)
        );
        CREATE TABLE IF NOT EXISTS synced_records (
            device_id TEXT NOT NULL DEFAULT '',
            episode_uuid TEXT NOT NULL,
            title TEXT NOT NULL,
            folder_title TEXT NOT NULL,
            synced_at INTEGER NOT NULL,
            pending_deletion INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (device_id, episode_uuid)
        );
        CREATE TABLE IF NOT EXISTS episode_number_overrides (
            episode_guid TEXT PRIMARY KEY,
            number INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS group_title_overrides (
            feed_url TEXT NOT NULL,
            group_key TEXT NOT NULL,
            custom_title TEXT NOT NULL,
            PRIMARY KEY (feed_url, group_key)
        );
        CREATE TABLE IF NOT EXISTS category_assignments (
            feed_url TEXT NOT NULL,
            group_key TEXT NOT NULL,
            target_category_uuid TEXT NOT NULL,
            target_category_title TEXT NOT NULL,
            PRIMARY KEY (feed_url, group_key)
        );
        CREATE TABLE IF NOT EXISTS episode_title_overrides (
            episode_guid TEXT PRIMARY KEY,
            custom_title TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS folder_image_overrides (
            folder_uuid TEXT PRIMARY KEY,
            image_source TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS episode_image_overrides (
            episode_guid TEXT PRIMARY KEY,
            image_source TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS manual_categories (
            uuid TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            image_source TEXT NOT NULL
        );
        -- Empreinte du dernier visuel (dossier ou episode) televerse sous ce
        -- nom, PAR enceinte : permet de ne re-uploader une image que si son
        -- contenu a change. Scope par device_id, comme synced_records.
        CREATE TABLE IF NOT EXISTS uploaded_image_fingerprints (
            device_id TEXT NOT NULL,
            remote_name TEXT NOT NULL,
            fingerprint TEXT NOT NULL,
            PRIMARY KEY (device_id, remote_name)
        );
        -- Enceintes enregistrees (MAC = device_id). L'enregistrement rend
        -- l'onglet Podcasts utilisable hors connexion et memorise la cible de
        -- synchro. Cf. merlin_domain::library::device.
        CREATE TABLE IF NOT EXISTS registered_devices (
            mac TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            registered_at INTEGER NOT NULL,
            last_connected_at INTEGER
        );
        -- Reglages cle/valeur (ex. active_device_mac : l'enceinte dont l'etat
        -- de synchro est affiche, meme hors connexion).
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        -- Episodes deja vus par flux (JSON de guids) : sert a marquer « NEW »
        -- les episodes apparus depuis. Le 1er chargement d'un flux sert de
        -- baseline (tout le back-catalogue est marque vu, rien n'est nouveau).
        CREATE TABLE IF NOT EXISTS feed_seen (
            feed_url TEXT PRIMARY KEY,
            seen_guids TEXT NOT NULL DEFAULT '[]'
        );",
    )
}
