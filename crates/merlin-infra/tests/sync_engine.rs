use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::Value;

use merlin_domain::library::manual_category::ManualCategory;
use merlin_domain::playlist::tree_edit::TreeEdit;
use merlin_domain::sync::types::{EpisodeToSync, SyncError};
use merlin_infra::sync::engine::{RepairFile, SyncCallbacks, SyncEngine, SyncRequest};
use merlin_infra::sync::error::EngineError;
use merlin_mock_device::FakeMerlinDevice;

use merlin_domain::playlist::bin_encoder;
use merlin_domain::playlist::bin_parser::{KIND_FAVORITE, KIND_FOLDER, KIND_ROOT, KIND_SOUND};

const RECORD_SIZE: usize = 152;

fn make_record(id: u16, parent_id: u16, kind: u16, uuid: &str, title: &str) -> Vec<u8> {
    let mut record = vec![0u8; RECORD_SIZE];
    record[0..2].copy_from_slice(&id.to_le_bytes());
    record[2..4].copy_from_slice(&parent_id.to_le_bytes());
    record[10..12].copy_from_slice(&kind.to_le_bytes());
    let uuid_bytes = uuid.as_bytes();
    record[20] = uuid_bytes.len() as u8;
    record[21..21 + uuid_bytes.len()].copy_from_slice(uuid_bytes);
    let title_bytes = title.as_bytes();
    record[85] = title_bytes.len() as u8;
    record[86..86 + title_bytes.len()].copy_from_slice(title_bytes);
    record
}

fn make_playlist_bin(records: &[(u16, u16, &str, &str)]) -> Vec<u8> {
    let parents: HashSet<u16> = records.iter().map(|(_, pid, _, _)| *pid).collect();
    records
        .iter()
        .flat_map(|(id, pid, uuid, title)| {
            let kind = if *pid == 0 {
                KIND_ROOT
            } else if *title == bin_encoder::FAVORITE_RECORD_TITLE {
                KIND_FAVORITE
            } else if parents.contains(id) {
                KIND_FOLDER
            } else {
                KIND_SOUND
            };
            make_record(*id, *pid, kind, uuid, title)
        })
        .collect()
}

fn make_temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("{}-{name}", uuid::Uuid::new_v4()));
    std::fs::write(&path, bytes).expect("écriture du fichier temporaire");
    path
}

fn make_audio() -> PathBuf {
    let bytes: Vec<u8> = (0..4096usize).map(|i| (i % 256) as u8).collect();
    make_temp_file("episode.mp3", &bytes)
}

fn make_image() -> PathBuf {
    let mut bytes = vec![0xFFu8, 0xD8, 0xFF, 0xE0];
    bytes.extend(std::iter::repeat_n(0xABu8, 64));
    make_temp_file("episode.jpg", &bytes)
}

struct EpisodeSpec {
    folder_uuid: &'static str,
    folder_title: &'static str,
    episode_uuid: &'static str,
    episode_title: &'static str,
    audio: PathBuf,
    image: Option<PathBuf>,
    category_title: &'static str,
    category_uuid: Option<&'static str>,
    already_uploaded: bool,
}

impl Default for EpisodeSpec {
    fn default() -> Self {
        Self {
            folder_uuid: "folder-1",
            folder_title: "Podcast",
            episode_uuid: "ep-1",
            episode_title: "Episode",
            audio: make_audio(),
            image: None,
            category_title: "",
            category_uuid: None,
            already_uploaded: false,
        }
    }
}

fn episode(spec: EpisodeSpec) -> EpisodeToSync {
    EpisodeToSync {
        folder_uuid: spec.folder_uuid.into(),
        folder_title: spec.folder_title.into(),
        episode_uuid: spec.episode_uuid.into(),
        episode_title: spec.episode_title.into(),
        audio_path: spec.audio,
        image_path: spec.image,
        category_title: spec.category_title.into(),
        category_uuid: spec.category_uuid.map(String::from),
        folder_image_url: None,
        already_uploaded: spec.already_uploaded,
        order: None,
    }
}

async fn start_device(initial_files: &[(&str, Vec<u8>)]) -> FakeMerlinDevice {
    let files: HashMap<String, Vec<u8>> = initial_files
        .iter()
        .map(|(name, bytes)| (name.to_string(), bytes.clone()))
        .collect();
    FakeMerlinDevice::start(0, files, false, None)
        .await
        .expect("démarrage du mock")
}

fn request(episodes: Vec<EpisodeToSync>) -> SyncRequest {
    SyncRequest {
        episodes,
        ..Default::default()
    }
}

fn parse_playlist(device: &FakeMerlinDevice) -> Vec<Value> {
    let data = device
        .received_playlist_json()
        .expect("playlist.json jamais reçu par le faux appareil");
    serde_json::from_slice::<Value>(&data)
        .expect("playlist.json reçu n'est pas du JSON valide")
        .as_array()
        .expect("playlist.json reçu n'est pas un tableau")
        .clone()
}

fn all_titles(folders: &[Value]) -> Vec<String> {
    let mut titles = Vec::new();
    for folder in folders {
        if let Some(title) = folder["title"].as_str() {
            titles.push(title.to_string());
        }
        if let Some(children) = folder["child"].as_array() {
            titles.extend(all_titles(children));
        }
    }
    titles
}

fn find_folder<'a>(title: &str, folders: &'a [Value]) -> Option<&'a Value> {
    folders.iter().find(|f| f["title"].as_str() == Some(title))
}

#[tokio::test]
async fn fresh_sync_uploads_episode_and_builds_correct_playlist() {
    let device = start_device(&[]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let ep = episode(EpisodeSpec {
        folder_uuid: "folder-1",
        folder_title: "Mon Podcast",
        episode_title: "Episode Un",
        image: Some(make_image()),
        ..Default::default()
    });

    let synced = engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await
        .expect("sync");

    assert_eq!(synced.len(), 1);
    assert_eq!(synced[0].episode_uuid, "ep-1");

    let (files, _) = device.snapshot();
    assert!(
        files.contains_key("ep-1.mp3"),
        "l'audio doit avoir été envoyé"
    );
    assert!(
        files.contains_key("ep-1.jpg"),
        "l'image doit avoir été envoyée"
    );
    assert!(
        files.contains_key("folder-1.jpg"),
        "l'image de dossier (repli sur celle de l'épisode) doit avoir été envoyée"
    );

    let root = parse_playlist(&device);
    let titles = all_titles(&root);
    assert!(titles.contains(&"Mon Podcast".to_string()));
    assert!(titles.contains(&"Episode Un".to_string()));

    assert_eq!(root.iter().filter(|f| f["favorite"] == 1).count(), 1);
}

#[tokio::test]
async fn missing_playlist_bin_starts_from_empty_tree_instead_of_failing() {
    let device = start_device(&[]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let synced = engine
        .sync(
            &request(vec![episode(EpisodeSpec::default())]),
            &SyncCallbacks::silent(),
        )
        .await
        .expect("sync");
    assert_eq!(synced.len(), 1);
    assert!(all_titles(&parse_playlist(&device)).contains(&"Episode".to_string()));
}

#[tokio::test]
async fn merges_new_content_with_existing_live_tree_instead_of_replacing_it() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "cat-histoires", "Histoires"),
        (3, 2, "existing-ep", "Deja Present"),
    ]);
    let device = start_device(&[
        ("playlist.bin", existing_bin),
        ("existing-ep.mp3", vec![0x01, 0x02]),
    ])
    .await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let ep = episode(EpisodeSpec {
        folder_uuid: "folder-nouveau",
        folder_title: "Nouveau Podcast",
        episode_uuid: "ep-nouveau",
        episode_title: "Episode Nouveau",
        image: Some(make_image()),
        category_title: "Documentaires",
        ..Default::default()
    });

    engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await
        .expect("sync");

    let titles = all_titles(&parse_playlist(&device));

    assert!(
        titles.contains(&"Histoires".to_string()),
        "la catégorie existante ne doit pas disparaître"
    );
    assert!(
        titles.contains(&"Deja Present".to_string()),
        "l'épisode existant ne doit pas disparaître"
    );

    assert!(titles.contains(&"Documentaires".to_string()));
    assert!(titles.contains(&"Episode Nouveau".to_string()));
}

#[tokio::test]
async fn deletion_only_sync_still_rebuilds_and_uploads_playlist() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "cat-1", "Categorie"),
        (3, 2, "to-delete", "A Supprimer"),
    ]);
    let device = start_device(&[
        ("playlist.bin", existing_bin),
        ("to-delete.mp3", vec![0x01]),
        ("to-delete.jpg", vec![0x02]),
    ])
    .await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let sync_request = SyncRequest {
        files_to_delete: vec!["to-delete.mp3".into(), "to-delete.jpg".into()],
        ..Default::default()
    };
    let synced = engine
        .sync(&sync_request, &SyncCallbacks::silent())
        .await
        .expect("sync");

    assert!(synced.is_empty());
    let (files, deleted) = device.snapshot();
    assert!(deleted.contains(&"to-delete.mp3".to_string()));
    assert!(deleted.contains(&"to-delete.jpg".to_string()));

    assert!(
        files.contains_key("playlist.json"),
        "playlist.json doit être envoyé même pour un sync de suppression pure"
    );
    assert!(!all_titles(&parse_playlist(&device)).contains(&"A Supprimer".to_string()));
}

#[tokio::test]
async fn tree_edit_rename_is_applied_against_freshly_downloaded_live_tree() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "folder-a", "Ancien Nom"),
        (3, 2, "ep-in-folder-a", "Episode"),
    ]);
    let device = start_device(&[
        ("playlist.bin", existing_bin),
        ("ep-in-folder-a.mp3", vec![1]),
    ])
    .await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let sync_request = SyncRequest {
        tree_edits: vec![TreeEdit::RenamedFolder {
            uuid: "folder-a".into(),
            new_title: "Nouveau Nom".into(),
        }],
        ..Default::default()
    };
    engine
        .sync(&sync_request, &SyncCallbacks::silent())
        .await
        .expect("sync");

    let titles = all_titles(&parse_playlist(&device));
    assert!(
        titles.contains(&"Nouveau Nom".to_string()),
        "le renommage doit être rejoué contre l'arbre live"
    );
    assert!(!titles.contains(&"Ancien Nom".to_string()));
}

#[tokio::test]
async fn a_lone_folder_image_override_syncs_the_image() {
    let uuid = "8a251a87-0000-0000-0000-000000000001";
    let child = "91213766-0000-0000-0000-000000000002";

    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, uuid, "Histoires"),
        (3, 2, child, "Ep"),
    ]);
    let device = start_device(&[
        ("playlist.bin", existing_bin),
        (&format!("{child}.mp3"), vec![1]),
    ])
    .await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let jpg = std::env::temp_dir().join(format!("{}-folder.jpg", uuid::Uuid::new_v4()));
    image::RgbImage::new(2, 2).save(&jpg).expect("jpeg valide");

    let sync_request = SyncRequest {
        folder_image_overrides: HashMap::from([(
            uuid.to_string(),
            jpg.to_string_lossy().into_owned(),
        )]),
        ..Default::default()
    };
    engine
        .sync(&sync_request, &SyncCallbacks::silent())
        .await
        .expect("sync d'image seule");

    let (files, _) = device.snapshot();
    assert!(
        files.contains_key(&format!("{uuid}.jpg")),
        "le visuel de dossier doit être téléversé"
    );
}

#[tokio::test]
async fn move_into_a_new_manual_category_lands_at_sync() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "cat-src", "Source"),
        (3, 2, "podcast-1", "Podcast"),
        (4, 3, "ep-1", "Episode"),
    ]);
    let device = start_device(&[("playlist.bin", existing_bin), ("ep-1.mp3", vec![1])]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let sync_request = SyncRequest {
        manual_categories: vec![ManualCategory {
            uuid: "manual-cat".into(),
            title: "Ma Categorie".into(),
            image_source: String::new(),
        }],
        tree_edits: vec![TreeEdit::Moved {
            uuid: "podcast-1".into(),
            to_parent_uuid: "manual-cat".into(),
        }],
        ..Default::default()
    };
    engine
        .sync(&sync_request, &SyncCallbacks::silent())
        .await
        .expect("sync");

    let root = parse_playlist(&device);
    let cat = find_folder("Ma Categorie", &root).expect("catégorie manuelle créée");
    let children = cat["child"].as_array().expect("child array");
    assert!(
        children.iter().any(|c| c["title"] == "Podcast"),
        "le podcast déplacé doit être sous la catégorie manuelle"
    );
}

#[tokio::test]
async fn tree_edit_targeting_deleted_node_is_ignored_without_failing_whole_sync() {
    let existing_bin = make_playlist_bin(&[(1, 0, "", "Root")]);
    let device = start_device(&[("playlist.bin", existing_bin)]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let sync_request = SyncRequest {
        tree_edits: vec![TreeEdit::RenamedFolder {
            uuid: "folder-fantome".into(),
            new_title: "Peu Importe".into(),
        }],
        ..Default::default()
    };

    engine
        .sync(&sync_request, &SyncCallbacks::silent())
        .await
        .expect("sync");
}

#[tokio::test]
async fn stale_category_uuid_falls_back_to_deterministic_uuid_instead_of_resurrecting_deleted_category()
 {
    let existing_bin = make_playlist_bin(&[(1, 0, "", "Root")]);
    let device = start_device(&[("playlist.bin", existing_bin)]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let ep = episode(EpisodeSpec {
        category_title: "Histoires",
        category_uuid: Some("stale-category-uuid"),
        ..Default::default()
    });

    engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await
        .expect("sync");

    let root = parse_playlist(&device);

    assert!(!root.iter().any(|f| f["uuid"] == "stale-category-uuid"));
    assert!(
        all_titles(&root).contains(&"Histoires".to_string()),
        "une catégorie de repli (nouvel uuid) doit tout de même être créée"
    );
}

#[tokio::test]
async fn all_occurrences_of_deleted_uuid_are_removed_even_if_duplicated_elsewhere_in_tree() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "cat-1", "Documentaires"),
        (3, 2, "dup-ep", "Episode Duplique"),
        (4, 1, "dernier-ajouts", "Dernier ajouts"),
        (5, 4, "dup-ep", "Episode Duplique"),
    ]);
    let device = start_device(&[("playlist.bin", existing_bin), ("dup-ep.mp3", vec![0x01])]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let sync_request = SyncRequest {
        files_to_delete: vec!["dup-ep.mp3".into()],
        ..Default::default()
    };
    engine
        .sync(&sync_request, &SyncCallbacks::silent())
        .await
        .expect("sync");

    assert!(
        !all_titles(&parse_playlist(&device)).contains(&"Episode Duplique".to_string()),
        "aucune des deux occurrences ne doit survivre"
    );
}

#[tokio::test]
async fn a_dangling_audio_reference_makes_the_sync_fail_loudly() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "cat-1", "Categorie"),
        (3, 2, "orphan-ref", "Reference Fantome"),
    ]);
    let jpg = vec![0xFFu8, 0xD8, 0xFF];
    let device = start_device(&[
        ("playlist.bin", existing_bin),
        ("cat-1.jpg", jpg.clone()),
        ("orphan-ref.jpg", jpg),
    ])
    .await;

    device.set_enforce_firmware_validation(true);
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let result = engine
        .sync(
            &request(vec![episode(EpisodeSpec::default())]),
            &SyncCallbacks::silent(),
        )
        .await;

    assert!(
        matches!(
            result,
            Err(EngineError::Sync(SyncError::UpdatePlaylistRejected(_)))
        ),
        "une référence audio fantôme doit faire échouer la synchro, obtenu {result:?}"
    );
}

#[tokio::test]
async fn freshly_uploaded_episode_is_not_incorrectly_pruned_by_integrity_check() {
    let device = start_device(&[]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let ep = episode(EpisodeSpec {
        episode_uuid: "ep-tout-neuf",
        episode_title: "Tout Neuf",
        ..Default::default()
    });

    engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await
        .expect("sync");

    assert!(all_titles(&parse_playlist(&device)).contains(&"Tout Neuf".to_string()));
}

#[tokio::test]
async fn existing_favorite_folder_is_reused_not_duplicated() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "existing-fav", "Merlin_favorite"),
        (3, 2, "fav-ep", "Episode Favori"),
    ]);
    let device = start_device(&[("playlist.bin", existing_bin), ("fav-ep.mp3", vec![1])]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    engine
        .sync(
            &request(vec![episode(EpisodeSpec::default())]),
            &SyncCallbacks::silent(),
        )
        .await
        .expect("sync");

    let root = parse_playlist(&device);
    let favorites: Vec<&Value> = root.iter().filter(|f| f["favorite"] == 1).collect();
    assert_eq!(
        favorites.len(),
        1,
        "il ne doit jamais y avoir 2 entrées favorite:1"
    );
    assert_eq!(
        favorites[0]["uuid"], "existing-fav",
        "la VRAIE entrée favorite existante doit être réutilisée"
    );
}

#[tokio::test]
async fn already_uploaded_episode_skips_file_transfer_but_still_appears_in_playlist() {
    let device = start_device(&[("ep-deja-la.mp3", vec![0x01, 0x02, 0x03])]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let ep = episode(EpisodeSpec {
        episode_uuid: "ep-deja-la",
        episode_title: "Deja La",
        already_uploaded: true,
        ..Default::default()
    });

    let synced = engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await
        .expect("sync");

    assert!(synced.is_empty());
    let (files, _) = device.snapshot();

    assert_eq!(
        files["ep-deja-la.mp3"],
        vec![0x01, 0x02, 0x03],
        "le fichier ne doit pas être re-envoyé"
    );
    assert!(all_titles(&parse_playlist(&device)).contains(&"Deja La".to_string()));
}

#[tokio::test]
async fn already_uploaded_episode_in_live_tree_is_not_duplicated_in_manifest() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "folder-1", "Podcast"),
        (3, 2, "ep-deja-la", "Deja La"),
    ]);
    let device = start_device(&[
        ("playlist.bin", existing_bin),
        ("ep-deja-la.mp3", vec![0x01, 0x02, 0x03]),
    ])
    .await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let ep = episode(EpisodeSpec {
        episode_uuid: "ep-deja-la",
        episode_title: "Deja La",
        already_uploaded: true,
        ..Default::default()
    });

    engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await
        .expect("sync");

    let root = parse_playlist(&device);
    let podcast_folder = find_folder("Podcast", &root).expect("dossier podcast manquant");
    let entries = podcast_folder["child"]
        .as_array()
        .map(|children| children.iter().filter(|c| c["title"] == "Deja La").count())
        .unwrap_or(0);
    assert_eq!(
        entries, 1,
        "un épisode déjà présent dans l'arbre live et toujours sélectionné ne doit apparaître qu'UNE seule fois"
    );
}

#[tokio::test]
async fn update_playlist_rejection_throws_descriptive_error() {
    let device = start_device(&[]).await;
    device.set_update_playlist_status(19);
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let result = engine
        .sync(
            &request(vec![episode(EpisodeSpec::default())]),
            &SyncCallbacks::silent(),
        )
        .await;
    match result {
        Err(EngineError::Sync(SyncError::UpdatePlaylistRejected(_))) => {}
        other => panic!("attendu UpdatePlaylistRejected, obtenu {other:?}"),
    }
}

#[tokio::test]
async fn files_uploaded_before_a_failure_are_cleaned_up_afterward() {
    let device = start_device(&[]).await;
    device.set_update_playlist_status(19);
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let ep = episode(EpisodeSpec {
        image: Some(make_image()),
        ..Default::default()
    });

    let result = engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await;
    assert!(result.is_err(), "attendu : échec (updatePlaylist rejeté)");

    let (files, deleted) = device.snapshot();
    assert!(
        deleted.contains(&"ep-1.mp3".to_string()),
        "le fichier uploadé avant l'échec doit être nettoyé"
    );
    assert!(
        deleted.contains(&"ep-1.jpg".to_string()),
        "l'image uploadée avant l'échec doit être nettoyée"
    );
    assert!(
        !files.contains_key("ep-1.mp3"),
        "ne doit plus être présent après nettoyage"
    );
    assert!(!files.contains_key("ep-1.jpg"));
}

#[tokio::test]
async fn rejected_update_playlist_leaves_files_marked_for_deletion_untouched() {
    let existing_bin = make_playlist_bin(&[
        (1, 0, "", "Root"),
        (2, 1, "folder-a", "Podcast"),
        (3, 2, "ep-a-supprimer", "A Supprimer"),
    ]);
    let device = start_device(&[
        ("playlist.bin", existing_bin),
        ("ep-a-supprimer.mp3", vec![0x01, 0x02, 0x03]),
    ])
    .await;
    device.set_update_playlist_status(19);
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let sync_request = SyncRequest {
        files_to_delete: vec!["ep-a-supprimer.mp3".into()],
        ..Default::default()
    };
    let result = engine.sync(&sync_request, &SyncCallbacks::silent()).await;
    assert!(result.is_err(), "attendu : échec (updatePlaylist rejeté)");

    let (files, deleted) = device.snapshot();
    assert!(
        !deleted.contains(&"ep-a-supprimer.mp3".to_string()),
        "un rejet d'updatePlaylist ne doit PAS avoir appliqué la suppression"
    );
    assert_eq!(
        files["ep-a-supprimer.mp3"],
        vec![0x01, 0x02, 0x03],
        "le fichier doit rester intact"
    );
}

#[tokio::test]
async fn send_file_rejection_throws_descriptive_error() {
    let device = start_device(&[]).await;
    device.set_send_file_ready_status_override(Some(0x02));
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let result = engine
        .sync(
            &request(vec![episode(EpisodeSpec::default())]),
            &SyncCallbacks::silent(),
        )
        .await;
    match result {
        Err(EngineError::Sync(SyncError::FolderImageUploadFailed {
            folder_title,
            underlying,
        })) => {
            assert_eq!(folder_title, "Podcast");
            assert!(
                underlying.to_lowercase().contains("insuffisant"),
                "la raison originale de l'enceinte doit rester lisible : {underlying}"
            );
        }
        other => panic!("attendu FolderImageUploadFailed, obtenu {other:?}"),
    }
}

#[tokio::test]
async fn category_and_podcast_nesting_stays_within_three_levels() {
    let device = start_device(&[]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let ep = episode(EpisodeSpec {
        category_title: "Categorie",
        ..Default::default()
    });

    engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await
        .expect("sync");

    let root = parse_playlist(&device);
    let category = find_folder("Categorie", &root).expect("catégorie manquante");
    let category_children = category["child"].as_array().expect("enfants de catégorie");
    let podcast_folder =
        find_folder("Podcast", category_children).expect("dossier podcast manquant");
    let podcast_children = podcast_folder["child"]
        .as_array()
        .expect("enfants du podcast");

    assert!(
        podcast_children.iter().all(|c| c.get("child").is_none()),
        "aucun 4e niveau ne doit exister sous catégorie -> podcast -> épisode"
    );
    assert!(podcast_children.iter().any(|c| c["title"] == "Episode"));
}

#[tokio::test]
async fn repair_files_uploads_exact_names_without_touching_playlist() {
    let device = start_device(&[]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let image_file = make_image();
    let audio_file = make_audio();

    engine
        .repair_files(
            &[
                RepairFile {
                    local_path: image_file.clone(),
                    remote_name: "8a251a87-0000-0000-0000-000000000001.jpg".into(),
                },
                RepairFile {
                    local_path: audio_file.clone(),
                    remote_name: "8a251a87-0000-0000-0000-000000000001.mp3".into(),
                },
            ],
            &SyncCallbacks::silent(),
        )
        .await
        .expect("repair");

    let (files, _) = device.snapshot();
    assert_eq!(
        files["8a251a87-0000-0000-0000-000000000001.jpg"],
        std::fs::read(&image_file).unwrap()
    );
    assert_eq!(
        files["8a251a87-0000-0000-0000-000000000001.mp3"],
        std::fs::read(&audio_file).unwrap()
    );
    assert!(
        device.received_playlist_json().is_none(),
        "repair_files ne doit jamais toucher à playlist.json"
    );
}

#[tokio::test]
async fn repair_files_skips_already_present_file() {
    let device =
        start_device(&[("8a251a87-0000-0000-0000-000000000001.jpg", vec![0xAA, 0xBB])]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    engine
        .repair_files(
            &[RepairFile {
                local_path: make_image(),
                remote_name: "8a251a87-0000-0000-0000-000000000001.jpg".into(),
            }],
            &SyncCallbacks::silent(),
        )
        .await
        .expect("repair");

    let (files, _) = device.snapshot();

    assert_eq!(
        files["8a251a87-0000-0000-0000-000000000001.jpg"],
        vec![0xAA, 0xBB]
    );
}

#[tokio::test]
async fn malformed_playlist_bin_aborts_sync_instead_of_continuing_with_empty_tree() {
    let device = start_device(&[("playlist.bin", vec![0xAB; 100])]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());

    let result = engine
        .sync(
            &request(vec![episode(EpisodeSpec::default())]),
            &SyncCallbacks::silent(),
        )
        .await;
    match result {
        Err(EngineError::Sync(SyncError::CorruptPlaylistBin { .. })) => {}
        other => panic!("attendu CorruptPlaylistBin, obtenu {other:?}"),
    }
}

#[tokio::test]
async fn image_is_reuploaded_only_when_its_content_changes() {
    let device = start_device(&[]).await;
    let mut engine = SyncEngine::new("127.0.0.1", device.port());
    let img = make_image();

    let ep = episode(EpisodeSpec {
        image: Some(img.clone()),
        ..Default::default()
    });
    engine
        .sync(&request(vec![ep]), &SyncCallbacks::silent())
        .await
        .expect("sync 1");
    let recorded: HashMap<String, String> = engine
        .uploaded_image_fingerprints()
        .iter()
        .cloned()
        .collect();
    assert!(
        recorded.contains_key("ep-1.jpg"),
        "l'image est téléversée au 1er sync"
    );

    let ep_same = episode(EpisodeSpec {
        image: Some(img.clone()),
        already_uploaded: true,
        ..Default::default()
    });
    let mut req_same = request(vec![ep_same]);
    req_same.image_fingerprints = recorded.clone();
    engine
        .sync(&req_same, &SyncCallbacks::silent())
        .await
        .expect("sync 2");
    assert!(
        !engine
            .uploaded_image_fingerprints()
            .iter()
            .any(|(name, _)| name == "ep-1.jpg"),
        "image inchangée : aucun re-téléversement"
    );

    let new_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 9, 8, 7, 6, 5, 4, 3, 2, 1];
    let changed = make_temp_file("episode.jpg", &new_bytes);
    let ep_changed = episode(EpisodeSpec {
        image: Some(changed),
        already_uploaded: true,
        ..Default::default()
    });
    let mut req_changed = request(vec![ep_changed]);
    req_changed.image_fingerprints = recorded;
    engine
        .sync(&req_changed, &SyncCallbacks::silent())
        .await
        .expect("sync 3");
    assert!(
        engine
            .uploaded_image_fingerprints()
            .iter()
            .any(|(name, _)| name == "ep-1.jpg"),
        "image changée : re-téléversée"
    );
    let (files, _) = device.snapshot();
    assert_eq!(
        files.get("ep-1.jpg"),
        Some(&new_bytes),
        "l'enceinte porte le nouveau visuel"
    );
}
