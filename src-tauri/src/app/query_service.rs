use rusqlite::{params, Connection};

use crate::{
    app::settings_service,
    domain::{runtime::RuntimeStatusDto, session::SessionDto, todo::TodoDto},
};

pub(crate) fn query_todos(connection: &Connection) -> Result<Vec<TodoDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
      SELECT
        id,
        title,
        note,
        status,
        created_at,
        conversation_session_id,
        IFNULL(source_text, '')
      FROM todos
      ORDER BY datetime(created_at) DESC, id DESC
      "#,
        )
        .map_err(|error| format!("准备 Todo 查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(TodoDto {
                id: row.get(0)?,
                title: row.get(1)?,
                note: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                conversation_session_id: row.get(5)?,
                source_text: row.get(6)?,
            })
        })
        .map_err(|error| format!("查询 Todo 失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取 Todo 列表失败: {error}"))
}

pub(crate) fn query_sessions(connection: &Connection) -> Result<Vec<SessionDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
      SELECT
        id,
        merged_text,
        started_at,
        ended_at,
        trigger_reason,
        extraction_status,
        extraction_provider_used,
        extraction_fallback_used,
        extraction_fallback_reason,
        transcript_count
      FROM conversation_sessions
      ORDER BY datetime(created_at) DESC, id DESC
      "#,
        )
        .map_err(|error| format!("准备会话查询失败: {error}"))?;

    let session_rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)? == 1,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })
        .map_err(|error| format!("查询会话失败: {error}"))?;

    let mut sessions = Vec::new();
    for session in session_rows {
        let (
            id,
            merged_text,
            started_at,
            ended_at,
            trigger_reason,
            extraction_status,
            extraction_provider_used,
            extraction_fallback_used,
            extraction_fallback_reason,
            transcript_count,
        ) = session.map_err(|error| format!("读取会话行失败: {error}"))?;

        let mut todo_statement = connection
            .prepare(
                r#"
        SELECT id
        FROM todos
        WHERE conversation_session_id = ?1
        ORDER BY datetime(created_at) ASC, id ASC
        "#,
            )
            .map_err(|error| format!("准备会话关联 Todo 查询失败: {error}"))?;

        let todo_rows = todo_statement
            .query_map(params![id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|error| format!("查询会话关联 Todo 失败: {error}"))?;

        let related_todo_ids = todo_rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("读取会话关联 Todo 失败: {error}"))?;

        sessions.push(SessionDto {
            id,
            merged_text,
            started_at,
            ended_at,
            trigger_reason,
            extraction_status,
            extraction_provider_used,
            extraction_fallback_used,
            extraction_fallback_reason,
            transcript_count,
            related_todo_ids,
        });
    }

    Ok(sessions)
}

pub(crate) fn latest_session(connection: &Connection) -> Result<Option<SessionDto>, String> {
    Ok(query_sessions(connection)?.into_iter().next())
}

pub(crate) fn query_runtime_status(connection: &Connection) -> Result<RuntimeStatusDto, String> {
    let settings = settings_service::load_settings(connection)?;
    let last_slice_at: Option<String> = connection
        .query_row(
            "SELECT ended_at FROM audio_segments ORDER BY datetime(created_at) DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let last_session: Option<(String, String)> = connection
    .query_row(
      "SELECT ended_at, extraction_status FROM conversation_sessions ORDER BY datetime(created_at) DESC LIMIT 1",
      [],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .ok();

    Ok(RuntimeStatusDto {
        runtime_label: if settings.record_enabled {
            "录音中".into()
        } else {
            "已暂停".into()
        },
        current_session_status: if settings.record_enabled {
            "collecting".into()
        } else if last_session
            .as_ref()
            .map(|(_, status)| status == "pending")
            .unwrap_or(false)
        {
            "ready_for_extraction".into()
        } else {
            "idle_waiting".into()
        },
        last_slice_at: last_slice_at.unwrap_or_else(|| "暂无切片".into()),
        last_extraction_at: last_session
            .as_ref()
            .map(|value| value.0.clone())
            .unwrap_or_else(|| "暂无".into()),
        last_extraction_summary: if let Some((_, status)) = last_session {
            match status.as_str() {
                "success" => "最近一次会话提取成功".to_string(),
                "failed" => "最近一次会话提取失败，建议重试".to_string(),
                "pending" => "最近一次会话已生成，等待后续提取".to_string(),
                _ => "暂无会话提取记录".to_string(),
            }
        } else {
            "暂无会话提取记录".to_string()
        },
    })
}
