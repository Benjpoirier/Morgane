use std::time::Duration;

use merlin_protocol::client::{Frame, MerlinClient};
use merlin_protocol::commands;

use merlin_domain::sync::types::SyncError;

use super::error::EngineError;

const INTER_COMMAND_DELAY: Duration = Duration::from_millis(80);

pub type LogFn<'a> = &'a (dyn Fn(&str) + Send + Sync);

pub fn noop_log(_: &str) {}

pub async fn open(
    host: &str,
    port: u16,
    timeout: Duration,
    log: LogFn<'_>,
) -> Result<MerlinClient, EngineError> {
    let mut client = MerlinClient::new(host, port);
    client.connect(timeout);
    match handshake(&mut client, timeout, log).await {
        Ok(()) => Ok(client),
        Err(e) => {
            client.close().await;
            Err(e)
        }
    }
}

pub async fn send(
    client: &mut MerlinClient,
    name: &str,
    body: &[u8],
    timeout: Duration,
) -> Result<Vec<Frame>, EngineError> {
    let expected_opcode = body[0];
    let has_opcode =
        move |frames: &[Frame]| frames.iter().any(|f| f.opcode() == Some(expected_opcode));
    let frames = client
        .send_frame_command(body, timeout, Some(&has_opcode))
        .await?;

    if !frames.iter().any(|f| f.opcode() == Some(expected_opcode)) {
        return Err(SyncError::NoResponse(name.to_string()).into());
    }
    tokio::time::sleep(INTER_COMMAND_DELAY).await;
    Ok(frames)
}

pub async fn handshake(
    client: &mut MerlinClient,
    timeout: Duration,
    log: LogFn<'_>,
) -> Result<(), EngineError> {
    log("Connexion a l'enceinte...");
    send(client, "connect", &commands::connect(), timeout).await?;
    log("Recuperation de l'adresse MAC...");
    send(
        client,
        "getMacAddress",
        &commands::get_mac_address(),
        timeout,
    )
    .await?;
    log("Jeton de session...");
    send(
        client,
        "getSessionToken",
        &commands::get_session_token(),
        timeout,
    )
    .await?;
    log("Telemetrie...");
    send(
        client,
        "getTelemetry0e",
        &commands::get_telemetry_0e(),
        timeout,
    )
    .await?;
    log("Informations firmware...");
    send(
        client,
        "getFirmwareInfo",
        &commands::get_firmware_info(),
        timeout,
    )
    .await?;
    log("Identifiant materiel...");
    send(
        client,
        "getHardwareId",
        &commands::get_hardware_id(),
        timeout,
    )
    .await?;
    log("Telemetrie...");
    send(
        client,
        "getTelemetry1b",
        &commands::get_telemetry_1b(),
        timeout,
    )
    .await?;
    log("Connexion a l'enceinte etablie.");
    Ok(())
}
