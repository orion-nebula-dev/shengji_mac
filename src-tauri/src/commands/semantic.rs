use std::path::PathBuf;

use crate::{
    app::semantic_service,
    domain::{
        artifact::{
            AddResearchToMindMapCommand, ConvertResearchToTodoCommand, MindMapExportDto,
            SemanticArtifactDto, SemanticWorkbenchDto, StartResearchFromSegmentCommand,
            ToggleMindMapNodeCommand, UpdateMindMapNodeCommand,
        },
        correction::{CorrectionPatternDto, DeletedCorrectionPatternDto, TranscriptRevisionDto},
        todo::TodoDto,
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

pub(crate) fn generate_mind_map_payload(
    db_path: &PathBuf,
) -> Result<SemanticArtifactDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::generate_mind_map(&connection)
}

pub(crate) fn update_mind_map_node_payload(
    db_path: &PathBuf,
    command: UpdateMindMapNodeCommand,
) -> Result<SemanticArtifactDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::update_mind_map_node(&connection, command)
}

pub(crate) fn toggle_mind_map_node_payload(
    db_path: &PathBuf,
    command: ToggleMindMapNodeCommand,
) -> Result<SemanticArtifactDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::toggle_mind_map_node(&connection, command)
}

pub(crate) fn export_mind_map_payload(
    db_path: &PathBuf,
    artifact_id: &str,
    format: &str,
) -> Result<MindMapExportDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::export_mind_map(&connection, artifact_id, format)
}

pub(crate) fn generate_value_discovery_payload(
    db_path: &PathBuf,
) -> Result<SemanticArtifactDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::generate_value_discovery(&connection)
}

pub(crate) fn start_research_from_segment_payload(
    db_path: &PathBuf,
    command: StartResearchFromSegmentCommand,
) -> Result<SemanticArtifactDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::start_research_from_segment(&connection, command)
}

pub(crate) fn convert_research_to_todo_payload(
    db_path: &PathBuf,
    command: ConvertResearchToTodoCommand,
) -> Result<TodoDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::convert_research_to_todo(&connection, command)
}

pub(crate) fn add_research_to_mind_map_payload(
    db_path: &PathBuf,
    command: AddResearchToMindMapCommand,
) -> Result<SemanticArtifactDto, String> {
    let connection = open_connection(db_path)?;
    semantic_service::add_research_to_mind_map(&connection, command)
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

#[tauri::command]
pub(crate) fn generate_mind_map(
    state: tauri::State<'_, AppState>,
) -> Result<SemanticArtifactDto, String> {
    generate_mind_map_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn update_mind_map_node(
    command: UpdateMindMapNodeCommand,
    state: tauri::State<'_, AppState>,
) -> Result<SemanticArtifactDto, String> {
    update_mind_map_node_payload(&state.db_path, command)
}

#[tauri::command]
pub(crate) fn toggle_mind_map_node(
    command: ToggleMindMapNodeCommand,
    state: tauri::State<'_, AppState>,
) -> Result<SemanticArtifactDto, String> {
    toggle_mind_map_node_payload(&state.db_path, command)
}

#[tauri::command]
pub(crate) fn export_mind_map(
    artifact_id: String,
    format: String,
    state: tauri::State<'_, AppState>,
) -> Result<MindMapExportDto, String> {
    export_mind_map_payload(&state.db_path, &artifact_id, &format)
}

#[tauri::command]
pub(crate) fn generate_value_discovery(
    state: tauri::State<'_, AppState>,
) -> Result<SemanticArtifactDto, String> {
    generate_value_discovery_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn start_research_from_segment(
    command: StartResearchFromSegmentCommand,
    state: tauri::State<'_, AppState>,
) -> Result<SemanticArtifactDto, String> {
    start_research_from_segment_payload(&state.db_path, command)
}

#[tauri::command]
pub(crate) fn convert_research_to_todo(
    command: ConvertResearchToTodoCommand,
    state: tauri::State<'_, AppState>,
) -> Result<TodoDto, String> {
    convert_research_to_todo_payload(&state.db_path, command)
}

#[tauri::command]
pub(crate) fn add_research_to_mind_map(
    command: AddResearchToMindMapCommand,
    state: tauri::State<'_, AppState>,
) -> Result<SemanticArtifactDto, String> {
    add_research_to_mind_map_payload(&state.db_path, command)
}
