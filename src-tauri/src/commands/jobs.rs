use std::path::PathBuf;

use crate::{
    app::{query_service, runtime_observability_service},
    domain::{
        processing::ProcessingActionResult,
        runtime::{ProcessingJobDto, RuntimeDashboardDto, SegmentTimelineDto},
    },
    infra::sqlite::open_connection,
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

pub(crate) fn get_runtime_dashboard_payload(
    db_path: &PathBuf,
) -> Result<RuntimeDashboardDto, String> {
    let connection = open_connection(db_path)?;
    runtime_observability_service::query_runtime_dashboard(&connection)
}

pub(crate) fn get_segment_timeline_payload(
    db_path: &PathBuf,
    audio_segment_id: &str,
) -> Result<SegmentTimelineDto, String> {
    let connection = open_connection(db_path)?;
    runtime_observability_service::query_segment_timeline(&connection, audio_segment_id)
}

pub(crate) fn retry_processing_job_payload(
    db_path: &PathBuf,
    job_id: &str,
) -> Result<ProcessingJobDto, String> {
    let connection = open_connection(db_path)?;
    runtime_observability_service::retry_processing_job(&connection, job_id)
}

#[tauri::command]
pub(crate) fn process_pending_jobs(
    state: tauri::State<'_, AppState>,
) -> Result<ProcessingActionResult, String> {
    process_pending_jobs_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn get_runtime_dashboard(
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeDashboardDto, String> {
    get_runtime_dashboard_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn get_segment_timeline(
    audio_segment_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SegmentTimelineDto, String> {
    get_segment_timeline_payload(&state.db_path, &audio_segment_id)
}

#[tauri::command]
pub(crate) fn retry_processing_job(
    job_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ProcessingJobDto, String> {
    retry_processing_job_payload(&state.db_path, &job_id)
}
