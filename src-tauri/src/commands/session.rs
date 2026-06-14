use std::path::PathBuf;

use rusqlite::params;

use crate::{
    app::query_service, current_timestamp_label, domain::session::SessionDto,
    ensure_manual_flush_allowed, infra::sqlite::open_connection, insert_processing_job,
    process_pending_jobs_internal, AppState,
};

pub(crate) fn flush_current_session_payload(db_path: &PathBuf) -> Result<SessionDto, String> {
    let connection = open_connection(db_path)?;
    ensure_manual_flush_allowed(&connection)?;
    let timestamp = current_timestamp_label();
    let session_id = format!("session_manual_{timestamp}");
    let trace_id = format!("trace_manual_{timestamp}");
    let merged_text = "手动刷新会话，当前未绑定真实转写文稿。".to_string();

    connection
        .execute(
            r#"
      INSERT INTO conversation_sessions (
        id,
        merged_text,
        started_at,
        ended_at,
        idle_trigger_seconds,
        trigger_reason,
        transcript_count,
        extraction_status,
        extraction_provider_used,
        extraction_fallback_used,
        extraction_fallback_reason,
        trace_id,
        created_at
      ) VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', 0, 'pending', 'pending', 0, '', ?3, CURRENT_TIMESTAMP)
      "#,
            params![session_id.as_str(), merged_text.as_str(), trace_id.as_str()],
        )
        .map_err(|error| format!("写入手动会话失败: {error}"))?;
    insert_processing_job(&connection, "todo_extraction", &session_id, &trace_id)?;
    let _ = process_pending_jobs_internal(&connection)?;

    query_service::latest_session(&connection)?.ok_or_else(|| "未找到刚创建的会话".to_string())
}

#[tauri::command]
pub(crate) fn flush_current_session(
    state: tauri::State<'_, AppState>,
) -> Result<SessionDto, String> {
    flush_current_session_payload(&state.db_path)
}
