use std::collections::{HashMap, HashSet};
use std::time::Duration;

use merlin_domain::playlist::bin_parser::{KIND_FOLDER, KIND_ROOT, KIND_SOUND};
use merlin_infra::sync::device_tree_fetcher;
use merlin_mock_device::FakeMerlinDevice;

const TIMEOUT: Duration = Duration::from_secs(10);
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

#[tokio::test]
async fn list_files_enumerates_beyond_256_entries() {
    let mut initial_files: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..300 {
        initial_files.insert(format!("file-{i:03}.mp3"), vec![0u8]);
    }
    let device = FakeMerlinDevice::start(0, initial_files, false, None)
        .await
        .expect("mock");

    let files = device_tree_fetcher::list_files("127.0.0.1", device.port(), TIMEOUT)
        .await
        .expect("list_files");

    assert_eq!(
        files.len(),
        300,
        "les fichiers au-delà de l'index 255 ne doivent plus être perdus"
    );
    assert!(files.iter().any(|f| f.name == "file-299.mp3"));
}

#[tokio::test]
async fn check_files_exist_returns_only_names_present_on_device() {
    let initial_files = HashMap::from([(
        "8a251a87-0000-0000-0000-000000000001.jpg".to_string(),
        vec![0u8],
    )]);
    let device = FakeMerlinDevice::start(0, initial_files, false, None)
        .await
        .expect("mock");

    let found = device_tree_fetcher::check_files_exist(
        "127.0.0.1",
        device.port(),
        &[
            "8a251a87-0000-0000-0000-000000000001.jpg".to_string(),
            "8a251a87-0000-0000-0000-000000000001.mp3".to_string(),
        ],
        TIMEOUT,
    )
    .await
    .expect("check_files_exist");

    assert_eq!(
        found,
        HashSet::from(["8a251a87-0000-0000-0000-000000000001.jpg".to_string()])
    );
}

#[tokio::test]
async fn check_files_exist_with_empty_names_returns_empty_without_connecting() {
    let found = device_tree_fetcher::check_files_exist("127.0.0.1", 1, &[], TIMEOUT)
        .await
        .expect("check_files_exist");
    assert!(found.is_empty());
}

#[tokio::test]
async fn fetch_tree_and_check_integrity_returns_empty_when_playlist_bin_missing() {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("mock");

    let (tree, existing_names) = device_tree_fetcher::fetch_tree_and_check_integrity(
        "127.0.0.1",
        device.port(),
        TIMEOUT,
        None,
    )
    .await
    .expect("fetch");

    assert!(tree.is_empty());
    assert!(existing_names.is_empty());
}

#[tokio::test]
async fn fetch_tree_and_check_integrity_finds_folder_image() {
    let uuid = "8a251a87-0000-0000-0000-000000000001";
    let sound_uuid = "91213766-0000-0000-0000-000000000002";

    let root = make_record(1, 0, KIND_ROOT, "", "Root");
    let folder = make_record(2, 1, KIND_FOLDER, uuid, "Histoires");
    let sound = make_record(3, 2, KIND_SOUND, sound_uuid, "Un episode");
    let playlist_bin: Vec<u8> = [root, folder, sound].concat();

    let initial_files = HashMap::from([
        ("playlist.bin".to_string(), playlist_bin),
        (format!("{uuid}.jpg"), vec![0u8]),
        (format!("{sound_uuid}.mp3"), vec![0u8]),
    ]);
    let device = FakeMerlinDevice::start(0, initial_files, false, None)
        .await
        .expect("mock");

    let (tree, existing_names) = device_tree_fetcher::fetch_tree_and_check_integrity(
        "127.0.0.1",
        device.port(),
        TIMEOUT,
        None,
    )
    .await
    .expect("fetch");

    let uuids: Vec<&str> = tree.iter().map(|f| f.uuid.as_str()).collect();
    assert_eq!(uuids, [uuid]);
    assert_eq!(
        existing_names,
        HashSet::from([format!("{uuid}.jpg"), format!("{sound_uuid}.mp3")])
    );
}

#[tokio::test]
async fn fetch_tree_and_check_integrity_treats_a_zero_byte_file_as_missing() {
    let uuid = "8a251a87-0000-0000-0000-000000000001";
    let sound_uuid = "91213766-0000-0000-0000-000000000002";
    let root = make_record(1, 0, KIND_ROOT, "", "Root");
    let folder = make_record(2, 1, KIND_FOLDER, uuid, "Histoires");
    let sound = make_record(3, 2, KIND_SOUND, sound_uuid, "Un episode");
    let playlist_bin: Vec<u8> = [root, folder, sound].concat();

    let initial_files = HashMap::from([
        ("playlist.bin".to_string(), playlist_bin),
        (format!("{uuid}.jpg"), vec![0u8]),
        (format!("{sound_uuid}.mp3"), Vec::new()),
    ]);
    let device = FakeMerlinDevice::start(0, initial_files, false, None)
        .await
        .expect("mock");

    let (_tree, existing_names) = device_tree_fetcher::fetch_tree_and_check_integrity(
        "127.0.0.1",
        device.port(),
        TIMEOUT,
        None,
    )
    .await
    .expect("fetch");

    assert!(
        existing_names.contains(&format!("{uuid}.jpg")),
        "l'image valide reste presente"
    );
    assert!(
        !existing_names.contains(&format!("{sound_uuid}.mp3")),
        "le mp3 vide (0 octet) doit etre retire de found -> signale comme manquant"
    );
}
