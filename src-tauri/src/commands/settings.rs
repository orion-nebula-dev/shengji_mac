use std::path::PathBuf;

use crate::{
    app::settings_service, domain::settings::SettingsDto, infra::sqlite::open_connection, AppState,
};

pub(crate) fn save_settings_payload(
    db_path: &PathBuf,
    payload: SettingsDto,
) -> Result<SettingsDto, String> {
    let connection = open_connection(db_path)?;
    settings_service::save_settings(&connection, &payload)?;
    settings_service::load_settings(&connection)
}

#[tauri::command]
pub(crate) fn save_settings(
    payload: SettingsDto,
    state: tauri::State<'_, AppState>,
) -> Result<SettingsDto, String> {
    save_settings_payload(&state.db_path, payload)
}
