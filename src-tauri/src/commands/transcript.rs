use std::path::PathBuf;

use crate::{
    app::transcript_service,
    domain::{
        speaker::SpeakerDto,
        transcript::{TranscriptJobDto, TranscriptReviewDto, TranscriptSegmentDto},
    },
    infra::sqlite::open_connection,
    AppState,
};

pub(crate) fn import_local_audio_payload(
    db_path: &PathBuf,
    file_path: &str,
) -> Result<TranscriptReviewDto, String> {
    let connection = open_connection(db_path)?;
    transcript_service::import_local_audio(&connection, file_path)
}

pub(crate) fn get_transcript_review_payload(
    db_path: &PathBuf,
) -> Result<TranscriptReviewDto, String> {
    let connection = open_connection(db_path)?;
    transcript_service::get_transcript_review(&connection)
}

pub(crate) fn rename_speaker_payload(
    db_path: &PathBuf,
    speaker_id: &str,
    label: &str,
) -> Result<SpeakerDto, String> {
    let connection = open_connection(db_path)?;
    transcript_service::rename_speaker(&connection, speaker_id, label)
}

pub(crate) fn mark_transcript_segment_payload(
    db_path: &PathBuf,
    segment_id: &str,
    issue_type: &str,
    reason: &str,
) -> Result<TranscriptSegmentDto, String> {
    let connection = open_connection(db_path)?;
    transcript_service::mark_transcript_segment(&connection, segment_id, issue_type, reason)
}

pub(crate) fn retry_transcript_job_payload(
    db_path: &PathBuf,
    job_id: &str,
) -> Result<TranscriptJobDto, String> {
    let connection = open_connection(db_path)?;
    transcript_service::retry_transcript_job(&connection, job_id)
}

#[tauri::command]
pub(crate) fn import_local_audio(
    file_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<TranscriptReviewDto, String> {
    import_local_audio_payload(&state.db_path, &file_path)
}

#[tauri::command]
pub(crate) fn get_transcript_review(
    state: tauri::State<'_, AppState>,
) -> Result<TranscriptReviewDto, String> {
    get_transcript_review_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn rename_speaker(
    speaker_id: String,
    label: String,
    state: tauri::State<'_, AppState>,
) -> Result<SpeakerDto, String> {
    rename_speaker_payload(&state.db_path, &speaker_id, &label)
}

#[tauri::command]
pub(crate) fn mark_transcript_segment(
    segment_id: String,
    issue_type: String,
    reason: String,
    state: tauri::State<'_, AppState>,
) -> Result<TranscriptSegmentDto, String> {
    mark_transcript_segment_payload(&state.db_path, &segment_id, &issue_type, &reason)
}

#[tauri::command]
pub(crate) fn retry_transcript_job(
    job_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<TranscriptJobDto, String> {
    retry_transcript_job_payload(&state.db_path, &job_id)
}
