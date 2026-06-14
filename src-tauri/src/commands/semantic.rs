use std::path::PathBuf;

use crate::{
    app::semantic_service,
    domain::{
        artifact::{SemanticArtifactDto, SemanticWorkbenchDto},
        correction::{CorrectionPatternDto, DeletedCorrectionPatternDto, TranscriptRevisionDto},
    },
    infra::sqlite::open_connection,
    AppState,
};

pub(crate) fn generate_semantic_workbench_payload(
    db_path: &PathBuf,
) -> Result<SemanticWorkbenchDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::generate_semantic_workbench(&connection)
}

pub(crate) fn get_semantic_workbench_payload(
    db_path: &PathBuf,
) -> Result<SemanticWorkbenchDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::get_semantic_workbench(&connection)
}

pub(crate) fn set_correction_pattern_enabled_payload(
    db_path: &PathBuf,
    pattern_id: &str,
    enabled: bool,
) -> Result<CorrectionPatternDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::set_correction_pattern_enabled(&connection, pattern_id, enabled)
}

pub(crate) fn delete_correction_pattern_payload(
    db_path: &PathBuf,
    pattern_id: &str,
) -> Result<DeletedCorrectionPatternDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::delete_correction_pattern(&connection, pattern_id)
}

#[cfg(test)]
pub(crate) fn record_semantic_parse_failure_payload(
    db_path: &PathBuf,
    session_id: &str,
    raw_response: &str,
) -> Result<SemanticArtifactDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::record_parse_failure_unchecked(&connection, session_id, raw_response)
}

pub(crate) fn retry_semantic_artifact_payload(
    db_path: &PathBuf,
    artifact_id: &str,
) -> Result<SemanticArtifactDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::retry_semantic_artifact(&connection, artifact_id)
}

pub(crate) fn reject_transcript_revision_payload(
    db_path: &PathBuf,
    revision_id: &str,
) -> Result<TranscriptRevisionDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::reject_transcript_revision(&connection, revision_id)
}

#[tauri::command]
pub(crate) fn generate_semantic_workbench(
    state: tauri::State<'_, AppState>,
) -> Result<SemanticWorkbenchDto, String> {
    generate_semantic_workbench_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn get_semantic_workbench(
    state: tauri::State<'_, AppState>,
) -> Result<SemanticWorkbenchDto, String> {
    get_semantic_workbench_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn set_correction_pattern_enabled(
    pattern_id: String,
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<CorrectionPatternDto, String> {
    set_correction_pattern_enabled_payload(&state.db_path, &pattern_id, enabled)
}

#[tauri::command]
pub(crate) fn delete_correction_pattern(
    pattern_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<DeletedCorrectionPatternDto, String> {
    delete_correction_pattern_payload(&state.db_path, &pattern_id)
}

#[tauri::command]
pub(crate) fn retry_semantic_artifact(
    artifact_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SemanticArtifactDto, String> {
    retry_semantic_artifact_payload(&state.db_path, &artifact_id)
}

#[tauri::command]
pub(crate) fn reject_transcript_revision(
    revision_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<TranscriptRevisionDto, String> {
    reject_transcript_revision_payload(&state.db_path, &revision_id)
}
