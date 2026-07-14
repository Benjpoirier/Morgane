use std::collections::HashMap;
use std::time::Duration;

use merlin_domain::playlist::bin_parser;
use merlin_domain::playlist::builder;
use merlin_domain::playlist::model::{PlaylistFolder, PlaylistNode};
use merlin_mock_device::FakeMerlinDevice;
use merlin_protocol::client::MerlinClient;
use merlin_protocol::commands::{self, send_file_status};
use merlin_protocol::crc32_mpeg2;

const TIMEOUT: Duration = Duration::from_secs(5);

async fn start_enforcing_device() -> FakeMerlinDevice {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("démarrage du mock");
    device.set_enforce_firmware_validation(true);
    device
}

async fn connected_client(device: &FakeMerlinDevice) -> MerlinClient {
    let mut client = MerlinClient::new("127.0.0.1", device.port());
    client.connect(TIMEOUT);
    client
}

async fn upload_file(client: &mut MerlinClient, name: &str, content: &[u8]) {
    let is_send_file = |frames: &[merlin_protocol::client::Frame]| {
        frames
            .iter()
            .any(|f| f.opcode() == Some(commands::OP_SEND_FILE))
    };
    client
        .send_frame_command(
            &commands::send_file_announce(name, content),
            TIMEOUT,
            Some(&is_send_file),
        )
        .await
        .expect("annonce");
    client
        .send_bulk(content, TIMEOUT, None)
        .await
        .expect("bulk");
    client
        .read_frames(TIMEOUT, Some(&is_send_file))
        .await
        .expect("statut final");
}

async fn update_playlist(client: &mut MerlinClient, file_name: &str) -> u8 {
    let is_update = |frames: &[merlin_protocol::client::Frame]| {
        frames
            .iter()
            .any(|f| f.opcode() == Some(commands::OP_UPDATE_PLAYLIST))
    };
    let frames = client
        .send_frame_command(
            &commands::update_playlist(file_name),
            TIMEOUT,
            Some(&is_update),
        )
        .await
        .expect("updatePlaylist");
    let frame = frames
        .iter()
        .find(|f| f.opcode() == Some(commands::OP_UPDATE_PLAYLIST))
        .expect("pas de réponse updatePlaylist");
    assert!(frame.body.len() > 1);
    frame.body[1]
}

fn make_valid_tree() -> Vec<PlaylistFolder> {
    let mut category = PlaylistFolder::new("cat-1", "Histoires");
    category.add_sound("snd-1", "Episode 1");
    let mut favorite = PlaylistFolder::new("fav-1", "Favoris");
    favorite.is_favorite = true;
    favorite.add_sound("fav-placeholder", "placeholder");
    vec![category, favorite]
}

async fn upload_tree(
    tree: &[PlaylistFolder],
    client: &mut MerlinClient,
    json_file_name: &str,
    omitting: &[&str],
) {
    let json = builder::build_json(tree, 1_700_000_000);
    upload_file(client, json_file_name, json.as_bytes()).await;

    let mut stack: Vec<&PlaylistFolder> = tree.iter().collect();
    while let Some(folder) = stack.pop() {
        let image_name = format!("{}.jpg", folder.uuid);
        if !omitting.contains(&image_name.as_str()) {
            upload_file(client, &image_name, &[0xFF]).await;
        }
        for child in &folder.children {
            match child {
                PlaylistNode::Folder(sub) => stack.push(sub),
                PlaylistNode::Sound { uuid, .. } => {
                    let audio_name = format!("{uuid}.mp3");
                    if !omitting.contains(&audio_name.as_str()) {
                        upload_file(client, &audio_name, &[0x01]).await;
                    }

                    let sound_image_name = format!("{uuid}.jpg");
                    if !omitting.contains(&sound_image_name.as_str()) {
                        upload_file(client, &sound_image_name, &[0xFE]).await;
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn valid_playlist_is_accepted_and_produces_parsable_playlist_bin() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    upload_tree(&make_valid_tree(), &mut client, "playlist.json", &[]).await;
    let status = update_playlist(&mut client, "playlist.json").await;
    assert_eq!(status, 0x00);

    let bin = client
        .download_file("playlist.bin", TIMEOUT)
        .await
        .expect("download");
    let parsed = bin_parser::parse(&bin);
    assert!(
        parsed.iter().any(|f| f.uuid == "cat-1"),
        "playlist.bin généré doit refléter le contenu envoyé"
    );
    client.close().await;
}

#[tokio::test]
async fn missing_favorite_is_rejected_with_0x13() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    let mut category = PlaylistFolder::new("cat-1", "Histoires");
    category.add_sound("snd-1", "Episode 1");
    upload_tree(&[category], &mut client, "playlist.json", &[]).await;

    assert_eq!(update_playlist(&mut client, "playlist.json").await, 0x13);
    client.close().await;
}

#[tokio::test]
async fn too_many_favorites_is_rejected_with_0x14() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    let mut favorite1 = PlaylistFolder::new("fav-1", "Favoris");
    favorite1.is_favorite = true;
    favorite1.add_sound("fav1-placeholder", "placeholder");
    let mut favorite2 = PlaylistFolder::new("fav-2", "Favoris 2");
    favorite2.is_favorite = true;
    favorite2.add_sound("fav2-placeholder", "placeholder");
    upload_tree(&[favorite1, favorite2], &mut client, "playlist.json", &[]).await;

    assert_eq!(update_playlist(&mut client, "playlist.json").await, 0x14);
    client.close().await;
}

#[tokio::test]
async fn missing_audio_file_is_rejected_with_0x12() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    upload_tree(
        &make_valid_tree(),
        &mut client,
        "playlist.json",
        &["snd-1.mp3"],
    )
    .await;
    assert_eq!(update_playlist(&mut client, "playlist.json").await, 0x12);
    client.close().await;
}

#[tokio::test]
async fn missing_image_file_is_rejected_with_0x11() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    upload_tree(
        &make_valid_tree(),
        &mut client,
        "playlist.json",
        &["cat-1.jpg"],
    )
    .await;
    assert_eq!(update_playlist(&mut client, "playlist.json").await, 0x11);
    client.close().await;
}

#[tokio::test]
async fn missing_sound_image_is_rejected_with_0x11() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    upload_tree(
        &make_valid_tree(),
        &mut client,
        "playlist.json",
        &["snd-1.jpg"],
    )
    .await;
    assert_eq!(update_playlist(&mut client, "playlist.json").await, 0x11);
    client.close().await;
}

#[tokio::test]
async fn wrong_extension_is_rejected_with_0x02() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    upload_tree(&make_valid_tree(), &mut client, "playlist.txt", &[]).await;
    assert_eq!(update_playlist(&mut client, "playlist.txt").await, 0x02);
    client.close().await;
}

#[tokio::test]
async fn missing_playlist_file_is_rejected_with_0x03() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    assert_eq!(update_playlist(&mut client, "playlist.json").await, 0x03);
    client.close().await;
}

#[tokio::test]
async fn validation_disabled_by_default_ignores_real_content() {
    let device = FakeMerlinDevice::start(0, HashMap::new(), false, None)
        .await
        .expect("mock");
    let mut client = connected_client(&device).await;

    assert_eq!(
        update_playlist(&mut client, "playlist.json").await,
        0x00,
        "sans enforce_firmware_validation, update_playlist_status (0 par défaut) est renvoyé tel quel"
    );
    client.close().await;
}

#[tokio::test]
async fn explicit_status_override_bypasses_real_validation_even_when_enforced() {
    let device = start_enforcing_device().await;
    device.set_update_playlist_status(19);
    let mut client = connected_client(&device).await;

    assert_eq!(update_playlist(&mut client, "playlist.json").await, 19);
    client.close().await;
}

#[tokio::test]
async fn sha256_mismatch_is_rejected_with_hash_mismatch_status() {
    let device = start_enforcing_device().await;
    let mut client = connected_client(&device).await;

    let content: [u8; 4] = [1, 2, 3, 4];
    let name_bytes = b"bad.mp3";
    let wrong_sha256 = [0xABu8; 32];
    let mut prefix = vec![commands::OP_SEND_FILE, name_bytes.len() as u8];
    prefix.extend_from_slice(name_bytes);
    prefix.extend_from_slice(&(content.len() as u32).to_le_bytes());
    prefix.extend_from_slice(&wrong_sha256);
    let crc = crc32_mpeg2::checksum_le(&prefix);
    prefix.extend_from_slice(&crc);

    let is_send_file = |frames: &[merlin_protocol::client::Frame]| {
        frames
            .iter()
            .any(|f| f.opcode() == Some(commands::OP_SEND_FILE))
    };
    let announce_frames = client
        .send_frame_command(&prefix, TIMEOUT, Some(&is_send_file))
        .await
        .expect("annonce");
    let ready = announce_frames
        .iter()
        .find(|f| f.opcode() == Some(commands::OP_SEND_FILE))
        .and_then(|f| f.body.get(1).copied());
    assert_eq!(ready, Some(send_file_status::READY));

    client
        .send_bulk(&content, TIMEOUT, None)
        .await
        .expect("bulk");
    let final_frames = client
        .read_frames(TIMEOUT, Some(&is_send_file))
        .await
        .expect("statut");
    let status = final_frames
        .iter()
        .find(|f| f.opcode() == Some(commands::OP_SEND_FILE))
        .and_then(|f| f.body.get(1).copied());
    assert_eq!(status, Some(send_file_status::HASH_MISMATCH));
    client.close().await;
}
