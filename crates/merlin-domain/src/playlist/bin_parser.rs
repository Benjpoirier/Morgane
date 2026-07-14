use std::collections::{BTreeMap, HashSet};

use super::model::{PlaylistFolder, PlaylistNode};

pub const ACTUAL_RECORD_SIZE: usize = 152;

const ID_OFFSET: usize = 0;
const PARENT_ID_OFFSET: usize = 2;
const KIND_OFFSET: usize = 10;
const UUID_LENGTH_OFFSET: usize = 20;
const TITLE_LENGTH_OFFSET: usize = 85;

pub const KIND_ROOT: u16 = 1;
pub const KIND_FOLDER: u16 = 2;
pub const KIND_SOUND: u16 = 4;
pub const KIND_FAVORITE: u16 = 10;

struct RawRecord {
    id: u16,
    parent_id: u16,
    kind: u16,
    uuid: String,
    title: String,
}

impl RawRecord {
    fn is_folder(&self) -> bool {
        matches!(self.kind, KIND_ROOT | KIND_FOLDER | KIND_FAVORITE)
    }
}

pub fn parse(data: &[u8]) -> Vec<PlaylistFolder> {
    if data.is_empty() || !data.len().is_multiple_of(ACTUAL_RECORD_SIZE) {
        return Vec::new();
    }
    let record_count = data.len() / ACTUAL_RECORD_SIZE;

    let mut records: BTreeMap<usize, RawRecord> = BTreeMap::new();
    for i in 0..record_count {
        if let Some(record) = parse_record(data, i * ACTUAL_RECORD_SIZE) {
            records.insert(i, record);
        }
    }
    if records.is_empty() {
        return Vec::new();
    }

    let known_ids: HashSet<u16> = records.values().map(|r| r.id).collect();
    let root_record = records
        .values()
        .find(|r| r.kind == KIND_ROOT)
        .or_else(|| {
            records
                .values()
                .filter(|r| r.parent_id == 0)
                .min_by_key(|r| r.id)
        })
        .or_else(|| {
            records
                .values()
                .filter(|r| !known_ids.contains(&r.parent_id))
                .min_by_key(|r| r.id)
        });
    let Some(root_record) = root_record else {
        return Vec::new();
    };
    let root_id = root_record.id;

    let mut visited: HashSet<u16> = HashSet::from([root_id]);
    build_children(root_id, &records, &mut visited)
        .into_iter()
        .filter_map(|node| match node {
            PlaylistNode::Folder(folder) => Some(folder),

            PlaylistNode::Sound { .. } => None,
        })
        .collect()
}

fn build_children(
    parent_id: u16,
    records: &BTreeMap<usize, RawRecord>,
    visited: &mut HashSet<u16>,
) -> Vec<PlaylistNode> {
    let mut children: Vec<&RawRecord> = records
        .values()
        .filter(|r| r.parent_id == parent_id)
        .collect();
    children.sort_by_key(|r| r.id);

    children
        .into_iter()
        .filter_map(|record| {
            if !record.is_folder() {
                return Some(PlaylistNode::Sound {
                    uuid: record.uuid.clone(),
                    title: record.title.clone(),
                });
            }

            if !visited.insert(record.id) {
                return None;
            }
            let mut folder = PlaylistFolder::new(record.uuid.clone(), record.title.clone());
            folder.is_favorite = record.kind == KIND_FAVORITE;
            folder.children = build_children(record.id, records, visited);
            Some(PlaylistNode::Folder(folder))
        })
        .collect()
}

fn parse_record(bytes: &[u8], base: usize) -> Option<RawRecord> {
    if base + ACTUAL_RECORD_SIZE > bytes.len() {
        return None;
    }

    let id = u16::from_le_bytes([bytes[base + ID_OFFSET], bytes[base + ID_OFFSET + 1]]);
    let parent_id = u16::from_le_bytes([
        bytes[base + PARENT_ID_OFFSET],
        bytes[base + PARENT_ID_OFFSET + 1],
    ]);
    let kind = u16::from_le_bytes([bytes[base + KIND_OFFSET], bytes[base + KIND_OFFSET + 1]]);

    let uuid = read_length_prefixed_string(bytes, base, UUID_LENGTH_OFFSET)?;
    let title = read_length_prefixed_string(bytes, base, TITLE_LENGTH_OFFSET)?;

    Some(RawRecord {
        id,
        parent_id,
        kind,
        uuid,
        title,
    })
}

fn read_length_prefixed_string(
    bytes: &[u8],
    record_base: usize,
    field_offset: usize,
) -> Option<String> {
    let length_index = record_base + field_offset;
    if length_index >= bytes.len() {
        return None;
    }
    let length = bytes[length_index] as usize;
    if length == 0 {
        return Some(String::new());
    }
    let string_start = length_index + 1;
    let declared_end = string_start + length;
    let record_end = record_base + ACTUAL_RECORD_SIZE;
    let string_end = declared_end.min(record_end);
    if string_start > string_end || string_end > bytes.len() {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes[string_start..string_end]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn make_record(
        id: u16,
        parent_id: u16,
        kind: u16,
        uuid: &str,
        title: &str,
    ) -> Vec<u8> {
        let mut record = vec![0u8; ACTUAL_RECORD_SIZE];
        record[ID_OFFSET..ID_OFFSET + 2].copy_from_slice(&id.to_le_bytes());
        record[PARENT_ID_OFFSET..PARENT_ID_OFFSET + 2].copy_from_slice(&parent_id.to_le_bytes());
        record[KIND_OFFSET..KIND_OFFSET + 2].copy_from_slice(&kind.to_le_bytes());

        let uuid_bytes = uuid.as_bytes();
        record[UUID_LENGTH_OFFSET] = uuid_bytes.len() as u8;
        record[UUID_LENGTH_OFFSET + 1..UUID_LENGTH_OFFSET + 1 + uuid_bytes.len()]
            .copy_from_slice(uuid_bytes);

        let title_bytes = title.as_bytes();
        record[TITLE_LENGTH_OFFSET] = title_bytes.len() as u8;
        record[TITLE_LENGTH_OFFSET + 1..TITLE_LENGTH_OFFSET + 1 + title_bytes.len()]
            .copy_from_slice(title_bytes);

        record
    }

    #[test]
    fn an_empty_folder_is_still_a_folder() {
        let root = make_record(1, 0, KIND_ROOT, "", "Root");
        let empty = make_record(2, 1, KIND_FOLDER, "cat-vide", "Dernier ajouts");
        let root_folders = parse(&[root, empty].concat());

        assert_eq!(
            root_folders.len(),
            1,
            "une categorie vide ne doit pas disparaitre"
        );
        assert_eq!(root_folders[0].title, "Dernier ajouts");
        assert!(root_folders[0].children.is_empty());
    }

    #[test]
    fn a_record_that_is_its_own_parent_does_not_loop_forever() {
        let self_parent_root = make_record(0, 0, KIND_ROOT, "", "Root");
        let child = make_record(1, 0, KIND_SOUND, "ep", "Episode");
        let root_folders = parse(&[self_parent_root, child].concat());

        assert!(
            root_folders.is_empty(),
            "aucun dossier, mais surtout : ca termine"
        );
    }

    #[test]
    fn a_cycle_between_two_folders_does_not_loop_forever() {
        let root = make_record(1, 0, KIND_ROOT, "", "Root");
        let a = make_record(2, 1, KIND_FOLDER, "a", "A");
        let b = make_record(3, 2, KIND_FOLDER, "b", "B");
        let cycle = make_record(2, 3, KIND_FOLDER, "a", "A");
        let root_folders = parse(&[root, a, b, cycle].concat());

        assert_eq!(root_folders.len(), 1);
        assert_eq!(root_folders[0].title, "A");
    }

    #[test]
    fn parses_root_folder_and_leaf_from_synthetic_playlist_bin() {
        let root = make_record(1, 0, KIND_ROOT, "", "Root");
        let folder = make_record(
            2,
            1,
            KIND_FOLDER,
            "8a251a87-0000-0000-0000-000000000001",
            "Histoires",
        );

        let leaf = make_record(
            3,
            2,
            KIND_SOUND,
            "91213766-0000-0000-0000-000000000002",
            "Mystère à l'école",
        );

        let data: Vec<u8> = [root, folder, leaf].concat();
        let root_folders = parse(&data);

        assert_eq!(root_folders.len(), 1);
        let histoires = &root_folders[0];
        assert_eq!(histoires.uuid, "8a251a87-0000-0000-0000-000000000001");
        assert_eq!(histoires.title, "Histoires");
        assert_eq!(histoires.children.len(), 1);

        let PlaylistNode::Sound { uuid, title } = &histoires.children[0] else {
            panic!("l'enfant attendu est un son (feuille), pas un dossier");
        };
        assert_eq!(uuid, "91213766-0000-0000-0000-000000000002");
        assert_eq!(title, "Mystère à l'école");
    }

    #[test]
    fn overflowing_declared_title_length_is_clamped_not_rejected() {
        const AVAILABLE: usize = ACTUAL_RECORD_SIZE - TITLE_LENGTH_OFFSET - 1;
        let mut record = vec![0u8; ACTUAL_RECORD_SIZE];
        record[ID_OFFSET] = 2;
        record[PARENT_ID_OFFSET] = 1;
        record[KIND_OFFSET] = KIND_FOLDER as u8;
        record[TITLE_LENGTH_OFFSET] = (AVAILABLE + 9) as u8;
        let title = "Titre bien trop long pour la place disponible dans l'enregistrement, vraiment";
        let title_bytes = &title.as_bytes()[..AVAILABLE];
        record[TITLE_LENGTH_OFFSET + 1..TITLE_LENGTH_OFFSET + 1 + AVAILABLE]
            .copy_from_slice(title_bytes);

        let root = make_record(1, 0, KIND_ROOT, "", "Root");
        let data: Vec<u8> = [root, record].concat();
        let root_folders = parse(&data);

        assert_eq!(
            root_folders.len(),
            1,
            "l'enregistrement ne doit pas être rejeté malgré la longueur incohérente"
        );
        assert_eq!(root_folders[0].title, String::from_utf8_lossy(title_bytes));
    }

    #[test]
    fn root_selection_is_deterministic_when_parent_id_zero_appears_once() {
        let root = make_record(5, 0, KIND_ROOT, "", "Root");
        let folder = make_record(6, 5, KIND_FOLDER, "f", "Dossier");
        let child = make_record(7, 6, KIND_SOUND, "child", "Enfant");
        let data: Vec<u8> = [root, folder, child].concat();
        let root_folders = parse(&data);
        assert_eq!(root_folders.len(), 1);
        assert_eq!(root_folders[0].uuid, "f");
    }

    #[test]
    fn empty_data_returns_empty_array_instead_of_crashing() {
        assert!(parse(&[]).is_empty());
    }

    #[test]
    fn size_not_multiple_of_record_size_returns_empty_array() {
        assert!(parse(&[0x01, 0x02, 0x03]).is_empty());
    }

    #[test]
    fn same_uuid_appearing_twice_in_tree_is_preserved_as_two_separate_nodes() {
        let root = make_record(1, 0, KIND_ROOT, "", "Root");
        let category_a = make_record(2, 1, KIND_FOLDER, "cat-a", "Categorie A");
        let episode_in_a = make_record(3, 2, KIND_SOUND, "shared-ep", "Episode Partage");
        let category_b = make_record(4, 1, KIND_FOLDER, "cat-b", "Dernier ajouts");
        let episode_in_b = make_record(5, 4, KIND_SOUND, "shared-ep", "Episode Partage");

        let data: Vec<u8> = [root, category_a, episode_in_a, category_b, episode_in_b].concat();
        let root_folders = parse(&data);

        assert_eq!(root_folders.len(), 2);
        for folder in &root_folders {
            let Some(PlaylistNode::Sound { uuid, .. }) = folder.children.first() else {
                panic!("chaque catégorie doit contenir son épisode");
            };
            assert_eq!(uuid, "shared-ep");
        }
    }

    #[test]
    fn favorite_folder_is_recognized_by_its_kind() {
        let root = make_record(1, 0, KIND_ROOT, "", "Root");
        let favorite = make_record(2, 1, KIND_FAVORITE, "fav-uuid", "Merlin_favorite");
        let child = make_record(3, 2, KIND_SOUND, "ep", "Episode");
        let data: Vec<u8> = [root, favorite, child].concat();
        let root_folders = parse(&data);
        assert_eq!(root_folders.len(), 1);
        assert!(root_folders[0].is_favorite);
    }
}
