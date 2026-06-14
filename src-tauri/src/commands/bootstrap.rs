use std::path::PathBuf;

use crate::{
    app::{query_service, settings_service},
    domain::bootstrap::BootstrapData,
    infra::sqlite::open_connection,
    AppState,
};

pub(crate) fn get_bootstrap_data_payload(db_path: &PathBuf) -> Result<BootstrapData, String> {
    let connection = open_connection(db_path)?;
    Ok(BootstrapData {
        settings: settings_service::load_settings(&connection)?,
        todos: query_service::query_todos(&connection)?,
        sessions: query_service::query_sessions(&connection)?,
        runtime: query_service::query_runtime_status(&connection)?,
    })
}

#[tauri::command]
pub(crate) fn get_bootstrap_data(
    state: tauri::State<'_, AppState>,
) -> Result<BootstrapData, String> {
    get_bootstrap_data_payload(&state.db_path)
}
