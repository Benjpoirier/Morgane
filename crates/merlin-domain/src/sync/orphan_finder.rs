use std::collections::HashSet;

use crate::playlist::model::{PlaylistFolder, PlaylistNode};

use super::device_file::{DeviceFile, is_protected_file};

pub fn find_orphan_files(files: &[DeviceFile], tree: &[PlaylistFolder]) -> Vec<String> {
    let referenced_uuids = collect_referenced_uuids(tree);
    let mut seen = HashSet::new();
    let mut orphans = Vec::new();
    for file in files {
        if is_protected_file(&file.name) {
            continue;
        }

        let base = file
            .name
            .rsplit_once('.')
            .map(|(b, _)| b)
            .unwrap_or(&file.name);
        if referenced_uuids.contains(base) {
            continue;
        }

        if !seen.insert(file.name.clone()) {
            continue;
        }
        orphans.push(file.name.clone());
    }
    orphans
}

fn collect_referenced_uuids(folders: &[PlaylistFolder]) -> HashSet<String> {
    let mut uuids = HashSet::new();
    fn visit(folder: &PlaylistFolder, uuids: &mut HashSet<String>) {
        uuids.insert(folder.uuid.clone());
        for child in &folder.children {
            match child {
                PlaylistNode::Sound { uuid, .. } => {
                    uuids.insert(uuid.clone());
                }
                PlaylistNode::Folder(subfolder) => visit(subfolder, uuids),
            }
        }
    }
    for folder in folders {
        visit(folder, &mut uuids);
    }
    uuids
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCED_UUID: &str = "8a251a87-0000-0000-0000-000000000001";
    const ORPHAN_UUID: &str = "91213766-0000-0000-0000-000000000002";

    fn make_tree() -> Vec<PlaylistFolder> {
        let mut category = PlaylistFolder::new("cat-uuid", "Histoires");
        category
            .get_or_create_subfolder("podcast-uuid", "Un podcast")
            .add_sound(REFERENCED_UUID, "Un episode reference");
        vec![category]
    }

    #[test]
    fn orphan_audio_and_image_are_both_detected_by_exact_name() {
        let files = vec![
            DeviceFile::new(format!("{REFERENCED_UUID}.mp3"), 100),
            DeviceFile::new(format!("{REFERENCED_UUID}.jpg"), 10),
            DeviceFile::new(format!("{ORPHAN_UUID}.mp3"), 200),
            DeviceFile::new(format!("{ORPHAN_UUID}.jpg"), 10),
        ];

        let orphans = find_orphan_files(&files, &make_tree());

        assert_eq!(
            orphans,
            [format!("{ORPHAN_UUID}.mp3"), format!("{ORPHAN_UUID}.jpg")]
        );
    }

    #[test]
    fn referenced_folder_thumbnail_is_not_an_orphan() {
        let files = vec![DeviceFile::new("cat-uuid.jpg", 10)];
        assert!(find_orphan_files(&files, &make_tree()).is_empty());
    }

    #[test]
    fn system_and_credential_files_are_never_orphans() {
        let files = vec![
            DeviceFile::new("wifi.cfg", 50),
            DeviceFile::new("sta_wifi.json", 50),
            DeviceFile::new("sleep.cfg", 50),
            DeviceFile::new("playlist.bin", 1064),
            DeviceFile::new("playlist.json", 500),
        ];

        assert!(find_orphan_files(&files, &[]).is_empty());
    }

    #[test]
    fn a_stray_non_uuid_file_is_an_orphan() {
        let files = vec![DeviceFile::new("leftover.tmp", 42)];
        assert_eq!(find_orphan_files(&files, &[]), ["leftover.tmp"]);
    }

    #[test]
    fn referenced_episode_is_not_flagged_as_orphan() {
        let files = vec![DeviceFile::new(format!("{REFERENCED_UUID}.mp3"), 100)];

        assert!(find_orphan_files(&files, &make_tree()).is_empty());
    }

    #[test]
    fn no_orphans_when_list_is_empty() {
        assert!(find_orphan_files(&[], &make_tree()).is_empty());
    }

    #[test]
    fn orphan_aac_not_referenced_in_tree_is_detected() {
        let files = vec![DeviceFile::new(format!("{ORPHAN_UUID}.aac"), 200)];

        assert_eq!(
            find_orphan_files(&files, &[]),
            [format!("{ORPHAN_UUID}.aac")]
        );
    }

    #[test]
    fn referenced_aac_episode_is_not_flagged_as_orphan() {
        let files = vec![DeviceFile::new(format!("{REFERENCED_UUID}.aac"), 100)];

        assert!(find_orphan_files(&files, &make_tree()).is_empty());
    }
}
