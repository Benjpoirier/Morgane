use super::model::{PlaylistFolder, PlaylistNode};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TreeEdit {
    RenamedFolder {
        uuid: String,
        new_title: String,
    },

    RenamedSound {
        uuid: String,
        new_title: String,
    },

    Moved {
        uuid: String,
        to_parent_uuid: String,
    },

    Removed {
        uuid: String,
    },
}

impl TreeEdit {
    pub fn node_uuid(&self) -> &str {
        match self {
            TreeEdit::RenamedFolder { uuid, .. }
            | TreeEdit::RenamedSound { uuid, .. }
            | TreeEdit::Moved { uuid, .. }
            | TreeEdit::Removed { uuid } => uuid,
        }
    }

    pub fn targets_same_node_and_kind(&self, other: &TreeEdit) -> bool {
        match (self, other) {
            (TreeEdit::Removed { .. }, _) | (_, TreeEdit::Removed { .. }) => {
                self.node_uuid() == other.node_uuid()
            }
            (TreeEdit::RenamedFolder { uuid: a, .. }, TreeEdit::RenamedFolder { uuid: b, .. }) => {
                a == b
            }
            (TreeEdit::RenamedSound { uuid: a, .. }, TreeEdit::RenamedSound { uuid: b, .. }) => {
                a == b
            }
            (TreeEdit::Moved { uuid: a, .. }, TreeEdit::Moved { uuid: b, .. }) => a == b,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveRejectionReason {
    SameNode,
    DestinationNotFound,
    DestinationNotARootCategory,
    DestinationIsSynthetic,
    WouldCreateCycle,
    NodeNotFound,
}

pub fn find_folder<'a>(uuid: &str, folders: &'a [PlaylistFolder]) -> Option<&'a PlaylistFolder> {
    for folder in folders {
        if folder.uuid == uuid {
            return Some(folder);
        }
        if let Some(found) = find_folder_in_children(uuid, &folder.children) {
            return Some(found);
        }
    }
    None
}

fn find_folder_in_children<'a>(
    uuid: &str,
    children: &'a [PlaylistNode],
) -> Option<&'a PlaylistFolder> {
    for child in children {
        if let PlaylistNode::Folder(folder) = child {
            if folder.uuid == uuid {
                return Some(folder);
            }
            if let Some(found) = find_folder_in_children(uuid, &folder.children) {
                return Some(found);
            }
        }
    }
    None
}

pub fn find_folder_mut<'a>(
    uuid: &str,
    folders: &'a mut [PlaylistFolder],
) -> Option<&'a mut PlaylistFolder> {
    for folder in folders {
        if folder.uuid == uuid {
            return Some(folder);
        }
        if let Some(found) = find_folder_mut_in_children(uuid, &mut folder.children) {
            return Some(found);
        }
    }
    None
}

fn find_folder_mut_in_children<'a>(
    uuid: &str,
    children: &'a mut [PlaylistNode],
) -> Option<&'a mut PlaylistFolder> {
    for child in children {
        if let PlaylistNode::Folder(folder) = child {
            if folder.uuid == uuid {
                return Some(folder);
            }
            if let Some(found) = find_folder_mut_in_children(uuid, &mut folder.children) {
                return Some(found);
            }
        }
    }
    None
}

pub fn contains_folder(target_uuid: &str, children: &[PlaylistNode]) -> bool {
    for child in children {
        if let PlaylistNode::Folder(folder) = child {
            if folder.uuid == target_uuid {
                return true;
            }
            if contains_folder(target_uuid, &folder.children) {
                return true;
            }
        }
    }
    false
}

pub fn remove_node(uuid: &str, folders: &mut [PlaylistFolder]) -> Option<PlaylistNode> {
    for folder in folders {
        if let Some(removed) = remove_node_within(uuid, folder) {
            return Some(removed);
        }
    }
    None
}

fn remove_node_within(uuid: &str, folder: &mut PlaylistFolder) -> Option<PlaylistNode> {
    if let Some(removed) = folder.remove_child(uuid) {
        return Some(removed);
    }
    for child in &mut folder.children {
        if let PlaylistNode::Folder(subfolder) = child
            && let Some(removed) = remove_node_within(uuid, subfolder)
        {
            return Some(removed);
        }
    }
    None
}

pub fn rename_sound(uuid: &str, new_title: &str, folders: &mut [PlaylistFolder]) -> bool {
    for folder in folders {
        if rename_sound_within(uuid, new_title, folder) {
            return true;
        }
    }
    false
}

fn rename_sound_within(uuid: &str, new_title: &str, folder: &mut PlaylistFolder) -> bool {
    if folder.children.iter().any(|child| child.uuid() == uuid) {
        folder.rename_sound(uuid, new_title);
        return true;
    }
    for child in &mut folder.children {
        if let PlaylistNode::Folder(subfolder) = child
            && rename_sound_within(uuid, new_title, subfolder)
        {
            return true;
        }
    }
    false
}

pub fn validate_move(
    uuid: &str,
    destination_uuid: &str,
    root_folders: &[PlaylistFolder],
) -> Option<MoveRejectionReason> {
    if uuid == destination_uuid {
        return Some(MoveRejectionReason::SameNode);
    }
    let Some(destination) = find_folder(destination_uuid, root_folders) else {
        return Some(MoveRejectionReason::DestinationNotFound);
    };
    if destination.is_synthetic {
        return Some(MoveRejectionReason::DestinationIsSynthetic);
    }
    if !root_folders.iter().any(|f| f.uuid == destination.uuid) {
        return Some(MoveRejectionReason::DestinationNotARootCategory);
    }
    if let Some(dragged) = find_folder(uuid, root_folders)
        && contains_folder(&destination.uuid, &dragged.children)
    {
        return Some(MoveRejectionReason::WouldCreateCycle);
    }
    if find_folder(uuid, root_folders).is_none() && !find_sound(uuid, root_folders) {
        return Some(MoveRejectionReason::NodeNotFound);
    }
    None
}

fn find_sound(uuid: &str, folders: &[PlaylistFolder]) -> bool {
    for folder in folders {
        if folder.children.iter().any(|child| child.uuid() == uuid) {
            return true;
        }
        for child in &folder.children {
            if let PlaylistNode::Folder(subfolder) = child
                && find_sound(uuid, std::slice::from_ref(subfolder))
            {
                return true;
            }
        }
    }
    false
}

pub fn apply(edits: &[TreeEdit], root_folders: &mut [PlaylistFolder]) -> Vec<String> {
    let mut messages = Vec::new();
    for edit in edits {
        match edit {
            TreeEdit::RenamedFolder { uuid, new_title } => {
                match find_folder_mut(uuid, root_folders) {
                    Some(folder) => folder.title = new_title.clone(),
                    None => messages.push(format!(
                        "Edition locale ignoree : dossier introuvable dans l'arbre de l'enceinte (uuid {uuid}, probablement supprime via l'app officielle) - renommage abandonne."
                    )),
                }
            }
            TreeEdit::RenamedSound { uuid, new_title } => {
                if !rename_sound(uuid, new_title, root_folders) {
                    messages.push(format!(
                        "Edition locale ignoree : episode introuvable dans l'arbre de l'enceinte (uuid {uuid}, probablement supprime via l'app officielle) - renommage abandonne."
                    ));
                }
            }
            TreeEdit::Moved {
                uuid,
                to_parent_uuid,
            } => {
                if let Some(rejection) = validate_move(uuid, to_parent_uuid, root_folders) {
                    match rejection {
                        MoveRejectionReason::SameNode => {}
                        MoveRejectionReason::DestinationNotFound
                        | MoveRejectionReason::DestinationNotARootCategory
                        | MoveRejectionReason::DestinationIsSynthetic => messages.push(format!(
                            "Edition locale ignoree : dossier de destination introuvable dans l'arbre de l'enceinte (uuid {to_parent_uuid}, probablement supprime via l'app officielle) - deplacement abandonne."
                        )),
                        MoveRejectionReason::WouldCreateCycle => messages.push(format!(
                            "Edition locale ignoree : deplacement de {uuid} vers son propre sous-arbre (cycle) - abandonne."
                        )),
                        MoveRejectionReason::NodeNotFound => messages.push(format!(
                            "Edition locale ignoree : noeud introuvable dans l'arbre de l'enceinte (uuid {uuid}, probablement supprime via l'app officielle) - deplacement abandonne."
                        )),
                    }
                    continue;
                }

                if let Some(node) = remove_node(uuid, root_folders)
                    && let Some(destination) = find_folder_mut(to_parent_uuid, root_folders)
                {
                    destination.children.push(node);
                }
            }
            TreeEdit::Removed { uuid } => {
                if remove_node(uuid, root_folders).is_none() {
                    messages.push(format!(
                        "Edition locale ignoree : noeud a supprimer introuvable dans l'arbre de l'enceinte (uuid {uuid}, probablement deja supprime) - suppression du menu abandonnee."
                    ));
                }
            }
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LiveTree {
        root_folders: Vec<PlaylistFolder>,
        category_uuid: &'static str,
        podcast_uuid: &'static str,
        sound_uuid: &'static str,
        other_category_uuid: &'static str,
    }

    fn make_live_tree() -> LiveTree {
        let category_uuid = "cat-histoires";
        let podcast_uuid = "pod-mystere";
        let sound_uuid = "sound-episode-1";
        let other_category_uuid = "cat-autre";

        let mut category = PlaylistFolder::new(category_uuid, "Histoires");
        let mut podcast = PlaylistFolder::new(podcast_uuid, "Mystere a l'ecole");
        podcast.add_sound(sound_uuid, "Episode 1");
        category.children = vec![PlaylistNode::Folder(podcast)];

        let other = PlaylistFolder::new(other_category_uuid, "Autre");

        LiveTree {
            root_folders: vec![category, other],
            category_uuid,
            podcast_uuid,
            sound_uuid,
            other_category_uuid,
        }
    }

    #[test]
    fn applies_folder_rename_and_move_against_live_tree() {
        let mut tree = make_live_tree();

        let edits = vec![
            TreeEdit::RenamedFolder {
                uuid: tree.podcast_uuid.into(),
                new_title: "Mystere a l'ecole (renomme)".into(),
            },
            TreeEdit::RenamedSound {
                uuid: tree.sound_uuid.into(),
                new_title: "Episode 1 (renomme)".into(),
            },
            TreeEdit::Moved {
                uuid: tree.podcast_uuid.into(),
                to_parent_uuid: tree.other_category_uuid.into(),
            },
        ];

        let messages = apply(&edits, &mut tree.root_folders);
        assert_eq!(
            messages,
            Vec::<String>::new(),
            "aucune de ces 3 éditions ne devrait échouer contre cet arbre"
        );

        let histoires = tree
            .root_folders
            .iter()
            .find(|f| f.uuid == tree.category_uuid)
            .unwrap();
        assert!(
            histoires.children.is_empty(),
            "le podcast déplacé ne doit plus être un enfant de la catégorie d'origine"
        );

        let autre = tree
            .root_folders
            .iter()
            .find(|f| f.uuid == tree.other_category_uuid)
            .unwrap();
        assert_eq!(autre.children.len(), 1);
        let PlaylistNode::Folder(moved_podcast) = &autre.children[0] else {
            panic!("l'enfant déplacé attendu est un dossier");
        };
        assert_eq!(moved_podcast.uuid, tree.podcast_uuid);
        assert_eq!(moved_podcast.title, "Mystere a l'ecole (renomme)");
        assert_eq!(moved_podcast.children.len(), 1);
        let PlaylistNode::Sound { uuid, title } = &moved_podcast.children[0] else {
            panic!("l'enfant du podcast déplacé attendu est un son");
        };
        assert_eq!(uuid, tree.sound_uuid);
        assert_eq!(title, "Episode 1 (renomme)");
    }

    #[test]
    fn missing_target_is_skipped_gracefully_with_log_message() {
        let mut tree = make_live_tree();

        let messages = apply(
            &[TreeEdit::RenamedFolder {
                uuid: "uuid-disparu".into(),
                new_title: "Peu importe".into(),
            }],
            &mut tree.root_folders,
        );

        assert_eq!(messages.len(), 1);
        let histoires = tree
            .root_folders
            .iter()
            .find(|f| f.uuid == tree.category_uuid)
            .unwrap();
        assert_eq!(
            histoires.children.len(),
            1,
            "l'arbre ne doit pas avoir bougé suite à un edit introuvable"
        );
    }

    #[test]
    fn reapplying_same_rename_is_harmless_no_op() {
        let mut tree = make_live_tree();
        let rename = TreeEdit::RenamedFolder {
            uuid: tree.podcast_uuid.into(),
            new_title: "Nouveau titre".into(),
        };

        apply(std::slice::from_ref(&rename), &mut tree.root_folders);
        let messages = apply(std::slice::from_ref(&rename), &mut tree.root_folders);

        assert_eq!(messages, Vec::<String>::new());
        let podcast = find_folder(tree.podcast_uuid, &tree.root_folders);
        assert_eq!(podcast.map(|f| f.title.as_str()), Some("Nouveau titre"));
    }

    #[test]
    fn validate_move_accepts_move_to_real_root_category() {
        let tree = make_live_tree();
        assert_eq!(
            validate_move(
                tree.podcast_uuid,
                tree.other_category_uuid,
                &tree.root_folders
            ),
            None
        );
    }

    #[test]
    fn validate_move_rejects_same_node() {
        let tree = make_live_tree();
        assert_eq!(
            validate_move(tree.category_uuid, tree.category_uuid, &tree.root_folders),
            Some(MoveRejectionReason::SameNode)
        );
    }

    #[test]
    fn validate_move_rejects_destination_not_found() {
        let tree = make_live_tree();
        assert_eq!(
            validate_move(tree.podcast_uuid, "uuid-disparu", &tree.root_folders),
            Some(MoveRejectionReason::DestinationNotFound)
        );
    }

    #[test]
    fn validate_move_rejects_destination_that_is_not_a_root_category() {
        let tree = make_live_tree();
        assert_eq!(
            validate_move("whatever", tree.podcast_uuid, &tree.root_folders),
            Some(MoveRejectionReason::DestinationNotARootCategory)
        );
    }

    #[test]
    fn validate_move_rejects_synthetic_destination() {
        let tree = make_live_tree();
        let mut synthetic = PlaylistFolder::new("synth-1", "En attente");
        synthetic.is_synthetic = true;
        let mut live_with_synthetic = tree.root_folders.clone();
        live_with_synthetic.push(synthetic);
        assert_eq!(
            validate_move(tree.podcast_uuid, "synth-1", &live_with_synthetic),
            Some(MoveRejectionReason::DestinationIsSynthetic)
        );
    }

    #[test]
    fn validate_move_rejects_cycle_with_duplicated_folder_reference() {
        let b = PlaylistFolder::new("cat-b", "B");
        let mut a = PlaylistFolder::new("cat-a", "A");
        a.children = vec![PlaylistNode::Folder(b.clone())];
        let root_folders = vec![a, b];

        assert_eq!(
            validate_move("cat-a", "cat-b", &root_folders),
            Some(MoveRejectionReason::WouldCreateCycle)
        );
    }

    #[test]
    fn targets_same_node_and_kind_matches_only_same_kind_and_uuid() {
        let rename_a = TreeEdit::RenamedFolder {
            uuid: "a".into(),
            new_title: "X".into(),
        };
        let rename_a2 = TreeEdit::RenamedFolder {
            uuid: "a".into(),
            new_title: "Y".into(),
        };
        let rename_sound_a = TreeEdit::RenamedSound {
            uuid: "a".into(),
            new_title: "X".into(),
        };
        let move_a = TreeEdit::Moved {
            uuid: "a".into(),
            to_parent_uuid: "b".into(),
        };

        assert!(rename_a.targets_same_node_and_kind(&rename_a2));
        assert!(!rename_a.targets_same_node_and_kind(&rename_sound_a));
        assert!(!rename_a.targets_same_node_and_kind(&move_a));
    }
}
