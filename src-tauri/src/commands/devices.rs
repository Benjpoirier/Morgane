use tauri::State;

use merlin_domain::library::device::RegisteredDevice;
use merlin_infra::persistence::db;
use merlin_infra::persistence::device_repository::SqliteDeviceRepository;

use crate::state::AppState;

fn repo(state: &AppState) -> Result<SqliteDeviceRepository, String> {
    db::open(&state.db_path)
        .map(SqliteDeviceRepository::new)
        .map_err(|e| format!("base locale inaccessible : {e}"))
}

#[tauri::command]
pub fn list_registered_devices(state: State<AppState>) -> Result<Vec<RegisteredDevice>, String> {
    Ok(repo(&state)?.all())
}

#[tauri::command]
pub fn set_active_device(mac: String, state: State<AppState>) -> Result<(), String> {
    repo(&state)?.set_active(Some(&mac));
    *state.current_device.lock().expect("lock") = Some(mac);
    Ok(())
}

#[tauri::command]
pub fn rename_registered_device(
    mac: String,
    name: String,
    state: State<AppState>,
) -> Result<(), String> {
    repo(&state)?.rename(&mac, &name);
    Ok(())
}

#[tauri::command]
pub fn remove_registered_device(mac: String, state: State<AppState>) -> Result<(), String> {
    let devices = repo(&state)?;
    devices.remove(&mac);

    if devices.active().is_none()
        && let Some(next) = devices.all().into_iter().next()
    {
        devices.set_active(Some(&next.mac));
    }
    *state.current_device.lock().expect("lock") = devices.active();
    Ok(())
}
