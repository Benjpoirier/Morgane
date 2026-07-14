use std::collections::HashSet;

use crate::playlist::model::{PlaylistFolder, PlaylistNode};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MissingFile {
    Image { remote_name: String },

    Audio { base_uuid: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueKind {
    Folder,
    Sound,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub uuid: String,
    pub title: String,
    pub kind: IssueKind,
    pub missing_files: Vec<MissingFile>,
}

pub fn expected_file_names(tree: &[PlaylistFolder]) -> Vec<String> {
    let mut names = Vec::new();
    fn visit(folder: &PlaylistFolder, names: &mut Vec<String>) {
        names.push(format!("{}.jpg", folder.uuid));
        for child in &folder.children {
            match child {
                PlaylistNode::Sound { uuid, .. } => {
                    names.push(format!("{uuid}.mp3"));
                    names.push(format!("{uuid}.aac"));
                    names.push(format!("{uuid}.jpg"));
                }
                PlaylistNode::Folder(subfolder) => visit(subfolder, names),
            }
        }
    }
    for folder in tree.iter().filter(|f| !f.is_synthetic) {
        visit(folder, &mut names);
    }
    names
}

pub fn check(tree: &[PlaylistFolder], existing_file_names: &HashSet<String>) -> Vec<Issue> {
    let mut issues = Vec::new();

    let mut seen_uuids: HashSet<String> = HashSet::new();

    fn visit(
        folder: &PlaylistFolder,
        existing: &HashSet<String>,
        seen: &mut HashSet<String>,
        issues: &mut Vec<Issue>,
    ) {
        if seen.insert(folder.uuid.clone()) && !existing.contains(&format!("{}.jpg", folder.uuid)) {
            issues.push(Issue {
                uuid: folder.uuid.clone(),
                title: folder.title.clone(),
                kind: IssueKind::Folder,
                missing_files: vec![MissingFile::Image {
                    remote_name: format!("{}.jpg", folder.uuid),
                }],
            });
        }
        for child in &folder.children {
            match child {
                PlaylistNode::Sound { uuid, title } => {
                    if !seen.insert(uuid.clone()) {
                        continue;
                    }
                    let mut missing = Vec::new();
                    if !existing.contains(&format!("{uuid}.mp3"))
                        && !existing.contains(&format!("{uuid}.aac"))
                    {
                        missing.push(MissingFile::Audio {
                            base_uuid: uuid.clone(),
                        });
                    }
                    if !existing.contains(&format!("{uuid}.jpg")) {
                        missing.push(MissingFile::Image {
                            remote_name: format!("{uuid}.jpg"),
                        });
                    }
                    if !missing.is_empty() {
                        issues.push(Issue {
                            uuid: uuid.clone(),
                            title: title.clone(),
                            kind: IssueKind::Sound,
                            missing_files: missing,
                        });
                    }
                }
                PlaylistNode::Folder(subfolder) => visit(subfolder, existing, seen, issues),
            }
        }
    }
    for folder in tree.iter().filter(|f| !f.is_synthetic) {
        visit(folder, existing_file_names, &mut seen_uuids, &mut issues);
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATEGORY_UUID: &str = "cat-uuid";
    const SOUND_UUID: &str = "8a251a87-0000-0000-0000-000000000001";

    fn names(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn folder_missing_image_is_flagged() {
        let category = PlaylistFolder::new(CATEGORY_UUID, "Histoires");

        let issues = check(&[category], &HashSet::new());

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].uuid, CATEGORY_UUID);
        assert_eq!(issues[0].kind, IssueKind::Folder);
        assert_eq!(
            issues[0].missing_files,
            vec![MissingFile::Image {
                remote_name: format!("{CATEGORY_UUID}.jpg")
            }]
        );
    }

    #[test]
    fn folder_with_image_present_is_not_flagged() {
        let category = PlaylistFolder::new(CATEGORY_UUID, "Histoires");
        assert!(check(&[category], &names(&["cat-uuid.jpg"])).is_empty());
    }

    #[test]
    fn sound_missing_audio_and_image_is_flagged_with_both() {
        let mut category = PlaylistFolder::new(CATEGORY_UUID, "Histoires");
        category.add_sound(SOUND_UUID, "Un episode");

        let issues = check(&[category], &names(&["cat-uuid.jpg"]));

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].uuid, SOUND_UUID);
        assert_eq!(issues[0].kind, IssueKind::Sound);
        assert_eq!(
            issues[0].missing_files,
            vec![
                MissingFile::Audio {
                    base_uuid: SOUND_UUID.into()
                },
                MissingFile::Image {
                    remote_name: format!("{SOUND_UUID}.jpg")
                },
            ]
        );
    }

    #[test]
    fn sound_with_aac_instead_of_mp3_is_not_flagged_for_audio() {
        let mut category = PlaylistFolder::new(CATEGORY_UUID, "Histoires");
        category.add_sound(SOUND_UUID, "Un episode");
        let existing = names(&[
            "cat-uuid.jpg",
            "8a251a87-0000-0000-0000-000000000001.aac",
            "8a251a87-0000-0000-0000-000000000001.jpg",
        ]);

        assert!(check(&[category], &existing).is_empty());
    }

    #[test]
    fn synthetic_root_folders_are_excluded() {
        let mut orphan_root =
            PlaylistFolder::new("merlinsync-fichiers-retrouves", "Fichiers retrouvés");
        orphan_root.is_synthetic = true;

        assert!(check(&[orphan_root], &HashSet::new()).is_empty());
    }

    #[test]
    fn nested_podcast_folder_is_checked() {
        let mut category = PlaylistFolder::new(CATEGORY_UUID, "Histoires");
        category
            .get_or_create_subfolder("podcast-uuid", "Un podcast")
            .add_sound(SOUND_UUID, "Un episode");
        let existing = names(&[
            "cat-uuid.jpg",
            "8a251a87-0000-0000-0000-000000000001.mp3",
            "8a251a87-0000-0000-0000-000000000001.jpg",
        ]);

        let issues = check(&[category], &existing);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].uuid, "podcast-uuid");
        assert_eq!(issues[0].kind, IssueKind::Folder);
    }

    #[test]
    fn mirrored_sound_across_two_folders_produces_only_one_issue() {
        let mut category = PlaylistFolder::new(CATEGORY_UUID, "Histoires");
        category.add_sound(SOUND_UUID, "Un episode");
        let mut recent = PlaylistFolder::new("recent-uuid", "Dernier ajouts");
        recent.add_sound(SOUND_UUID, "Un episode");

        let issues = check(
            &[category, recent],
            &names(&["cat-uuid.jpg", "recent-uuid.jpg"]),
        );

        assert_eq!(issues.iter().filter(|i| i.uuid == SOUND_UUID).count(), 1);
    }

    #[test]
    fn expected_file_names_lists_both_audio_extensions_and_images() {
        let mut category = PlaylistFolder::new(CATEGORY_UUID, "Histoires");
        category.add_sound(SOUND_UUID, "Un episode");

        let names = expected_file_names(&[category]);

        assert!(names.contains(&format!("{CATEGORY_UUID}.jpg")));
        assert!(names.contains(&format!("{SOUND_UUID}.mp3")));
        assert!(names.contains(&format!("{SOUND_UUID}.aac")));
        assert!(names.contains(&format!("{SOUND_UUID}.jpg")));
    }
}
