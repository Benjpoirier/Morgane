use merlin_domain::sync::types::SyncError;
use merlin_protocol::client::{DownloadError, MerlinConnectionError};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    Connection(#[from] MerlinConnectionError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("synchronisation annulee")]
    Cancelled,
    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for EngineError {
    fn from(error: std::io::Error) -> Self {
        EngineError::Io(error.to_string())
    }
}
