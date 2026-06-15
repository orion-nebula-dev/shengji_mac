use std::path::PathBuf;

use crate::{
    app::local_asr_service,
    domain::local_asr::{LocalAsrModelStatusDto, LocalAsrStateDto},
    infra::sqlite::open_connection,
    AppState,
};

pub(crate) fn get_local_asr_state_payload(db_path: &PathBuf) -> Result<LocalAsrStateDto, String> {
    let connection = open_connection(db_path)?;
    local_asr_service::get_local_asr_state(&connection)
}

pub(crate) fn refresh_local_asr_runtimes_payload(
    db_path: &PathBuf,
) -> Result<LocalAsrStateDto, String> {
    let connection = open_connection(db_path)?;
    local_asr_service::refresh_local_asr_runtimes(&connection)?;
    local_asr_service::get_local_asr_state(&connection)
}

pub(crate) fn select_local_asr_model_payload(
    db_path: &PathBuf,
    model_name: &str,
) -> Result<LocalAsrStateDto, String> {
    let connection = open_connection(db_path)?;
    local_asr_service::select_local_asr_model(&connection, model_name)
}

pub(crate) fn download_local_asr_model_payload(
    db_path: &PathBuf,
    model_name: &str,
) -> Result<LocalAsrModelStatusDto, String> {
    let connection = open_connection(db_path)?;
    local_asr_service::download_local_asr_model(&connection, model_name)
}

#[tauri::command]
pub(crate) fn get_local_asr_state(
    state: tauri::State<'_, AppState>,
) -> Result<LocalAsrStateDto, String> {
    get_local_asr_state_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn refresh_local_asr_runtimes(
    state: tauri::State<'_, AppState>,
) -> Result<LocalAsrStateDto, String> {
    refresh_local_asr_runtimes_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn select_local_asr_model(
    model_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<LocalAsrStateDto, String> {
    select_local_asr_model_payload(&state.db_path, &model_name)
}

#[tauri::command]
pub(crate) fn download_local_asr_model(
    model_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<LocalAsrModelStatusDto, String> {
    download_local_asr_model_payload(&state.db_path, &model_name)
}
