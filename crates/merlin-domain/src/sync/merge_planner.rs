use std::collections::HashMap;
use std::path::PathBuf;

use crate::library::deterministic_uuid;
use crate::library::manual_category::ManualCategory;
use crate::playlist::model::{PlaylistFolder, PlaylistNode};

use super::types::{EpisodeToSync, SyncedEpisode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderImageNeeded {
    pub folder_uuid: String,
    pub folder_title: String,
    pub image_url: Option<String>,
    pub fallback_local_image_path: Option<PathBuf>,
    pub use_bundled_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub root_folders: Vec<PlaylistFolder>,
    pub folder_images_needed: Vec<FolderImageNeeded>,
    pub synced: Vec<SyncedEpisode>,

    pub warnings: Vec<String>,
}

enum PodcastLocation {
    Root(usize),

    InCategory { category_index: usize },
}

pub fn plan(
    episodes: &[EpisodeToSync],
    live_root_folders: Vec<PlaylistFolder>,
    folder_image_overrides: &HashMap<String, String>,
    manual_categories: &[ManualCategory],
) -> Plan {
    let mut root_folders = live_root_folders;
    let mut podcast_locations: HashMap<String, PodcastLocation> = HashMap::new();
    let mut synced: Vec<SyncedEpisode> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut folder_images_needed: Vec<FolderImageNeeded> = Vec::new();

    for manual_category in manual_categories {
        if !root_folders.iter().any(|f| f.uuid == manual_category.uuid) {
            root_folders.push(PlaylistFolder::new(
                &manual_category.uuid,
                &manual_category.title,
            ));
        }

        folder_images_needed.push(FolderImageNeeded {
            folder_uuid: manual_category.uuid.clone(),
            folder_title: manual_category.title.clone(),
            image_url: non_empty(&manual_category.image_source),
            fallback_local_image_path: None,
            use_bundled_fallback: false,
        });
    }

    for episode in episodes {
        let has_category = episode.category_uuid.is_some() || !episode.category_title.is_empty();

        let target_location = if let Some(location) = podcast_locations.get(&episode.folder_uuid) {
            match location {
                PodcastLocation::Root(index) => PodcastLocation::Root(*index),
                PodcastLocation::InCategory { category_index } => PodcastLocation::InCategory {
                    category_index: *category_index,
                },
            }
        } else if has_category {
            let category_uuid = match &episode.category_uuid {
                Some(explicit_uuid) if root_folders.iter().any(|f| f.uuid == *explicit_uuid) => {
                    explicit_uuid.clone()
                }
                maybe_stale => {
                    if maybe_stale.is_some() {
                        warnings.push(format!(
                            "L'assignation de categorie de \"{}\" ne correspond plus a une categorie existante sur l'enceinte (probablement supprimee via l'app officielle) - repli sur une categorie synthetisee depuis son titre plutot que de la recreer avec son ancien uuid.",
                            episode.folder_title
                        ));
                    }
                    deterministic_uuid::from_prefixed_name(&format!(
                        "merlinsync-category:{}",
                        episode.category_title
                    ))
                }
            };
            let category_index = match root_folders.iter().position(|f| f.uuid == category_uuid) {
                Some(index) => index,
                None => {
                    root_folders.push(PlaylistFolder::new(&category_uuid, &episode.category_title));
                    folder_images_needed.push(FolderImageNeeded {
                        folder_uuid: category_uuid.clone(),
                        folder_title: episode.category_title.clone(),
                        image_url: None,
                        fallback_local_image_path: None,
                        use_bundled_fallback: true,
                    });
                    root_folders.len() - 1
                }
            };
            root_folders[category_index]
                .get_or_create_subfolder(&episode.folder_uuid, &episode.folder_title);
            podcast_locations.insert(
                episode.folder_uuid.clone(),
                PodcastLocation::InCategory { category_index },
            );
            folder_images_needed.push(FolderImageNeeded {
                folder_uuid: episode.folder_uuid.clone(),
                folder_title: episode.folder_title.clone(),
                image_url: episode.folder_image_url.clone(),
                fallback_local_image_path: episode.image_path.clone(),
                use_bundled_fallback: false,
            });
            PodcastLocation::InCategory { category_index }
        } else {
            root_folders.push(PlaylistFolder::new(
                &episode.folder_uuid,
                &episode.folder_title,
            ));
            let index = root_folders.len() - 1;
            podcast_locations.insert(episode.folder_uuid.clone(), PodcastLocation::Root(index));
            folder_images_needed.push(FolderImageNeeded {
                folder_uuid: episode.folder_uuid.clone(),
                folder_title: episode.folder_title.clone(),
                image_url: episode.folder_image_url.clone(),
                fallback_local_image_path: episode.image_path.clone(),
                use_bundled_fallback: false,
            });
            PodcastLocation::Root(index)
        };

        let target_folder = match &target_location {
            PodcastLocation::Root(index) => &mut root_folders[*index],
            PodcastLocation::InCategory { category_index } => root_folders[*category_index]
                .get_or_create_subfolder(&episode.folder_uuid, &episode.folder_title),
        };
        target_folder.add_sound(&episode.episode_uuid, &episode.episode_title);

        if !episode.already_uploaded {
            synced.push(SyncedEpisode {
                episode_uuid: episode.episode_uuid.clone(),
                title: episode.episode_title.clone(),
                folder_title: episode.folder_title.clone(),
            });
        }
    }

    if !root_folders.iter().any(|f| f.is_favorite) {
        let mut favorite_folder = PlaylistFolder::new(
            deterministic_uuid::from_prefixed_name("merlinsync-favorites"),
            "Favoris",
        );
        favorite_folder.is_favorite = true;
        folder_images_needed.push(FolderImageNeeded {
            folder_uuid: favorite_folder.uuid.clone(),
            folder_title: favorite_folder.title.clone(),
            image_url: None,
            fallback_local_image_path: None,
            use_bundled_fallback: true,
        });
        root_folders.push(favorite_folder);
    }

    let mut folder_images_needed: Vec<FolderImageNeeded> = folder_images_needed
        .into_iter()
        .map(|needed| {
            let Some(override_source) = folder_image_overrides
                .get(&needed.folder_uuid)
                .and_then(|s| non_empty(s))
            else {
                return needed;
            };
            FolderImageNeeded {
                folder_uuid: needed.folder_uuid,
                folder_title: needed.folder_title,
                image_url: Some(override_source),
                fallback_local_image_path: None,
                use_bundled_fallback: false,
            }
        })
        .collect();

    let already_scheduled: std::collections::HashSet<String> = folder_images_needed
        .iter()
        .map(|n| n.folder_uuid.clone())
        .collect();
    let folder_titles = collect_folder_titles(&root_folders);
    for (folder_uuid, source) in folder_image_overrides {
        if already_scheduled.contains(folder_uuid) {
            continue;
        }
        let Some(override_source) = non_empty(source) else {
            continue;
        };
        if let Some(folder_title) = folder_titles.get(folder_uuid) {
            folder_images_needed.push(FolderImageNeeded {
                folder_uuid: folder_uuid.clone(),
                folder_title: folder_title.clone(),
                image_url: Some(override_source),
                fallback_local_image_path: None,
                use_bundled_fallback: false,
            });
        }
    }

    Plan {
        root_folders,
        folder_images_needed,
        synced,
        warnings,
    }
}

fn collect_folder_titles(folders: &[PlaylistFolder]) -> HashMap<String, String> {
    let mut titles = HashMap::new();
    fn visit(folder: &PlaylistFolder, titles: &mut HashMap<String, String>) {
        titles.insert(folder.uuid.clone(), folder.title.clone());
        for child in &folder.children {
            if let PlaylistNode::Folder(subfolder) = child {
                visit(subfolder, titles);
            }
        }
    }
    for folder in folders {
        visit(folder, &mut titles);
    }
    titles
}

fn non_empty(source: &str) -> Option<String> {
    if source.is_empty() {
        None
    } else {
        Some(source.to_string())
    }
}

pub fn prune_dangling_sounds(
    folders: &mut [PlaylistFolder],
    existing_files: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut removed_titles = Vec::new();
    for folder in folders {
        folder.children.retain(|child| match child {
            PlaylistNode::Sound { uuid, title } => {
                let present = existing_files.contains(&format!("{uuid}.mp3"))
                    || existing_files.contains(&format!("{uuid}.aac"));
                if !present {
                    removed_titles.push(title.clone());
                }
                present
            }
            PlaylistNode::Folder(_) => true,
        });
        for child in &mut folder.children {
            if let PlaylistNode::Folder(subfolder) = child {
                removed_titles.extend(prune_dangling_sounds(
                    std::slice::from_mut(subfolder),
                    existing_files,
                ));
            }
        }
    }
    removed_titles
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    struct EpisodeSpec {
        folder_uuid: &'static str,
        folder_title: &'static str,
        episode_uuid: &'static str,
        episode_title: &'static str,
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
                category_title: "",
                category_uuid: None,
                already_uploaded: false,
            }
        }
    }

    fn make_episode(spec: EpisodeSpec) -> EpisodeToSync {
        EpisodeToSync {
            folder_uuid: spec.folder_uuid.into(),
            folder_title: spec.folder_title.into(),
            episode_uuid: spec.episode_uuid.into(),
            episode_title: spec.episode_title.into(),
            audio_path: PathBuf::from("/tmp/a.mp3"),
            image_path: None,
            category_title: spec.category_title.into(),
            category_uuid: spec.category_uuid.map(String::from),
            folder_image_url: None,
            already_uploaded: spec.already_uploaded,
            order: None,
        }
    }

    fn plan_simple(episodes: &[EpisodeToSync], live: Vec<PlaylistFolder>) -> Plan {
        plan(episodes, live, &HashMap::new(), &[])
    }

    #[test]
    fn episode_without_category_becomes_root_level_folder() {
        let result = plan_simple(&[make_episode(EpisodeSpec::default())], vec![]);
        let folder = result.root_folders.iter().find(|f| f.uuid == "folder-1");
        assert_eq!(folder.map(|f| f.title.as_str()), Some("Podcast"));
    }

    #[test]
    fn episode_with_category_nests_three_levels_deep() {
        let episode = make_episode(EpisodeSpec {
            category_title: "Histoires",
            ..Default::default()
        });
        let result = plan_simple(&[episode], vec![]);

        let category = result
            .root_folders
            .iter()
            .find(|f| f.title == "Histoires")
            .expect("catégorie manquante");
        let PlaylistNode::Folder(podcast_folder) =
            category.children.first().expect("dossier podcast manquant")
        else {
            panic!("dossier podcast manquant");
        };
        assert_eq!(podcast_folder.uuid, "folder-1");
        assert!(
            podcast_folder
                .children
                .iter()
                .all(|c| matches!(c, PlaylistNode::Sound { .. }))
        );
    }

    #[test]
    fn stale_category_uuid_falls_back_and_emits_warning() {
        let episode = make_episode(EpisodeSpec {
            category_title: "Histoires",
            category_uuid: Some("stale-uuid"),
            ..Default::default()
        });
        let result = plan_simple(&[episode], vec![]);

        assert!(
            !result.root_folders.iter().any(|f| f.uuid == "stale-uuid"),
            "ne doit jamais ressusciter l'uuid périmé"
        );
        assert!(result.root_folders.iter().any(|f| f.title == "Histoires"));
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn existing_category_uuid_is_reused_without_warning() {
        let existing_category = PlaylistFolder::new("cat-1", "Histoires");
        let episode = make_episode(EpisodeSpec {
            category_title: "Histoires",
            category_uuid: Some("cat-1"),
            ..Default::default()
        });
        let result = plan_simple(&[episode], vec![existing_category]);

        assert!(result.warnings.is_empty());

        assert_eq!(
            result
                .root_folders
                .iter()
                .filter(|f| f.title == "Histoires")
                .count(),
            1,
            "ne doit pas créer une catégorie en double"
        );
        let category = result
            .root_folders
            .iter()
            .find(|f| f.uuid == "cat-1")
            .unwrap();
        let PlaylistNode::Folder(podcast_folder) = category
            .children
            .first()
            .expect("le podcast doit s'imbriquer dans la catégorie EXISTANTE")
        else {
            panic!("attendu un dossier");
        };
        assert_eq!(podcast_folder.uuid, "folder-1");
    }

    #[test]
    fn multiple_episodes_in_same_podcast_share_one_folder_and_one_image_request() {
        let episodes = vec![
            make_episode(EpisodeSpec {
                episode_uuid: "ep-1",
                episode_title: "Un",
                ..Default::default()
            }),
            make_episode(EpisodeSpec {
                episode_uuid: "ep-2",
                episode_title: "Deux",
                ..Default::default()
            }),
        ];
        let result = plan_simple(&episodes, vec![]);

        let folder = result
            .root_folders
            .iter()
            .find(|f| f.uuid == "folder-1")
            .expect("dossier manquant");
        assert_eq!(folder.children.len(), 2);
        assert_eq!(
            result
                .folder_images_needed
                .iter()
                .filter(|n| n.folder_uuid == "folder-1")
                .count(),
            1,
            "l'image de dossier ne doit être demandée qu'une fois"
        );
    }

    #[test]
    fn manual_category_is_created_even_without_any_episode_assigned_to_it() {
        let manual = ManualCategory {
            uuid: "manual-1".into(),
            title: "Contes".into(),
            image_source: "https://example.com/contes.jpg".into(),
        };
        let result = plan(&[], vec![], &HashMap::new(), &[manual]);

        let folder = result.root_folders.iter().find(|f| f.uuid == "manual-1");
        assert_eq!(folder.map(|f| f.title.as_str()), Some("Contes"));
        assert!(
            folder.map(|f| f.children.is_empty()).unwrap_or(false),
            "une catégorie manuelle jamais assignée reste vide, pas d'erreur"
        );
        let needed = result
            .folder_images_needed
            .iter()
            .find(|n| n.folder_uuid == "manual-1")
            .unwrap();
        assert_eq!(
            needed.image_url.as_deref(),
            Some("https://example.com/contes.jpg")
        );
        assert!(!needed.use_bundled_fallback);
    }

    #[test]
    fn manual_category_present_in_tree_is_not_duplicated_but_still_needs_its_image() {
        let existing = PlaylistFolder::new("manual-1", "Contes (deja synchronisee)");
        let manual = ManualCategory {
            uuid: "manual-1".into(),
            title: "Contes".into(),
            image_source: "https://example.com/contes.jpg".into(),
        };
        let result = plan(&[], vec![existing], &HashMap::new(), &[manual]);

        assert_eq!(
            result
                .root_folders
                .iter()
                .filter(|f| f.uuid == "manual-1")
                .count(),
            1
        );

        let needed = result
            .folder_images_needed
            .iter()
            .find(|n| n.folder_uuid == "manual-1");
        assert_eq!(
            needed.and_then(|n| n.image_url.as_deref()),
            Some("https://example.com/contes.jpg")
        );
    }

    #[test]
    fn image_override_on_an_existing_folder_schedules_an_upload() {
        let existing = PlaylistFolder::new("cat-1", "Histoires");
        let overrides = HashMap::from([(
            "cat-1".to_string(),
            "https://example.com/nouveau-visuel.jpg".to_string(),
        )]);

        let result = plan(&[], vec![existing], &overrides, &[]);

        let needed = result
            .folder_images_needed
            .iter()
            .find(|n| n.folder_uuid == "cat-1")
            .expect("un dossier existant avec override doit programmer un upload");
        assert_eq!(
            needed.image_url.as_deref(),
            Some("https://example.com/nouveau-visuel.jpg")
        );
        assert_eq!(needed.folder_title, "Histoires");
    }

    #[test]
    fn manual_category_image_override_takes_priority() {
        let manual = ManualCategory {
            uuid: "manual-1".into(),
            title: "Contes".into(),
            image_source: "https://example.com/contes.jpg".into(),
        };
        let overrides = HashMap::from([(
            "manual-1".to_string(),
            "https://example.com/override.jpg".to_string(),
        )]);
        let result = plan(&[], vec![], &overrides, &[manual]);

        let needed = result
            .folder_images_needed
            .iter()
            .find(|n| n.folder_uuid == "manual-1")
            .unwrap();
        assert_eq!(
            needed.image_url.as_deref(),
            Some("https://example.com/override.jpg")
        );
    }

    #[test]
    fn folder_image_override_takes_priority_over_natural_image_url() {
        let mut episode = make_episode(EpisodeSpec::default());
        episode.folder_image_url = Some("https://example.com/feed.jpg".into());
        let overrides = HashMap::from([(
            "folder-1".to_string(),
            "https://example.com/custom-override.jpg".to_string(),
        )]);
        let result = plan(&[episode], vec![], &overrides, &[]);

        let needed = result
            .folder_images_needed
            .iter()
            .find(|n| n.folder_uuid == "folder-1")
            .unwrap();
        assert_eq!(
            needed.image_url.as_deref(),
            Some("https://example.com/custom-override.jpg")
        );
        assert!(needed.fallback_local_image_path.is_none());
        assert!(!needed.use_bundled_fallback);
    }

    #[test]
    fn folder_image_override_for_unrelated_folder_does_not_affect_others() {
        let overrides = HashMap::from([(
            "some-other-folder".to_string(),
            "https://example.com/x.jpg".to_string(),
        )]);
        let result = plan(
            &[make_episode(EpisodeSpec::default())],
            vec![],
            &overrides,
            &[],
        );

        let needed = result
            .folder_images_needed
            .iter()
            .find(|n| n.folder_uuid == "folder-1")
            .unwrap();
        assert_ne!(
            needed.image_url.as_deref(),
            Some("https://example.com/x.jpg")
        );
    }

    #[test]
    fn favorite_folder_created_when_none_exists_in_live_tree() {
        let result = plan_simple(&[make_episode(EpisodeSpec::default())], vec![]);
        let favorites: Vec<&PlaylistFolder> = result
            .root_folders
            .iter()
            .filter(|f| f.is_favorite)
            .collect();
        assert_eq!(favorites.len(), 1);
        assert!(
            result
                .folder_images_needed
                .iter()
                .any(|n| n.folder_uuid == favorites[0].uuid && n.use_bundled_fallback)
        );
    }

    #[test]
    fn existing_favorite_folder_is_reused_not_duplicated() {
        let mut existing_favorite = PlaylistFolder::new("existing-fav", "Merlin_favorite");
        existing_favorite.is_favorite = true;
        let result = plan_simple(
            &[make_episode(EpisodeSpec::default())],
            vec![existing_favorite],
        );

        let favorites: Vec<&PlaylistFolder> = result
            .root_folders
            .iter()
            .filter(|f| f.is_favorite)
            .collect();
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].uuid, "existing-fav");
        assert!(
            !result
                .folder_images_needed
                .iter()
                .any(|n| n.folder_uuid == "existing-fav"),
            "pas de nouvelle image demandée pour un favori déjà existant"
        );
    }

    #[test]
    fn already_uploaded_episode_is_not_included_in_synced_but_still_added_to_tree() {
        let episode = make_episode(EpisodeSpec {
            already_uploaded: true,
            ..Default::default()
        });
        let result = plan_simple(&[episode], vec![]);

        assert!(result.synced.is_empty());
        let folder = result
            .root_folders
            .iter()
            .find(|f| f.uuid == "folder-1")
            .unwrap();
        assert!(folder.children.iter().any(|c| c.uuid() == "ep-1"));
    }

    #[test]
    fn prune_dangling_sounds_removes_reference_without_matching_audio_file() {
        let mut folder = PlaylistFolder::new("f", "Dossier");
        folder.add_sound("has-file", "Present");
        folder.add_sound("no-file", "Fantome");
        let mut folders = vec![folder];

        let removed =
            prune_dangling_sounds(&mut folders, &HashSet::from(["has-file.mp3".to_string()]));

        assert_eq!(removed, ["Fantome"]);
        let uuids: Vec<&str> = folders[0].children.iter().map(|c| c.uuid()).collect();
        assert_eq!(uuids, ["has-file"]);
    }

    #[test]
    fn prune_dangling_sounds_accepts_aac_extension_too() {
        let mut folder = PlaylistFolder::new("f", "Dossier");
        folder.add_sound("aac-file", "Present");
        let mut folders = vec![folder];

        let removed =
            prune_dangling_sounds(&mut folders, &HashSet::from(["aac-file.aac".to_string()]));

        assert!(removed.is_empty());
    }

    #[test]
    fn prune_dangling_sounds_recurses_into_subfolders() {
        let mut parent = PlaylistFolder::new("parent", "Parent");
        parent
            .get_or_create_subfolder("child", "Enfant")
            .add_sound("orphan", "Fantome");
        let mut folders = vec![parent];

        let removed = prune_dangling_sounds(&mut folders, &HashSet::new());

        assert_eq!(removed, ["Fantome"]);
        let PlaylistNode::Folder(child) = &folders[0].children[0] else {
            panic!("attendu un dossier");
        };
        assert!(child.children.is_empty());
    }
}
