use super::model::{PlaylistFolder, PlaylistNode};

use super::bin_parser::{KIND_FAVORITE, KIND_FOLDER, KIND_ROOT, KIND_SOUND};

const RECORD_SIZE: usize = 152;
const ID_OFFSET: usize = 0;
const PARENT_ID_OFFSET: usize = 2;
const KIND_OFFSET: usize = 10;
const UUID_LENGTH_OFFSET: usize = 20;
const TITLE_LENGTH_OFFSET: usize = 85;
const MAX_UUID_BYTES: usize = TITLE_LENGTH_OFFSET - UUID_LENGTH_OFFSET - 1;
const MAX_TITLE_BYTES: usize = RECORD_SIZE - TITLE_LENGTH_OFFSET - 1;

pub const FAVORITE_RECORD_TITLE: &str = "Merlin_favorite";

pub fn encode(root_folders: &[PlaylistFolder]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut next_id: u16 = 1;

    let root_id = next_id;
    next_id += 1;
    output.extend_from_slice(&make_record(root_id, 0, KIND_ROOT, "", "Root"));

    for folder in root_folders {
        encode_folder(folder, root_id, &mut output, &mut next_id);
    }

    output
}

fn encode_folder(folder: &PlaylistFolder, parent_id: u16, output: &mut Vec<u8>, next_id: &mut u16) {
    let id = *next_id;
    *next_id += 1;
    let (kind, title) = if folder.is_favorite {
        (KIND_FAVORITE, FAVORITE_RECORD_TITLE)
    } else {
        (KIND_FOLDER, folder.title.as_str())
    };
    output.extend_from_slice(&make_record(id, parent_id, kind, &folder.uuid, title));
    for child in &folder.children {
        match child {
            PlaylistNode::Folder(subfolder) => encode_folder(subfolder, id, output, next_id),
            PlaylistNode::Sound { uuid, title } => {
                let sound_id = *next_id;
                *next_id += 1;
                output.extend_from_slice(&make_record(sound_id, id, KIND_SOUND, uuid, title));
            }
        }
    }
}

fn make_record(id: u16, parent_id: u16, kind: u16, uuid: &str, title: &str) -> [u8; RECORD_SIZE] {
    let mut record = [0u8; RECORD_SIZE];
    record[ID_OFFSET..ID_OFFSET + 2].copy_from_slice(&id.to_le_bytes());
    record[PARENT_ID_OFFSET..PARENT_ID_OFFSET + 2].copy_from_slice(&parent_id.to_le_bytes());
    record[KIND_OFFSET..KIND_OFFSET + 2].copy_from_slice(&kind.to_le_bytes());
    write_length_prefixed_string(uuid, &mut record, UUID_LENGTH_OFFSET, MAX_UUID_BYTES);
    write_length_prefixed_string(title, &mut record, TITLE_LENGTH_OFFSET, MAX_TITLE_BYTES);
    record
}

fn write_length_prefixed_string(
    string: &str,
    record: &mut [u8; RECORD_SIZE],
    field_offset: usize,
    max_bytes: usize,
) {
    let mut len = string.len().min(max_bytes);
    while len > 0 && !string.is_char_boundary(len) {
        len -= 1;
    }
    let bytes = string.as_bytes();
    record[field_offset] = len as u8;
    record[field_offset + 1..field_offset + 1 + len].copy_from_slice(&bytes[..len]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_never_splits_a_multibyte_char() {
        let title = format!("{}é", "a".repeat(65));
        let mut record = [0u8; RECORD_SIZE];
        write_length_prefixed_string(&title, &mut record, TITLE_LENGTH_OFFSET, MAX_TITLE_BYTES);

        let len = record[TITLE_LENGTH_OFFSET] as usize;
        let written = &record[TITLE_LENGTH_OFFSET + 1..TITLE_LENGTH_OFFSET + 1 + len];
        assert_eq!(std::str::from_utf8(written).expect("UTF-8 valide"), "a".repeat(65));
        assert!(len <= MAX_TITLE_BYTES);
    }

    #[test]
    fn short_ascii_title_is_written_verbatim() {
        let mut record = [0u8; RECORD_SIZE];
        write_length_prefixed_string("Root", &mut record, TITLE_LENGTH_OFFSET, MAX_TITLE_BYTES);
        let len = record[TITLE_LENGTH_OFFSET] as usize;
        assert_eq!(len, 4);
        assert_eq!(&record[TITLE_LENGTH_OFFSET + 1..TITLE_LENGTH_OFFSET + 1 + len], b"Root");
    }
}
