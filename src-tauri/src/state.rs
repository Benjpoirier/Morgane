use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;

use crate::tree_session::TreeSession;

pub struct AppState {
    pub db_path: PathBuf,
    pub work_dir: PathBuf,

    pub op_lock: Arc<tokio::sync::Mutex<()>>,

    pub sync_cancel: Arc<Mutex<Option<CancellationToken>>>,

    pub sync_running: Arc<AtomicBool>,

    pub prepare_running: Arc<AtomicBool>,

    pub current_device: Mutex<Option<String>>,

    pub tree: Mutex<TreeSession>,
}

impl AppState {
    pub fn device_id(&self) -> Option<String> {
        self.current_device.lock().expect("lock").clone()
    }

    pub fn require_device_id(&self) -> Result<String, String> {
        self.device_id().ok_or_else(|| {
            "enceinte non identifiee (adresse MAC inconnue) : connecte-toi a l'enceinte avant de synchroniser".to_string()
        })
    }

    pub fn read_device_id(&self) -> String {
        self.device_id()
            .unwrap_or_else(|| UNKNOWN_DEVICE.to_string())
    }
}

pub const UNKNOWN_DEVICE: &str = "\u{0}unknown-device";

impl AppState {
    pub fn new() -> Self {
        let db_path = merlin_infra::persistence::db::default_database_path();

        let active_device =
            merlin_infra::persistence::db::open(&db_path)
                .ok()
                .and_then(|connection| {
                    let devices =
                        merlin_infra::persistence::device_repository::SqliteDeviceRepository::new(
                            connection,
                        );
                    if let Some(active) = devices.active() {
                        return Some(active);
                    }

                    let fallback = devices.all().into_iter().next().map(|d| d.mac);
                    if let Some(mac) = &fallback {
                        devices.set_active(Some(mac));
                    }
                    fallback
                });

        if let Some(mac) = &active_device
            && let Ok(connection) = merlin_infra::persistence::db::open(&db_path)
        {
            merlin_infra::persistence::subscription_repository::SqliteSubscriptionRepository::new(
                connection,
                mac.clone(),
            )
            .claim_legacy();
        }
        Self {
            db_path,
            work_dir: dirs::data_dir()
                .expect("répertoire de données utilisateur introuvable")
                .join("merlinSync"),
            op_lock: Arc::new(tokio::sync::Mutex::new(())),
            sync_cancel: Arc::new(Mutex::new(None)),
            sync_running: Arc::new(AtomicBool::new(false)),
            prepare_running: Arc::new(AtomicBool::new(false)),
            current_device: Mutex::new(active_device),
            tree: Mutex::new(TreeSession::default()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
