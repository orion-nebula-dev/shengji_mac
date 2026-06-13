use std::path::PathBuf;

use crate::{
    app::settings_service, infra::sqlite::open_connection, query_runtime_status, query_sessions,
    query_todos, AppState, BootstrapData,
};

pub(crate) fn get_bootstrap_data_payload(db_path: &PathBuf) -> Result<BootstrapData, String> {
    let connection = open_connection(db_path)?;
    Ok(BootstrapData {
        settings: settings_service::load_settings(&connection)?,
        todos: query_todos(&connection)?,
        sessions: query_sessions(&connection)?,
        runtime: query_runtime_status(&connection)?,
    })
}

#[tauri::command]
pub(crate) fn get_bootstrap_data(
    state: tauri::State<'_, AppState>,
) -> Result<BootstrapData, String> {
    get_bootstrap_data_payload(&state.db_path)
}
