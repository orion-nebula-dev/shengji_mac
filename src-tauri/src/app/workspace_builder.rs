use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceSegment {
    pub(crate) id: String,
    pub(crate) speaker_label: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) text: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SemanticWorkspace {
    pub(crate) session_id: String,
    pub(crate) segments: Vec<WorkspaceSegment>,
}

pub(crate) fn build_latest_workspace(
    connection: &Connection,
) -> Result<SemanticWorkspace, String> {
    let audio_segment_id: String = connection
        .query_row(
            r#"
            SELECT id
            FROM audio_segments
            ORDER BY datetime(created_at) DESC, id DESC
            LIMIT 1
            "#,
            [],
            |row| row.get(0),
        )
        .map_err(|_| "暂无可用于语义处理的转写音频".to_string())?;

    let session_id = ensure_workspace_session(connection, &audio_segment_id)?;
    let segments = query_workspace_segments(connection, &audio_segment_id, &session_id)?;

    if segments.is_empty() {
        return Err("暂无可用于语义处理的转写片段".to_string());
    }

    Ok(SemanticWorkspace {
        session_id,
        segments,
    })
}

fn ensure_workspace_session(
    connection: &Connection,
    audio_segment_id: &str,
) -> Result<String, String> {
    let session_id = format!("semantic_session_{audio_segment_id}");
    let transcript_count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM transcript_segments WHERE audio_segment_id = ?1",
            params![audio_segment_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("统计转写片段失败: {error}"))?;

    let merged_text: String = connection
        .query_row(
            r#"
            SELECT IFNULL(group_concat(text, char(10)), '')
            FROM (
              SELECT text
              FROM transcript_segments
              WHERE audio_segment_id = ?1
              ORDER BY start_ms ASC, id ASC
            )
            "#,
            params![audio_segment_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("构造语义工作区失败: {error}"))?;

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
            ) VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', ?3, 'pending', 'minimax_m3', 0, '', ?4, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
              merged_text = excluded.merged_text,
              transcript_count = excluded.transcript_count,
              extraction_provider_used = excluded.extraction_provider_used
            "#,
            params![
                session_id.as_str(),
                merged_text,
                transcript_count,
                format!("trace_{session_id}")
            ],
        )
        .map_err(|error| format!("写入语义工作区会话失败: {error}"))?;

    connection
        .execute(
            r#"
            UPDATE transcript_segments
            SET conversation_session_id = ?1
            WHERE audio_segment_id = ?2
            "#,
            params![session_id.as_str(), audio_segment_id],
        )
        .map_err(|error| format!("关联转写片段到语义会话失败: {error}"))?;

    Ok(session_id)
}

fn query_workspace_segments(
    connection: &Connection,
    audio_segment_id: &str,
    session_id: &str,
) -> Result<Vec<WorkspaceSegment>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT
              transcript_segments.id,
              IFNULL(speakers.label, transcript_segments.speaker_id),
              transcript_segments.start_ms,
              transcript_segments.end_ms,
              transcript_segments.text
            FROM transcript_segments
            LEFT JOIN speakers ON speakers.id = transcript_segments.speaker_id
            WHERE transcript_segments.audio_segment_id = ?1
              AND transcript_segments.conversation_session_id = ?2
            ORDER BY transcript_segments.start_ms ASC, transcript_segments.id ASC
            "#,
        )
        .map_err(|error| format!("准备语义工作区片段查询失败: {error}"))?;

    let rows = statement
        .query_map(params![audio_segment_id, session_id], |row| {
            Ok(WorkspaceSegment {
                id: row.get(0)?,
                speaker_label: row.get(1)?,
                start_ms: row.get(2)?,
                end_ms: row.get(3)?,
                text: row.get(4)?,
            })
        })
        .map_err(|error| format!("查询语义工作区片段失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取语义工作区片段失败: {error}"))
}
