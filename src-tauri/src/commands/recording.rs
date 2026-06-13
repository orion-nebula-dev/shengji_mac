use std::path::PathBuf;

use rusqlite::params;

use crate::{
    app::settings_service, current_timestamp_label, infra::sqlite::open_connection,
    insert_processing_job, maybe_create_idle_session, process_pending_jobs_internal,
    query_runtime_status, AppState, RecordingActionResult,
};

pub(crate) fn simulate_audio_slice_payload(
    db_path: &PathBuf,
    has_effective_voice: bool,
) -> Result<RecordingActionResult, String> {
    let connection = open_connection(db_path)?;
    let settings = settings_service::load_settings(&connection)?;
    let timestamp = current_timestamp_label();
    let segment_id = format!("audio_sim_{timestamp}");
    let trace_id = format!("trace_sim_{timestamp}");

    connection
        .execute(
            r#"
      INSERT INTO audio_segments (
        id,
        file_path,
        started_at,
        ended_at,
        duration_ms,
        sample_rate,
        channels,
        has_effective_voice,
        voice_energy_score,
        processing_status,
        trace_id,
        created_at
      ) VALUES (
        ?1,
        ?2,
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP,
        ?3,
        16000,
        1,
        ?4,
        ?5,
        ?6,
        ?7,
        CURRENT_TIMESTAMP
      )
      "#,
            params![
                segment_id.as_str(),
                format!("/mock/{segment_id}.wav"),
                settings.chunk_seconds * 1000,
                if has_effective_voice { 1 } else { 0 },
                if has_effective_voice { 0.82 } else { 0.05 },
                if has_effective_voice {
                    "transcribed"
                } else {
                    "skipped"
                },
                trace_id.as_str()
            ],
        )
        .map_err(|error| format!("写入模拟切片失败: {error}"))?;

    if has_effective_voice {
        insert_processing_job(&connection, "transcription", &segment_id, &trace_id)?;
    }

    let latest_session = if has_effective_voice {
        None
    } else {
        maybe_create_idle_session(&connection)?
    };
    let processing_summary = process_pending_jobs_internal(&connection)?;

    Ok(RecordingActionResult {
        message: if has_effective_voice {
            format!("已写入一条有效录音切片。{processing_summary}")
        } else {
            format!("已写入一条静默切片，并检查空闲触发会话。{processing_summary}")
        },
        runtime: query_runtime_status(&connection)?,
        latest_session,
    })
}

#[tauri::command]
pub(crate) fn simulate_audio_slice(
    has_effective_voice: bool,
    state: tauri::State<'_, AppState>,
) -> Result<RecordingActionResult, String> {
    simulate_audio_slice_payload(&state.db_path, has_effective_voice)
}
