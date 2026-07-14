use std::time::{Duration, Instant};

use tauri::State;
use tracing::{debug, warn};

use merlin_infra::persistence::db;
use merlin_infra::persistence::device_repository::SqliteDeviceRepository;
use merlin_infra::persistence::subscription_repository::SqliteSubscriptionRepository;
use merlin_infra::persistence::sync_state_repository::SqliteSyncStateRepository;
use merlin_protocol::client::{Frame, MerlinClient};
use merlin_protocol::commands;

use crate::dto::ConnectionStatus;
use crate::state::AppState;

#[tauri::command]
pub async fn test_connection(
    host: String,
    port: u16,
    manual: bool,
    state: State<'_, AppState>,
) -> Result<ConnectionStatus, String> {
    let lock = state.op_lock.clone();

    let _guard = if manual {
        lock.lock_owned().await
    } else {
        match lock.try_lock_owned() {
            Ok(guard) => guard,
            Err(_) => {
                return Ok(ConnectionStatus {
                    connected: false,
                    latency_ms: None,
                    message: None,
                    busy: true,
                    device_mac: None,
                    device_name: None,
                    newly_registered: false,
                });
            }
        }
    };

    let start = Instant::now();
    match probe_handshake(&host, port).await {
        Ok((hex, mac)) => {
            let newly_registered = mac
                .as_deref()
                .map(|m| remember_device(&state, m))
                .unwrap_or(false);
            Ok(ConnectionStatus {
                connected: true,
                latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
                message: manual.then(|| format!("Connecte - handshake OK (reponse: {hex})")),
                busy: false,
                device_name: mac.as_deref().map(device_name),
                device_mac: mac,
                newly_registered,
            })
        }
        Err(error) => Ok(ConnectionStatus {
            connected: false,
            latency_ms: None,
            message: manual.then_some(format!("Erreur : {error}")),
            busy: false,
            device_mac: None,
            device_name: None,
            newly_registered: false,
        }),
    }
}

#[tauri::command]
pub async fn check_internet() -> bool {
    merlin_infra::podcasts::podcast_search::has_internet().await
}

const DEFAULT_DEVICE_NAME: &str = "Merlin";

fn remember_device(state: &AppState, mac: &str) -> bool {
    {
        let mut guard = state.current_device.lock().expect("lock");
        if guard.as_deref() == Some(mac) {
            return false;
        }
        *guard = Some(mac.to_string());
    }

    let mut newly_registered = false;
    if let Ok(connection) = db::open(&state.db_path) {
        let devices = SqliteDeviceRepository::new(connection);
        newly_registered = devices.register(mac, DEFAULT_DEVICE_NAME);
        devices.set_active(Some(mac));
    }

    if let Ok(connection) = db::open(&state.db_path) {
        SqliteSubscriptionRepository::new(connection, mac).claim_legacy();
    }
    if let Ok(connection) = db::open(&state.db_path) {
        let orphans = SqliteSyncStateRepository::new(connection, mac).orphan_record_count();
        if orphans > 0 {
            warn!(
                "{orphans} enregistrement(s) de synchro herite(s) d'avant le multi-enceinte : \
                 ils ne sont attribues a aucune enceinte, les episodes concernes seront renvoyes une fois"
            );
        }
    }
    newly_registered
}

async fn probe_handshake(host: &str, port: u16) -> Result<(String, Option<String>), String> {
    let mut client = MerlinClient::new(host, port);
    client.connect(Duration::from_secs(5));
    let connect = client
        .send_frame_command(&commands::connect(), Duration::from_secs(5), None)
        .await;
    let outcome = match connect {
        Ok(frames) if frames.is_empty() => Err("Aucune reponse de l'enceinte.".to_string()),
        Ok(frames) => {
            let hex = frames[0]
                .body
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();

            let mac = client
                .send_frame_command(&commands::get_hardware_id(), Duration::from_secs(5), None)
                .await
                .ok()
                .and_then(|frames| parse_mac(&frames));
            Ok((hex, mac))
        }
        Err(error) => Err(error.to_string()),
    };
    client.close().await;
    outcome
}

const MAC_BODY_LEN: usize = 1 + 6 + 4;

fn device_name(mac: &str) -> String {
    format!("MERLIN_{}", mac.to_uppercase())
}

fn parse_mac(frames: &[Frame]) -> Option<String> {
    let frame = frames
        .iter()
        .find(|f| f.opcode() == Some(commands::OP_GET_HARDWARE_ID))?;
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    debug!(
        "getHardwareId : corps de {} octet(s) : {}",
        frame.body.len(),
        hex(&frame.body)
    );

    if frame.body.len() != MAC_BODY_LEN {
        warn!(
            "getHardwareId : corps de {} octet(s), {MAC_BODY_LEN} attendus - identifiant \
             d'enceinte a verifier (octets : {})",
            frame.body.len(),
            hex(&frame.body)
        );
    }
    let mac = frame.body.get(1..7)?;
    Some(
        mac.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_name_matches_the_firmware_softap_ssid_format() {
        assert_eq!(device_name("aa:bb:cc:11:22:33"), "MERLIN_AA:BB:CC:11:22:33");
    }

    fn hardware_id_frame(mac: [u8; 6], crc: [u8; 4]) -> Vec<Frame> {
        let mut body = vec![commands::OP_GET_HARDWARE_ID];
        body.extend_from_slice(&mac);
        body.extend_from_slice(&crc);
        vec![Frame {
            length: body.len() as u8,
            body,
        }]
    }

    #[test]
    fn two_distinct_speakers_yield_two_distinct_ids() {
        let one = hardware_id_frame(
            [0xaa, 0xbb, 0xcc, 0x00, 0x00, 0x01],
            [0x1e, 0xbd, 0x54, 0x9b],
        );
        let two = hardware_id_frame(
            [0xaa, 0xbb, 0xcc, 0x00, 0x00, 0x02],
            [0x00, 0x00, 0x00, 0x00],
        );
        assert_eq!(parse_mac(&one).as_deref(), Some("aa:bb:cc:00:00:01"));
        assert_eq!(parse_mac(&two).as_deref(), Some("aa:bb:cc:00:00:02"));
        assert_ne!(
            parse_mac(&one),
            parse_mac(&two),
            "deux enceintes, deux device_id"
        );
    }

    #[test]
    fn a_zero_x04_response_is_ignored_by_parse_mac() {
        let body = vec![
            commands::OP_MAC_ADDRESS,
            0x00,
            0x00,
            0x50,
            0xee,
            0x33,
            0x49,
            0xd4,
            0xcf,
        ];
        let frames = vec![Frame {
            length: body.len() as u8,
            body,
        }];
        assert_eq!(
            parse_mac(&frames),
            None,
            "0x04 n'est plus une source d'identite"
        );
    }

    #[test]
    fn parse_mac_returns_none_when_the_frame_is_absent() {
        assert_eq!(parse_mac(&[]), None);
    }
}
