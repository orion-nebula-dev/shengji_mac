use std::path::PathBuf;

use rusqlite::params;

use crate::{
    app::{query_service, settings_service},
    current_timestamp_label,
    domain::recording::RecordingActionResult,
    infra::sqlite::open_connection,
    insert_audio_segment, insert_processing_job, maybe_create_idle_session,
    process_pending_jobs_internal, spawn_recording_controller, AppState, RecorderControl,
};

fn recording_file_label(file_path: &std::path::Path) -> String {
    file_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("录音文件.wav")
        .to_string()
}

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
        runtime: query_service::query_runtime_status(&connection)?,
        latest_session,
    })
}

pub(crate) fn start_recording_payload(state: &AppState) -> Result<RecordingActionResult, String> {
    let mut recorder_guard = state
        .recorder
        .lock()
        .map_err(|_| "录音状态锁定失败".to_string())?;
    if recorder_guard.is_some() {
        let connection = open_connection(&state.db_path)?;
        return Ok(RecordingActionResult {
            message: "录音已在进行中".into(),
            runtime: query_service::query_runtime_status(&connection)?,
            latest_session: query_service::latest_session(&connection)?,
        });
    }

    let controller = spawn_recording_controller(state.recordings_dir.clone())?;
    let connection = open_connection(&state.db_path)?;
    settings_service::set_record_enabled(&connection, true)?;
    *recorder_guard = Some(controller);

    Ok(RecordingActionResult {
        message: "已启动真实麦克风录音".into(),
        runtime: query_service::query_runtime_status(&connection)?,
        latest_session: query_service::latest_session(&connection)?,
    })
}

pub(crate) fn stop_recording_payload(state: &AppState) -> Result<RecordingActionResult, String> {
    let controller = state
        .recorder
        .lock()
        .map_err(|_| "录音状态锁定失败".to_string())?
        .take();

    let Some(controller) = controller else {
        let connection = open_connection(&state.db_path)?;
        return Ok(RecordingActionResult {
            message: "当前没有进行中的录音".into(),
            runtime: query_service::query_runtime_status(&connection)?,
            latest_session: query_service::latest_session(&connection)?,
        });
    };

    controller
        .stop_tx
        .send(RecorderControl::Stop)
        .map_err(|error| format!("发送停止录音指令失败: {error}"))?;
    let result = controller
        .join_handle
        .join()
        .map_err(|_| "录音线程异常退出".to_string())??;

    let connection = open_connection(&state.db_path)?;
    insert_audio_segment(
        &connection,
        &result.file_path,
        &result.started_at_label,
        result.duration_ms,
        result.sample_rate,
        result.channels,
        result.summary.total_energy,
        result.summary.sample_count,
        &result.trace_id,
    )?;
    settings_service::set_record_enabled(&connection, false)?;
    let processing_summary = process_pending_jobs_internal(&connection)?;

    Ok(RecordingActionResult {
        message: format!(
            "录音已停止，已保存本地 WAV 文件：{}。{}",
            recording_file_label(&result.file_path),
            processing_summary
        ),
        runtime: query_service::query_runtime_status(&connection)?,
        latest_session: query_service::latest_session(&connection)?,
    })
}

#[tauri::command]
pub(crate) fn simulate_audio_slice(
    has_effective_voice: bool,
    state: tauri::State<'_, AppState>,
) -> Result<RecordingActionResult, String> {
    simulate_audio_slice_payload(&state.db_path, has_effective_voice)
}

#[tauri::command]
pub(crate) fn start_recording(
    state: tauri::State<'_, AppState>,
) -> Result<RecordingActionResult, String> {
    start_recording_payload(&state)
}

#[tauri::command]
pub(crate) fn stop_recording(
    state: tauri::State<'_, AppState>,
) -> Result<RecordingActionResult, String> {
    stop_recording_payload(&state)
}
