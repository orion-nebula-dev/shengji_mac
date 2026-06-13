use rusqlite::{params, Connection};

use crate::{clip_text, current_timestamp_label, is_placeholder_session_text, SettingsDto};

fn register_semantic_todo_artifact_boundary(
    connection: &Connection,
    settings: &SettingsDto,
    session_id: &str,
    merged_text: &str,
) -> Result<(), String> {
    let payload = serde_json::json!({
        "boundary": "v0.4_semantic_provider",
        "status": "pending_provider_integration",
        "todo_candidates": [],
        "source_preview": clip_text(merged_text, 240),
    })
    .to_string();
    let artifact_id = format!("semantic_todo_{}", current_timestamp_label());

    connection
        .execute(
            "DELETE FROM semantic_artifacts WHERE session_id = ?1 AND artifact_type = 'todo_extraction'",
            params![session_id],
        )
        .map_err(|error| format!("清理已有 Todo 语义产物失败: {error}"))?;

    connection
        .execute(
            r#"
            INSERT INTO semantic_artifacts (
              id,
              session_id,
              artifact_type,
              status,
              provider,
              model_name,
              schema_version,
              source_span_refs,
              payload_json,
              error_message,
              created_at,
              updated_at
            ) VALUES (?1, ?2, 'todo_extraction', 'pending', ?3, ?4, 'v0.4', '[]', ?5, '', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
            params![
                artifact_id,
                session_id,
                settings.semantic_provider_type.as_str(),
                settings.semantic_model_name.as_str(),
                payload,
            ],
        )
        .map_err(|error| format!("登记 Todo 语义产物边界失败: {error}"))?;

    connection
        .execute(
            r#"
            UPDATE conversation_sessions
            SET
              extraction_status = 'success',
              extraction_provider_used = ?1,
              extraction_fallback_used = 0,
              extraction_fallback_reason = 'v0.4 仅登记 MiniMax M3 语义产物边界，实际 Todo 候选生成在后续版本接入'
            WHERE id = ?2
            "#,
            params![settings.semantic_provider_type.as_str(), session_id],
        )
        .map_err(|error| format!("更新 Todo 语义边界状态失败: {error}"))?;

    Ok(())
}

pub(crate) fn generate_for_session(
    connection: &Connection,
    settings: &SettingsDto,
    session_id: &str,
) -> Result<usize, String> {
    let (merged_text, trigger_reason, transcript_count): (String, String, i64) = connection
        .query_row(
            "SELECT merged_text, trigger_reason, transcript_count FROM conversation_sessions WHERE id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| format!("读取会话文稿失败: {error}"))?;

    let merged_text = merged_text.trim().to_string();
    if merged_text.is_empty()
        || (transcript_count == 0
            && (trigger_reason == "manual" || is_placeholder_session_text(&merged_text)))
    {
        connection
            .execute(
                "UPDATE conversation_sessions SET extraction_status = 'success', extraction_provider_used = 'skipped', extraction_fallback_used = 0 WHERE id = ?1",
                params![session_id],
            )
            .map_err(|error| format!("更新空会话状态失败: {error}"))?;
        return Ok(0);
    }

    register_semantic_todo_artifact_boundary(connection, settings, session_id, &merged_text)?;
    Ok(0)
}
