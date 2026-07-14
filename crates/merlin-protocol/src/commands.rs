use sha2::{Digest, Sha256};

use crate::crc32_mpeg2;
use crate::firmware_error_catalog;

pub const OP_SEND_FILE: u8 = 0x01;
pub const OP_CONNECT: u8 = 0x02;
pub const OP_SESSION_TOKEN: u8 = 0x03;
pub const OP_MAC_ADDRESS: u8 = 0x04;
pub const OP_FIRMWARE_INFO: u8 = 0x05;
pub const OP_UPDATE_PLAYLIST: u8 = 0x06;
pub const OP_DELETE_FILE: u8 = 0x0F;
pub const OP_SET_DATE: u8 = 0x08;
pub const OP_END_SYNCHRONIZATION: u8 = 0x09;
pub const OP_TELEMETRY_0E: u8 = 0x0E;
pub const OP_TELEMETRY_1B: u8 = 0x1B;
pub const OP_SET_ENABLING_HOURS: u8 = 0x1D;

pub const OP_GET_HARDWARE_ID: u8 = 0x1E;
pub const OP_SEARCH_FILE: u8 = 0x1F;
pub const OP_SET_WIFI_CONFIG: u8 = 0x10;

pub const OP_GET_NUMBER_OF_FILES: u8 = 0x0B;
pub const OP_GET_FILE_INFORMATION: u8 = 0x0C;
pub const OP_DOWNLOAD_FILE: u8 = 0x0D;

pub mod send_file_status {
    pub const READY: u8 = 0x00;
    pub const SUCCESS: u8 = 0x01;
    pub const INSUFFICIENT_SPACE: u8 = 0x02;
    pub const NAME_TOO_LONG: u8 = 0x03;
    pub const HASH_MISMATCH: u8 = 0x04;
    pub const BAD_LENGTH: u8 = 0x05;
    pub const OPEN_FAILED: u8 = 0x07;
    pub const WRITE_ERROR: u8 = 0x08;

    pub fn description(status: u8) -> String {
        super::firmware_error_catalog::send_file_status(status)
    }
}

const CONNECT_MAGIC: [u8; 4] = [0xda, 0x84, 0x8a, 0x47];
const SESSION_TOKEN_MAGIC: [u8; 4] = [0x6d, 0x99, 0x4b, 0x43];
const MAC_ADDRESS_MAGIC: [u8; 4] = [0x68, 0xc9, 0x0c, 0x5d];
const FIRMWARE_INFO_MAGIC: [u8; 4] = [0xdf, 0xd4, 0xcd, 0x59];
const TELEMETRY_0E_MAGIC: [u8; 4] = [0xbe, 0x1f, 0x86, 0x72];
const TELEMETRY_1B_MAGIC: [u8; 4] = [0xa5, 0xaf, 0x52, 0x29];
const HARDWARE_ID_MAGIC: [u8; 4] = [0xce, 0xc4, 0x97, 0x3e];
const END_SYNC_MAGIC: [u8; 4] = [0xbb, 0x4f, 0xc1, 0x6c];
const SET_ENABLING_HOURS_SUFFIX: [u8; 4] = [0x57, 0x52, 0x91, 0x2b];

fn fixed_command(opcode: u8, magic: [u8; 4]) -> Vec<u8> {
    let mut body = vec![opcode];
    body.extend_from_slice(&magic);
    body
}

pub fn connect() -> Vec<u8> {
    fixed_command(OP_CONNECT, CONNECT_MAGIC)
}

pub fn get_mac_address() -> Vec<u8> {
    fixed_command(OP_MAC_ADDRESS, MAC_ADDRESS_MAGIC)
}

pub fn get_session_token() -> Vec<u8> {
    fixed_command(OP_SESSION_TOKEN, SESSION_TOKEN_MAGIC)
}

pub fn get_firmware_info() -> Vec<u8> {
    fixed_command(OP_FIRMWARE_INFO, FIRMWARE_INFO_MAGIC)
}

pub fn get_telemetry_0e() -> Vec<u8> {
    fixed_command(OP_TELEMETRY_0E, TELEMETRY_0E_MAGIC)
}

pub fn get_telemetry_1b() -> Vec<u8> {
    fixed_command(OP_TELEMETRY_1B, TELEMETRY_1B_MAGIC)
}

pub fn get_hardware_id() -> Vec<u8> {
    fixed_command(OP_GET_HARDWARE_ID, HARDWARE_ID_MAGIC)
}

pub fn set_enabling_hours_disabled() -> Vec<u8> {
    let mut body = vec![OP_SET_ENABLING_HOURS];
    body.extend_from_slice(&[0u8; 8]);
    body.extend_from_slice(&SET_ENABLING_HOURS_SUFFIX);
    body
}

pub fn set_date(unix_timestamp: Option<u32>) -> Vec<u8> {
    let ts = unix_timestamp.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("horloge système antérieure à 1970")
            .as_secs() as u32
    });
    let mut prefix = vec![OP_SET_DATE];
    prefix.extend_from_slice(&ts.to_le_bytes());
    with_crc(prefix)
}

pub fn end_synchronization() -> Vec<u8> {
    fixed_command(OP_END_SYNCHRONIZATION, END_SYNC_MAGIC)
}

pub fn search_file(file_name: &str) -> Vec<u8> {
    let name_bytes = file_name.as_bytes();
    assert!(name_bytes.len() <= 255, "nom de fichier trop long");
    let mut prefix = vec![OP_SEARCH_FILE, 0x01];
    prefix.extend_from_slice(name_bytes);
    with_crc(prefix)
}

pub fn send_file_announce(file_name: &str, content: &[u8]) -> Vec<u8> {
    let name_bytes = file_name.as_bytes();
    assert!(name_bytes.len() <= 255, "nom de fichier trop long");
    let size = content.len() as u32;
    let sha256 = Sha256::digest(content);

    let mut prefix = vec![OP_SEND_FILE, name_bytes.len() as u8];
    prefix.extend_from_slice(name_bytes);
    prefix.extend_from_slice(&size.to_le_bytes());
    prefix.extend_from_slice(&sha256);
    with_crc(prefix)
}

pub fn update_playlist_status_description(status: u8) -> String {
    firmware_error_catalog::update_playlist_status(status)
}

pub fn update_playlist(file_name: &str) -> Vec<u8> {
    let mut prefix = vec![OP_UPDATE_PLAYLIST];
    prefix.extend_from_slice(file_name.as_bytes());
    with_crc(prefix)
}

pub fn delete_file(file_name: &str) -> Vec<u8> {
    let mut prefix = vec![OP_DELETE_FILE];
    prefix.extend_from_slice(file_name.as_bytes());
    with_crc(prefix)
}

pub fn get_number_of_files() -> Vec<u8> {
    with_crc(vec![OP_GET_NUMBER_OF_FILES])
}

pub fn get_file_information(index: u16) -> Vec<u8> {
    let mut prefix = vec![OP_GET_FILE_INFORMATION];
    prefix.extend_from_slice(&index.to_le_bytes());
    prefix.push(0x00);
    with_crc(prefix)
}

pub fn download_file(file_name: &str) -> Vec<u8> {
    let mut prefix = vec![OP_DOWNLOAD_FILE];
    prefix.extend_from_slice(file_name.as_bytes());
    with_crc(prefix)
}

pub fn set_wifi_config(ssid: &str, password: &str) -> Vec<u8> {
    let ssid_bytes = ssid.as_bytes();
    let password_bytes = password.as_bytes();
    assert!(
        ssid_bytes.len() <= 255 && password_bytes.len() <= 255,
        "trop long"
    );
    let mut prefix = vec![
        OP_SET_WIFI_CONFIG,
        0x00,
        ssid_bytes.len() as u8,
        password_bytes.len() as u8,
    ];
    prefix.extend_from_slice(ssid_bytes);
    prefix.extend_from_slice(password_bytes);
    with_crc(prefix)
}

fn with_crc(mut prefix: Vec<u8>) -> Vec<u8> {
    let crc = crc32_mpeg2::checksum_le(&prefix);
    prefix.extend_from_slice(&crc);
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_set_wifi_config_suffix() {
        let body = set_wifi_config("MERLIN_C59B2B", "MERLIN_APP");
        let hex: String = body.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "10000d0a4d45524c494e5f4335394232424d45524c494e5f415050ac1ab85d"
        );
    }

    #[test]
    fn handshake_magics_are_precomputed_crcs() {
        for (body, opcode) in [
            (connect(), OP_CONNECT),
            (get_mac_address(), OP_MAC_ADDRESS),
            (get_session_token(), OP_SESSION_TOKEN),
            (get_firmware_info(), OP_FIRMWARE_INFO),
            (get_telemetry_0e(), OP_TELEMETRY_0E),
            (get_telemetry_1b(), OP_TELEMETRY_1B),
            (get_hardware_id(), OP_GET_HARDWARE_ID),
            (end_synchronization(), OP_END_SYNCHRONIZATION),
            (set_enabling_hours_disabled(), OP_SET_ENABLING_HOURS),
        ] {
            let (prefix, suffix) = body.split_at(body.len() - 4);
            assert_eq!(body[0], opcode);
            assert_eq!(
                suffix,
                crate::crc32_mpeg2::checksum_le(prefix),
                "suffixe magique incohérent pour l'opcode 0x{opcode:02x}"
            );
        }
    }

    #[test]
    fn send_file_announce_layout() {
        let body = send_file_announce("a.mp3", &[1, 2, 3]);

        assert_eq!(body.len(), 1 + 1 + 5 + 4 + 32 + 4);
        assert_eq!(body[0], OP_SEND_FILE);
        assert_eq!(body[1], 5);
        assert_eq!(&body[2..7], b"a.mp3");
        assert_eq!(&body[7..11], &3u32.to_le_bytes());
        assert_crc_suffix(&body);
    }

    fn assert_crc_suffix(body: &[u8]) {
        let (prefix, suffix) = body.split_at(body.len() - 4);
        assert_eq!(
            suffix,
            crate::crc32_mpeg2::checksum_le(prefix),
            "CRC de fin incohérent"
        );
    }

    #[test]
    fn set_date_layout() {
        let body = set_date(Some(0x1122_3344));
        assert_eq!(body.len(), 1 + 4 + 4);
        assert_eq!(body[0], OP_SET_DATE);
        assert_eq!(&body[1..5], &0x1122_3344u32.to_le_bytes());
        assert_crc_suffix(&body);
    }

    #[test]
    fn search_file_layout() {
        let body = search_file("playlist.bin");
        assert_eq!(body[0], OP_SEARCH_FILE);
        assert_eq!(body[1], 0x01);
        assert_eq!(&body[2..14], b"playlist.bin");
        assert_eq!(body.len(), 2 + 12 + 4);
        assert_crc_suffix(&body);
    }

    #[test]
    fn update_playlist_layout() {
        let body = update_playlist("playlist.json");
        assert_eq!(body[0], OP_UPDATE_PLAYLIST);
        assert_eq!(&body[1..14], b"playlist.json");
        assert_eq!(body.len(), 1 + 13 + 4);
        assert_crc_suffix(&body);
    }

    #[test]
    fn delete_file_layout() {
        let body = delete_file("abc.mp3");
        assert_eq!(body[0], OP_DELETE_FILE);
        assert_eq!(&body[1..8], b"abc.mp3");
        assert_eq!(body.len(), 1 + 7 + 4);
        assert_crc_suffix(&body);
    }

    #[test]
    fn download_file_layout() {
        let body = download_file("song.mp3");
        assert_eq!(body[0], OP_DOWNLOAD_FILE);
        assert_eq!(&body[1..9], b"song.mp3");
        assert_eq!(body.len(), 1 + 8 + 4);
        assert_crc_suffix(&body);
    }

    #[test]
    fn get_number_of_files_layout() {
        let body = get_number_of_files();
        assert_eq!(body.len(), 1 + 4);
        assert_eq!(body[0], OP_GET_NUMBER_OF_FILES);
        assert_crc_suffix(&body);
    }

    #[test]
    fn get_file_information_uses_16bit_le_index_and_reserved_byte() {
        let body = get_file_information(0x0102);

        assert_eq!(body.len(), 1 + 2 + 1 + 4);
        assert_eq!(body[0], OP_GET_FILE_INFORMATION);
        assert_eq!(&body[1..3], &0x0102u16.to_le_bytes());
        assert_eq!(body[3], 0x00, "octet réservé");
        assert_crc_suffix(&body);

        let big = get_file_information(300);
        assert_eq!(&big[1..3], &300u16.to_le_bytes());
    }
}
