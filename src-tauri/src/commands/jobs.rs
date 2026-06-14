use std::path::PathBuf;

use crate::{
    app::query_service, domain::processing::ProcessingActionResult, infra::sqlite::open_connection,
    process_pending_jobs_internal, AppState,
};

pub(crate) fn process_pending_jobs_payload(
    db_path: &PathBuf,
) -> Result<ProcessingActionResult, String> {
    let connection = open_connection(db_path)?;
    let message = process_pending_jobs_internal(&connection)?;
    Ok(ProcessingActionResult {
        message,
        runtime: query_service::query_runtime_status(&connection)?,
        latest_session: query_service::latest_session(&connection)?,
        todos: query_service::query_todos(&connection)?,
        sessions: query_service::query_sessions(&connection)?,
    })
}

#[tauri::command]
pub(crate) fn process_pending_jobs(
    state: tauri::State<'_, AppState>,
) -> Result<ProcessingActionResult, String> {
    process_pending_jobs_payload(&state.db_path)
}
