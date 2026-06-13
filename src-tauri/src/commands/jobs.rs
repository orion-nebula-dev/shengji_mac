use std::path::PathBuf;

use crate::{
    domain::processing::ProcessingActionResult, infra::sqlite::open_connection, latest_session,
    process_pending_jobs_internal, query_runtime_status, query_sessions, query_todos, AppState,
};

pub(crate) fn process_pending_jobs_payload(
    db_path: &PathBuf,
) -> Result<ProcessingActionResult, String> {
    let connection = open_connection(db_path)?;
    let message = process_pending_jobs_internal(&connection)?;
    Ok(ProcessingActionResult {
        message,
        runtime: query_runtime_status(&connection)?,
        latest_session: latest_session(&connection)?,
        todos: query_todos(&connection)?,
        sessions: query_sessions(&connection)?,
    })
}

#[tauri::command]
pub(crate) fn process_pending_jobs(
    state: tauri::State<'_, AppState>,
) -> Result<ProcessingActionResult, String> {
    process_pending_jobs_payload(&state.db_path)
}
