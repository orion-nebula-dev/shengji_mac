use std::path::PathBuf;

pub(crate) use crate::domain::todo::{AcceptTodoCandidateCommand, UpdateTodoCandidateCommand};

use crate::{
    app::todo_service,
    domain::todo::{TodoCandidateDto, TodoDto},
    infra::sqlite::open_connection,
    AppState,
};

pub(crate) fn toggle_todo_status_payload(
    db_path: &PathBuf,
    todo_id: &str,
) -> Result<TodoDto, String> {
    let connection = open_connection(db_path)?;
    let current = todo_service::query_todo(&connection, todo_id)?;
    let next_status = match current.status.as_str() {
        "open" => "done",
        "in_progress" => "done",
        "done" => "open",
        "dismissed" => "open",
        "pending" => "done",
        "completed" => "open",
        _ => "open",
    };
    todo_service::update_todo_status(&connection, todo_id, next_status)
}

pub(crate) fn update_todo_status_payload(
    db_path: &PathBuf,
    todo_id: &str,
    status: &str,
) -> Result<TodoDto, String> {
    let connection = open_connection(db_path)?;
    todo_service::update_todo_status(&connection, todo_id, status)
}

pub(crate) fn sync_todo_candidates_payload(
    db_path: &PathBuf,
) -> Result<Vec<TodoCandidateDto>, String> {
    let connection = open_connection(db_path)?;
    todo_service::sync_todo_candidates(&connection)
}

pub(crate) fn list_todo_candidates_payload(
    db_path: &PathBuf,
) -> Result<Vec<TodoCandidateDto>, String> {
    let connection = open_connection(db_path)?;
    todo_service::query_todo_candidates(&connection)
}

pub(crate) fn accept_todo_candidate_payload(
    db_path: &PathBuf,
    command: AcceptTodoCandidateCommand,
) -> Result<TodoDto, String> {
    let connection = open_connection(db_path)?;
    todo_service::accept_todo_candidate(&connection, command)
}

pub(crate) fn update_todo_candidate_payload(
    db_path: &PathBuf,
    command: UpdateTodoCandidateCommand,
) -> Result<TodoCandidateDto, String> {
    let connection = open_connection(db_path)?;
    todo_service::update_todo_candidate(&connection, command)
}

pub(crate) fn dismiss_todo_candidate_payload(
    db_path: &PathBuf,
    candidate_id: &str,
) -> Result<TodoCandidateDto, String> {
    let connection = open_connection(db_path)?;
    todo_service::dismiss_todo_candidate(&connection, candidate_id)
}

#[tauri::command]
pub(crate) fn toggle_todo_status(
    todo_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<TodoDto, String> {
    toggle_todo_status_payload(&state.db_path, &todo_id)
}

#[tauri::command]
pub(crate) fn update_todo_status(
    todo_id: String,
    status: String,
    state: tauri::State<'_, AppState>,
) -> Result<TodoDto, String> {
    update_todo_status_payload(&state.db_path, &todo_id, &status)
}

#[tauri::command]
pub(crate) fn sync_todo_candidates(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<TodoCandidateDto>, String> {
    sync_todo_candidates_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn list_todo_candidates(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<TodoCandidateDto>, String> {
    list_todo_candidates_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn accept_todo_candidate(
    command: AcceptTodoCandidateCommand,
    state: tauri::State<'_, AppState>,
) -> Result<TodoDto, String> {
    accept_todo_candidate_payload(&state.db_path, command)
}

#[tauri::command]
pub(crate) fn update_todo_candidate(
    command: UpdateTodoCandidateCommand,
    state: tauri::State<'_, AppState>,
) -> Result<TodoCandidateDto, String> {
    update_todo_candidate_payload(&state.db_path, command)
}

#[tauri::command]
pub(crate) fn dismiss_todo_candidate(
    candidate_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<TodoCandidateDto, String> {
    dismiss_todo_candidate_payload(&state.db_path, &candidate_id)
}
