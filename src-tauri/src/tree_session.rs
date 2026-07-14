use std::collections::HashSet;

use merlin_domain::library::manual_category::ManualCategory;
use merlin_domain::playlist::model::{PlaylistFolder, PlaylistNode};
use merlin_domain::playlist::tree_edit::{self, TreeEdit};
use merlin_infra::podcasts::image_converter::is_valid_uuid;

use crate::dto::{EditDetail, TreeView};

pub const ORPHAN_ROOT_UUID: &str = "merlinsync-fichiers-retrouves";

#[derive(Default)]
pub struct TreeSession {
    pub folders: Vec<PlaylistFolder>,
    pub pending_edits: Vec<TreeEdit>,
    pub pending_orphan_deletions: HashSet<String>,
    live_baseline: Vec<PlaylistFolder>,
    manual_categories: Vec<ManualCategory>,
}

impl TreeSession {
    pub fn view(&self) -> TreeView {
        let thumbnail_uuids = self
            .folders
            .iter()
            .filter(|f| !f.is_synthetic && is_valid_uuid(&f.uuid))
            .map(|f| f.uuid.clone())
            .collect();
        TreeView {
            folders: self.folders.clone(),
            pending_edits: self.pending_edits.clone(),
            edit_details: self.edit_details(),
            pending_orphan_deletions: self.pending_orphan_deletions.iter().cloned().collect(),
            thumbnail_uuids,
        }
    }

    fn edit_details(&self) -> Vec<EditDetail> {
        self.pending_edits
            .iter()
            .map(|edit| {
                let uuid = edit.node_uuid().to_string();
                match edit {
                    TreeEdit::RenamedFolder { new_title, .. } => EditDetail {
                        title: find_node_title(&uuid, &self.live_baseline).unwrap_or_default(),
                        new_title: Some(new_title.clone()),
                        dest_title: None,
                        kind: "renamedFolder".into(),
                        uuid,
                    },
                    TreeEdit::RenamedSound { new_title, .. } => EditDetail {
                        title: find_node_title(&uuid, &self.live_baseline).unwrap_or_default(),
                        new_title: Some(new_title.clone()),
                        dest_title: None,
                        kind: "renamedSound".into(),
                        uuid,
                    },
                    TreeEdit::Moved { to_parent_uuid, .. } => EditDetail {
                        title: find_node_title(&uuid, &self.folders).unwrap_or_default(),
                        new_title: None,
                        dest_title: find_node_title(to_parent_uuid, &self.folders),
                        kind: "moved".into(),
                        uuid,
                    },
                    TreeEdit::Removed { .. } => EditDetail {
                        title: find_node_title(&uuid, &self.live_baseline).unwrap_or_default(),
                        new_title: None,
                        dest_title: None,
                        kind: "removed".into(),
                        uuid,
                    },
                }
            })
            .collect()
    }

    pub fn apply_refreshed(&mut self, live: Vec<PlaylistFolder>, manual: &[ManualCategory]) {
        self.live_baseline = live;
        self.manual_categories = manual.to_vec();
        self.recompute();
    }

    fn recompute(&mut self) {
        let synthetic: Vec<PlaylistFolder> = self
            .folders
            .iter()
            .filter(|f| f.is_synthetic)
            .cloned()
            .collect();
        self.folders = self.live_baseline.clone();

        for category in &self.manual_categories {
            if !self.folders.iter().any(|f| f.uuid == category.uuid) {
                self.folders
                    .push(PlaylistFolder::new(&category.uuid, &category.title));
            }
        }
        self.folders.extend(synthetic);
        tree_edit::apply(&self.pending_edits, &mut self.folders);
    }

    pub fn cancel_edit(&mut self, uuid: &str, kind: EditKind) -> Vec<String> {
        if !self
            .pending_edits
            .iter()
            .any(|e| edit_matches(e, uuid, kind))
        {
            return Vec::new();
        }
        let sounds_to_unmark = if kind == EditKind::Removed {
            collect_node_sound_uuids(uuid, &self.live_baseline)
        } else {
            Vec::new()
        };
        self.pending_edits.retain(|e| !edit_matches(e, uuid, kind));
        self.recompute();
        sounds_to_unmark
    }

    pub fn rename_folder(&mut self, uuid: &str, new_title: &str) {
        let trimmed = new_title.trim();
        let Some(folder) = tree_edit::find_folder_mut(uuid, &mut self.folders) else {
            return;
        };
        if trimmed.is_empty() || trimmed == folder.title {
            return;
        }
        folder.title = trimmed.to_string();
        self.record_edit(TreeEdit::RenamedFolder {
            uuid: uuid.to_string(),
            new_title: trimmed.to_string(),
        });
    }

    pub fn rename_pending_group_preview(&mut self, uuid: &str, new_title: &str) {
        let trimmed = new_title.trim();
        if let Some(folder) = tree_edit::find_folder_mut(uuid, &mut self.folders)
            && !trimmed.is_empty()
            && trimmed != folder.title
        {
            folder.title = trimmed.to_string();
        }
    }

    pub fn rename_sound(&mut self, uuid: &str, new_title: &str) {
        let trimmed = new_title.trim();
        if trimmed.is_empty() || !tree_edit::rename_sound(uuid, trimmed, &mut self.folders) {
            return;
        }
        self.record_edit(TreeEdit::RenamedSound {
            uuid: uuid.to_string(),
            new_title: trimmed.to_string(),
        });
    }

    pub fn move_node(&mut self, uuid: &str, destination_uuid: &str) {
        if tree_edit::validate_move(uuid, destination_uuid, &self.folders).is_some() {
            return;
        }
        let Some(node) = tree_edit::remove_node(uuid, &mut self.folders) else {
            return;
        };

        if let Some(destination) = tree_edit::find_folder_mut(destination_uuid, &mut self.folders) {
            destination.children.push(node);
            self.record_edit(TreeEdit::Moved {
                uuid: uuid.to_string(),
                to_parent_uuid: destination_uuid.to_string(),
            });
        }
    }

    pub fn delete_node(&mut self, uuid: &str) -> Vec<String> {
        let Some(node) = tree_edit::remove_node(uuid, &mut self.folders) else {
            return Vec::new();
        };
        let mut sound_uuids = Vec::new();
        collect_sound_uuids(&node, &mut sound_uuids);
        self.record_edit(TreeEdit::Removed {
            uuid: uuid.to_string(),
        });

        sound_uuids.retain(|s| find_node_title(s, &self.folders).is_none());
        sound_uuids
    }

    pub fn add_manual_category(&mut self, uuid: &str, title: &str) {
        if self.folders.iter().any(|f| f.uuid == uuid) {
            return;
        }
        self.folders.push(PlaylistFolder::new(uuid, title));

        self.manual_categories.push(ManualCategory {
            uuid: uuid.to_string(),
            title: title.to_string(),
            image_source: String::new(),
        });
    }

    pub fn remove_manual_category(&mut self, uuid: &str) {
        if !self.manual_categories.iter().any(|c| c.uuid == uuid) {
            return;
        }
        self.manual_categories.retain(|c| c.uuid != uuid);
        self.recompute();
    }

    pub fn apply_orphans(&mut self, orphan_files: Vec<String>) {
        self.folders.retain(|f| f.uuid != ORPHAN_ROOT_UUID);
        if orphan_files.is_empty() {
            return;
        }
        let mut orphan_folder = PlaylistFolder::new(ORPHAN_ROOT_UUID, "Fichiers retrouvés");
        orphan_folder.is_synthetic = true;
        for name in orphan_files {
            orphan_folder.add_sound(&name, &name);
        }
        self.folders.push(orphan_folder);
    }

    pub fn toggle_orphan(&mut self, uuid: &str) {
        if !self.pending_orphan_deletions.remove(uuid) {
            self.pending_orphan_deletions.insert(uuid.to_string());
        }
    }

    pub fn toggle_all_orphans(&mut self) {
        let Some(folder) = self.folders.iter().find(|f| f.uuid == ORPHAN_ROOT_UUID) else {
            return;
        };
        let all: Vec<String> = folder
            .children
            .iter()
            .map(|c| c.uuid().to_string())
            .collect();
        let all_marked = !all.is_empty()
            && all
                .iter()
                .all(|u| self.pending_orphan_deletions.contains(u));
        if all_marked {
            for u in &all {
                self.pending_orphan_deletions.remove(u);
            }
        } else {
            self.pending_orphan_deletions.extend(all);
        }
    }

    pub fn clear_pending_edits(&mut self) {
        self.pending_edits.clear();
    }

    pub fn clear_pending_orphan_deletions(&mut self) {
        self.pending_orphan_deletions.clear();
    }

    fn record_edit(&mut self, edit: TreeEdit) {
        self.pending_edits
            .retain(|e| !e.targets_same_node_and_kind(&edit));
        self.pending_edits.push(edit);
    }
}

fn collect_sound_uuids(node: &PlaylistNode, out: &mut Vec<String>) {
    match node {
        PlaylistNode::Sound { uuid, .. } => out.push(uuid.clone()),
        PlaylistNode::Folder(folder) => {
            for child in &folder.children {
                collect_sound_uuids(child, out);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    RenamedFolder,
    RenamedSound,
    Moved,
    Removed,
}

impl EditKind {
    pub fn from_tag(tag: &str) -> Option<EditKind> {
        match tag {
            "renamedFolder" => Some(EditKind::RenamedFolder),
            "renamedSound" => Some(EditKind::RenamedSound),
            "moved" => Some(EditKind::Moved),
            "removed" => Some(EditKind::Removed),
            _ => None,
        }
    }
}

fn edit_matches(edit: &TreeEdit, uuid: &str, kind: EditKind) -> bool {
    edit.node_uuid() == uuid
        && matches!(
            (edit, kind),
            (TreeEdit::RenamedFolder { .. }, EditKind::RenamedFolder)
                | (TreeEdit::RenamedSound { .. }, EditKind::RenamedSound)
                | (TreeEdit::Moved { .. }, EditKind::Moved)
                | (TreeEdit::Removed { .. }, EditKind::Removed)
        )
}

fn find_node_title(uuid: &str, folders: &[PlaylistFolder]) -> Option<String> {
    fn in_children(uuid: &str, children: &[PlaylistNode]) -> Option<String> {
        for child in children {
            match child {
                PlaylistNode::Sound { uuid: u, title } if u == uuid => return Some(title.clone()),
                PlaylistNode::Folder(f) => {
                    if f.uuid == uuid {
                        return Some(f.title.clone());
                    }
                    if let Some(t) = in_children(uuid, &f.children) {
                        return Some(t);
                    }
                }
                PlaylistNode::Sound { .. } => {}
            }
        }
        None
    }
    for folder in folders {
        if folder.uuid == uuid {
            return Some(folder.title.clone());
        }
        if let Some(t) = in_children(uuid, &folder.children) {
            return Some(t);
        }
    }
    None
}

fn collect_node_sound_uuids(uuid: &str, folders: &[PlaylistFolder]) -> Vec<String> {
    fn find_and_collect(node: &PlaylistNode, uuid: &str, out: &mut Vec<String>) -> bool {
        if node.uuid() == uuid {
            collect_sound_uuids(node, out);
            return true;
        }
        if let PlaylistNode::Folder(folder) = node {
            for child in &folder.children {
                if find_and_collect(child, uuid, out) {
                    return true;
                }
            }
        }
        false
    }
    let mut out = Vec::new();
    for folder in folders {
        if folder.uuid == uuid {
            for child in &folder.children {
                collect_sound_uuids(child, &mut out);
            }
            return out;
        }
        for child in &folder.children {
            if find_and_collect(child, uuid, &mut out) {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use merlin_domain::playlist::model::PlaylistNode;

    fn category(uuid: &str, title: &str) -> PlaylistFolder {
        PlaylistFolder::new(uuid, title)
    }

    #[test]
    fn apply_refreshed_preserves_synthetics_and_grafts_manual_categories() {
        let mut session = TreeSession::default();
        let mut orphans = PlaylistFolder::new(ORPHAN_ROOT_UUID, "Fichiers retrouvés");
        orphans.is_synthetic = true;
        session.folders.push(orphans);

        let manual = [ManualCategory {
            uuid: "cat-manuelle".into(),
            title: "Ma catégorie".into(),
            image_source: "img".into(),
        }];
        session.apply_refreshed(vec![category("cat-live", "Histoires")], &manual);

        let uuids: Vec<&str> = session.folders.iter().map(|f| f.uuid.as_str()).collect();
        assert!(uuids.contains(&"cat-live"), "catégorie live présente");
        assert!(uuids.contains(&ORPHAN_ROOT_UUID), "synthétique préservé");
        assert!(
            uuids.contains(&"cat-manuelle"),
            "catégorie manuelle greffée"
        );
    }

    #[test]
    fn rename_folder_records_edit_and_dedups() {
        let mut session = TreeSession::default();
        session.folders.push(category("cat-1", "Ancien"));

        session.rename_folder("cat-1", "Nouveau");
        session.rename_folder("cat-1", "Encore plus récent");

        assert_eq!(session.folders[0].title, "Encore plus récent");
        assert_eq!(
            session.pending_edits.len(),
            1,
            "une seule édition par nœud+nature"
        );
        assert!(matches!(
            &session.pending_edits[0],
            TreeEdit::RenamedFolder { new_title, .. } if new_title == "Encore plus récent"
        ));
    }

    #[test]
    fn rename_folder_ignores_empty_or_unchanged() {
        let mut session = TreeSession::default();
        session.folders.push(category("cat-1", "Titre"));
        session.rename_folder("cat-1", "   ");
        session.rename_folder("cat-1", "Titre");
        assert!(session.pending_edits.is_empty());
    }

    #[test]
    fn move_node_into_category_records_edit() {
        let mut session = TreeSession::default();
        let mut source = category("cat-src", "Source");
        source.add_sound("snd-1", "Episode");
        session.folders.push(source);
        session.folders.push(category("cat-dst", "Destination"));

        session.move_node("snd-1", "cat-dst");

        assert!(
            session.folders[0].children.is_empty(),
            "retiré de la source"
        );
        assert_eq!(
            session.folders[1].children.len(),
            1,
            "ajouté à la destination"
        );
        assert_eq!(session.pending_edits.len(), 1);
    }

    #[test]
    fn move_node_rejects_cycle() {
        let mut session = TreeSession::default();
        let mut parent = category("cat-parent", "Parent");
        parent
            .children
            .push(PlaylistNode::Folder(category("cat-child", "Enfant")));
        session.folders.push(parent);

        session.move_node("cat-parent", "cat-child");
        assert!(
            session.pending_edits.is_empty(),
            "déplacement cyclique refusé"
        );
    }

    #[test]
    fn toggle_orphan_flips_membership() {
        let mut session = TreeSession::default();
        session.toggle_orphan("u1");
        assert!(session.pending_orphan_deletions.contains("u1"));
        session.toggle_orphan("u1");
        assert!(!session.pending_orphan_deletions.contains("u1"));
    }

    #[test]
    fn delete_node_removes_subfolder_and_returns_its_sounds() {
        let mut session = TreeSession::default();
        let mut root = category("cat-root", "Root");
        let mut sub = category("cat-sub", "Sous-dossier");
        sub.add_sound("snd-1", "E1");
        sub.add_sound("snd-2", "E2");
        root.children.push(PlaylistNode::Folder(sub));
        session.folders.push(root);

        let sounds = session.delete_node("cat-sub");

        assert_eq!(
            sounds,
            ["snd-1", "snd-2"],
            "retourne les sons descendants à supprimer"
        );
        assert!(
            session.folders[0].children.is_empty(),
            "le sous-dossier a disparu de l'arbre"
        );
        assert!(
            matches!(session.pending_edits.as_slice(), [TreeEdit::Removed { uuid }] if uuid == "cat-sub"),
            "une édition Removed est enregistrée"
        );
    }

    #[test]
    fn cancel_renamed_folder_reverts_to_baseline_title() {
        let mut session = TreeSession::default();
        session.apply_refreshed(vec![category("cat-1", "Ancien")], &[]);
        session.rename_folder("cat-1", "Nouveau");
        assert_eq!(session.folders[0].title, "Nouveau");

        let unmark = session.cancel_edit("cat-1", EditKind::RenamedFolder);

        assert!(
            unmark.is_empty(),
            "un rename ne dé-marque aucune suppression"
        );
        assert!(session.pending_edits.is_empty());
        assert_eq!(session.folders[0].title, "Ancien");
    }

    #[test]
    fn cancel_moved_node_returns_it_to_its_baseline_parent() {
        let mut session = TreeSession::default();
        let mut src = category("cat-src", "Source");
        src.add_sound("snd-1", "Episode");
        session.apply_refreshed(vec![src, category("cat-dst", "Dest")], &[]);
        session.move_node("snd-1", "cat-dst");
        assert!(session.folders[0].children.is_empty());
        assert_eq!(session.folders[1].children.len(), 1);

        session.cancel_edit("snd-1", EditKind::Moved);

        assert_eq!(
            session.folders[0].children.len(),
            1,
            "le son revient à sa source"
        );
        assert!(session.folders[1].children.is_empty());
    }

    #[test]
    fn cancel_removed_folder_restores_it_and_returns_its_descendant_sounds() {
        let mut session = TreeSession::default();
        let mut root = category("cat-root", "Root");
        let mut sub = category("cat-sub", "Sous");
        sub.add_sound("snd-1", "E1");
        sub.add_sound("snd-2", "E2");
        root.children.push(PlaylistNode::Folder(sub));
        session.apply_refreshed(vec![root], &[]);
        assert_eq!(session.delete_node("cat-sub"), ["snd-1", "snd-2"]);
        assert!(session.folders[0].children.is_empty());

        let unmark = session.cancel_edit("cat-sub", EditKind::Removed);

        assert_eq!(
            unmark,
            ["snd-1", "snd-2"],
            "sons à dé-marquer (pas de suppression physique)"
        );
        assert!(session.pending_edits.is_empty());
        let PlaylistNode::Folder(sub_back) = &session.folders[0].children[0] else {
            panic!("attendu le sous-dossier restauré");
        };
        assert_eq!(sub_back.uuid, "cat-sub");
        assert_eq!(sub_back.children.len(), 2);
    }

    #[test]
    fn cancel_removed_sound_returns_that_sound() {
        let mut session = TreeSession::default();
        let mut root = category("cat-root", "Root");
        root.add_sound("snd-1", "E1");
        session.apply_refreshed(vec![root], &[]);
        assert_eq!(session.delete_node("snd-1"), ["snd-1"]);

        assert_eq!(session.cancel_edit("snd-1", EditKind::Removed), ["snd-1"]);
        assert!(session.pending_edits.is_empty());
    }

    #[test]
    fn edit_details_resolve_old_new_and_destination_titles() {
        let mut session = TreeSession::default();
        let mut root = category("cat-root", "Root");
        let mut sub = category("cat-sub", "Ancien");
        sub.add_sound("snd-1", "Episode");
        root.children.push(PlaylistNode::Folder(sub));
        session.apply_refreshed(vec![root, category("cat-dst", "Destination")], &[]);

        session.rename_folder("cat-sub", "Nouveau");
        session.move_node("snd-1", "cat-dst");

        let details = session.edit_details();
        let rename = details
            .iter()
            .find(|d| d.kind == "renamedFolder")
            .expect("rename");
        assert_eq!(rename.title, "Ancien");
        assert_eq!(rename.new_title.as_deref(), Some("Nouveau"));
        let moved = details.iter().find(|d| d.kind == "moved").expect("move");
        assert_eq!(moved.title, "Episode");
        assert_eq!(moved.dest_title.as_deref(), Some("Destination"));
    }

    #[test]
    fn edit_details_of_removed_node_keeps_its_title() {
        let mut session = TreeSession::default();
        let mut root = category("cat-root", "Root");
        root.children
            .push(PlaylistNode::Folder(category("cat-sub", "À supprimer")));
        session.apply_refreshed(vec![root], &[]);
        session.delete_node("cat-sub");

        let removed = session
            .edit_details()
            .into_iter()
            .find(|d| d.kind == "removed")
            .expect("removed");
        assert_eq!(removed.title, "À supprimer");
    }

    #[test]
    fn cancel_absent_edit_is_a_noop() {
        let mut session = TreeSession::default();
        session.apply_refreshed(vec![category("cat-1", "Titre")], &[]);
        assert!(
            session
                .cancel_edit("cat-1", EditKind::RenamedFolder)
                .is_empty()
        );
        assert_eq!(session.folders[0].title, "Titre");
    }

    #[test]
    fn deleting_a_folder_keeps_a_mirrored_episodes_file() {
        let mut session = TreeSession::default();
        let mut cat = category("cat", "Root");
        let mut g1 = category("g1", "Podcast");
        g1.add_sound("snd-1", "Ep");
        let mut g2 = category("g2", "Derniers ajouts");
        g2.add_sound("snd-1", "Ep");
        cat.children.push(PlaylistNode::Folder(g1));
        cat.children.push(PlaylistNode::Folder(g2));
        session.apply_refreshed(vec![cat], &[]);

        assert!(
            session.delete_node("g1").is_empty(),
            "un épisode miroir (encore dans g2) ne doit pas voir son fichier supprimé"
        );
    }

    #[test]
    fn deleting_a_folder_marks_a_non_mirrored_episode() {
        let mut session = TreeSession::default();
        let mut cat = category("cat", "Root");
        let mut g1 = category("g1", "Podcast");
        g1.add_sound("snd-1", "Ep");
        cat.children.push(PlaylistNode::Folder(g1));
        session.apply_refreshed(vec![cat], &[]);

        assert_eq!(session.delete_node("g1"), ["snd-1"]);
    }

    #[test]
    fn manual_category_survives_a_recompute() {
        let mut session = TreeSession::default();
        session.apply_refreshed(vec![category("cat-1", "Live")], &[]);
        session.add_manual_category("manual-1", "Ma catégorie");
        session.rename_folder("cat-1", "Renommé");
        session.cancel_edit("cat-1", EditKind::RenamedFolder);

        assert!(
            session.folders.iter().any(|f| f.uuid == "manual-1"),
            "la catégorie manuelle survit à un recompute"
        );
    }

    #[test]
    fn remove_manual_category_drops_it_but_spares_live_folders() {
        let mut session = TreeSession::default();
        session.apply_refreshed(vec![category("cat-1", "Live")], &[]);
        session.add_manual_category("manual-1", "Ma catégorie");

        session.remove_manual_category("manual-1");
        assert!(!session.folders.iter().any(|f| f.uuid == "manual-1"));

        session.remove_manual_category("cat-1");
        assert!(
            session.folders.iter().any(|f| f.uuid == "cat-1"),
            "un dossier réel de l'enceinte n'est pas une catégorie manuelle"
        );
    }

    #[test]
    fn removed_supersedes_a_prior_rename_on_the_same_node() {
        let mut session = TreeSession::default();
        let mut root = category("cat-root", "Root");
        root.children
            .push(PlaylistNode::Folder(category("cat-sub", "Sous")));
        session.folders.push(root);

        session.rename_folder("cat-sub", "Renommé");
        session.delete_node("cat-sub");

        assert!(
            matches!(session.pending_edits.as_slice(), [TreeEdit::Removed { uuid }] if uuid == "cat-sub"),
        );
    }
}
