use std::path::PathBuf;

#[derive(Debug, Clone, thiserror::Error)]
pub enum SyncError {
    #[error("aucun episode a synchroniser")]
    NoEpisodes,
    #[error("pas de reponse de l'enceinte pour {0} - la connexion Wi-Fi a peut-etre ete coupee")]
    NoResponse(String),
    #[error("pas de reponse searchFile valide de l'enceinte pour {0}")]
    FileSearchFailed(String),
    #[error("l'enceinte a rejete l'envoi de {0} : {1}")]
    SendFileRejected(String, String),
    #[error("l'enceinte a rejete la mise a jour de la playlist : {0}")]
    UpdatePlaylistRejected(String),

    #[error(
        "impossible de recuperer le visuel de la categorie \"{folder_title}\" ({underlying}) - synchronisation annulee avant d'envoyer quoi que ce soit. Corrige son visuel (cliquer sa vignette dans Synchroniser) puis reessaie."
    )]
    FolderImageUploadFailed {
        folder_title: String,
        underlying: String,
    },

    #[error(
        "playlist.bin recuperee ({byte_count} octets) mais aucune entree n'a pu en etre lue - fichier probablement corrompu. Synchronisation annulee plutot que de risquer d'ecraser le contenu existant avec un arbre vide."
    )]
    CorruptPlaylistBin { byte_count: usize },

    #[error(
        "l'enceinte n'a pas repondu a la mise a jour de la playlist - impossible de confirmer si elle a ete acceptee. Verifie la connexion et reessaie."
    )]
    UpdatePlaylistNoResponse,
}

#[derive(Debug, Clone)]
pub struct EpisodeToSync {
    pub folder_uuid: String,
    pub folder_title: String,
    pub episode_uuid: String,
    pub episode_title: String,
    pub audio_path: PathBuf,
    pub image_path: Option<PathBuf>,
    pub category_title: String,

    pub category_uuid: Option<String>,

    pub folder_image_url: Option<String>,

    pub already_uploaded: bool,

    pub order: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedEpisode {
    pub episode_uuid: String,
    pub title: String,
    pub folder_title: String,
}
