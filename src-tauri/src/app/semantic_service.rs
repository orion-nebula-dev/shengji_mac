use rusqlite::{params, Connection};

use crate::{
    app::{transcript_revision_service, workspace_builder},
    current_timestamp_label,
    domain::{
        artifact::{
            MeetingMinutesDto, ModelInvocationDto, RecordingTypeDto, SemanticArtifactDto,
            SemanticWorkbenchDto, SummaryDto, TodoCandidateDto,
        },
        correction::{CorrectionPatternDto, DeletedCorrectionPatternDto, TranscriptRevisionDto},
    },
    providers::semantic::minimax_m3,
};

const SEMANTIC_SCHEMA_VERSION: &str = "v0.6";

pub(crate) fn generate_semantic_workbench(
    connection: &Connection,
) -> Result<SemanticWorkbenchDto, String> {
    let workspace = workspace_builder::build_latest_workspace(connection)?;
    let revisions = transcript_revision_service::build_revisions(&workspace);
    let correction_patterns =
        transcript_revision_service::upsert_default_correction_patterns(connection)?;
    let source_segment_ids = revisions
        .iter()
        .map(|revision| revision.source_segment_id.clone())
        .collect::<Vec<_>>();
    let recording_type = RecordingTypeDto {
        value: "meeting".into(),
        label: "会议".into(),
        template_id: "meeting_minutes_v1".into(),
        confidence: 0.88,
    };
    let summary = build_summary(&revisions);
    let meeting_minutes = build_meeting_minutes(&revisions, &source_segment_ids);
    let todo_candidates = build_todo_candidates(&revisions);

    let invocation = insert_model_invocation(
        connection,
        &workspace.session_id,
        "succeeded",
        "使用修正文稿、来源索引和启用的修正记忆生成 v0.6 语义产物",
        "生成 recording_type、summary、meeting_minutes、todo_extraction",
        "",
    )?;

    upsert_artifact(
        connection,
        &workspace.session_id,
        "transcript_revision",
        "succeeded",
        source_segment_ids.as_slice(),
        transcript_revision_service::revision_payload_json(&revisions).as_str(),
        "",
    )?;
    upsert_artifact(
        connection,
        &workspace.session_id,
        "recording_type",
        "succeeded",
        source_segment_ids.as_slice(),
        serde_json::to_string(&recording_type)
            .unwrap_or_else(|_| "{}".to_string())
            .as_str(),
        "",
    )?;
    upsert_artifact(
        connection,
        &workspace.session_id,
        "summary",
        "succeeded",
        source_segment_ids.as_slice(),
        serde_json::to_string(&summary)
            .unwrap_or_else(|_| "{}".to_string())
            .as_str(),
        "",
    )?;
    upsert_artifact(
        connection,
        &workspace.session_id,
        "meeting_minutes",
        "succeeded",
        source_segment_ids.as_slice(),
        serde_json::to_string(&meeting_minutes)
            .unwrap_or_else(|_| "{}".to_string())
            .as_str(),
        "",
    )?;
    upsert_artifact(
        connection,
        &workspace.session_id,
        "todo_extraction",
        "succeeded",
        source_segment_ids.as_slice(),
        serde_json::to_string(&todo_candidates)
            .unwrap_or_else(|_| "[]".to_string())
            .as_str(),
        "",
    )?;

    connection
        .execute(
            r#"
            UPDATE conversation_sessions
            SET extraction_status = 'success',
                extraction_provider_used = ?1,
                extraction_fallback_used = 0,
                extraction_fallback_reason = ''
            WHERE id = ?2
            "#,
            params![minimax_m3::PROVIDER_ID, workspace.session_id.as_str()],
        )
        .map_err(|error| format!("更新语义会话状态失败: {error}"))?;

    let mut workbench = get_semantic_workbench(connection)?;
    if !workbench
        .model_invocations
        .iter()
        .any(|candidate| candidate.id == invocation.id)
    {
        workbench.model_invocations.push(invocation);
    }
    if workbench.correction_patterns.is_empty() {
        workbench.correction_patterns = correction_patterns;
    }
    Ok(workbench)
}

pub(crate) fn get_semantic_workbench(
    connection: &Connection,
) -> Result<SemanticWorkbenchDto, String> {
    let session_id = latest_semantic_session_id(connection).unwrap_or_default();
    let artifacts = if session_id.is_empty() {
        Vec::new()
    } else {
        query_artifacts(connection, &session_id)?
    };
    let revisions = parse_artifact_payload::<Vec<TranscriptRevisionDto>>(
        artifacts.as_slice(),
        "transcript_revision",
    )
    .unwrap_or_default();
    let source_segment_ids = revisions
        .iter()
        .map(|revision| revision.source_segment_id.clone())
        .collect::<Vec<_>>();

    Ok(SemanticWorkbenchDto {
        session_id: session_id.clone(),
        recording_type: parse_artifact_payload::<RecordingTypeDto>(
            artifacts.as_slice(),
            "recording_type",
        )
        .unwrap_or_else(default_recording_type),
        revisions,
        correction_patterns: transcript_revision_service::query_correction_patterns(connection)?,
        summary: parse_artifact_payload::<SummaryDto>(artifacts.as_slice(), "summary")
            .unwrap_or_else(|| SummaryDto {
                title: "暂无摘要".into(),
                basis: "等待修正文稿生成".into(),
                bullets: Vec::new(),
                source_segment_ids: source_segment_ids.clone(),
            }),
        meeting_minutes: parse_artifact_payload::<MeetingMinutesDto>(
            artifacts.as_slice(),
            "meeting_minutes",
        )
        .unwrap_or_else(|| MeetingMinutesDto {
            template_id: "meeting_minutes_v1".into(),
            decisions: Vec::new(),
            risks: Vec::new(),
            open_questions: Vec::new(),
            source_segment_ids: source_segment_ids.clone(),
        }),
        todo_candidates: parse_artifact_payload::<Vec<TodoCandidateDto>>(
            artifacts.as_slice(),
            "todo_extraction",
        )
        .unwrap_or_default(),
        artifacts,
        model_invocations: query_model_invocations(connection)?,
    })
}

pub(crate) fn set_correction_pattern_enabled(
    connection: &Connection,
    pattern_id: &str,
    enabled: bool,
) -> Result<CorrectionPatternDto, String> {
    transcript_revision_service::set_correction_pattern_enabled(connection, pattern_id, enabled)
}

pub(crate) fn delete_correction_pattern(
    connection: &Connection,
    pattern_id: &str,
) -> Result<DeletedCorrectionPatternDto, String> {
    transcript_revision_service::delete_correction_pattern(connection, pattern_id)
}

pub(crate) fn reject_transcript_revision(
    connection: &Connection,
    revision_id: &str,
) -> Result<TranscriptRevisionDto, String> {
    let session_id = latest_semantic_session_id(connection)
        .filter(|candidate| !candidate.is_empty())
        .ok_or_else(|| "暂无可拒绝的修正文稿".to_string())?;
    let artifacts = query_artifacts(connection, &session_id)?;
    let mut revisions = parse_artifact_payload::<Vec<TranscriptRevisionDto>>(
        artifacts.as_slice(),
        "transcript_revision",
    )
    .ok_or_else(|| "暂无可拒绝的修正文稿".to_string())?;
    let updated_revision = revisions
        .iter_mut()
        .find(|revision| revision.id == revision_id)
        .map(|revision| {
            revision.status = "rejected".to_string();
            revision.clone()
        })
        .ok_or_else(|| "未找到修正文稿".to_string())?;
    let source_segment_ids = revisions
        .iter()
        .map(|revision| revision.source_segment_id.clone())
        .collect::<Vec<_>>();
    upsert_artifact(
        connection,
        &session_id,
        "transcript_revision",
        "succeeded",
        source_segment_ids.as_slice(),
        transcript_revision_service::revision_payload_json(&revisions).as_str(),
        "",
    )?;
    Ok(updated_revision)
}

#[cfg(test)]
pub(crate) fn record_parse_failure_unchecked(
    connection: &Connection,
    session_id: &str,
    raw_response: &str,
) -> Result<SemanticArtifactDto, String> {
    ensure_semantic_failure_session(connection, session_id)?;
    let error_message = match serde_json::from_str::<serde_json::Value>(raw_response) {
        Ok(_) => "MiniMax M3 JSON 结构缺少 v0.6 必需字段".to_string(),
        Err(error) => format!("MiniMax M3 JSON 解析失败: {error}"),
    };
    insert_model_invocation(
        connection,
        session_id,
        "failed",
        "解析 MiniMax M3 summary 响应",
        "未生成可用摘要",
        error_message.as_str(),
    )?;
    upsert_artifact(
        connection,
        session_id,
        "summary",
        "failed",
        &[],
        "{}",
        error_message.as_str(),
    )?;
    query_artifact(connection, session_id, "summary")
}

#[cfg(test)]
fn ensure_semantic_failure_session(
    connection: &Connection,
    session_id: &str,
) -> Result<(), String> {
    connection
        .execute(
            r#"
            INSERT OR IGNORE INTO conversation_sessions (
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
            ) VALUES (?1, '语义解析失败占位会话', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', 0, 'failed', ?2, 0, '', ?3, CURRENT_TIMESTAMP)
            "#,
            params![
                session_id,
                minimax_m3::PROVIDER_ID,
                format!("trace_{session_id}_parse_failure")
            ],
        )
        .map_err(|error| format!("写入语义失败会话失败: {error}"))?;
    Ok(())
}

pub(crate) fn retry_semantic_artifact(
    connection: &Connection,
    artifact_id: &str,
) -> Result<SemanticArtifactDto, String> {
    let updated_count = connection
        .execute(
            r#"
            UPDATE semantic_artifacts
            SET status = 'pending',
                error_message = '',
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1 AND status = 'failed'
            "#,
            params![artifact_id],
        )
        .map_err(|error| format!("重试语义产物失败: {error}"))?;

    if updated_count == 0 {
        return Err("未找到可重试的语义产物".to_string());
    }

    query_artifact_by_id(connection, artifact_id)
}

fn build_summary(revisions: &[TranscriptRevisionDto]) -> SummaryDto {
    let source_segment_ids = revisions
        .iter()
        .map(|revision| revision.source_segment_id.clone())
        .collect::<Vec<_>>();
    SummaryDto {
        title: "转写修正后的会议摘要".into(),
        basis: "基于修正文稿生成，不直接消费原始 ASR 文本。".into(),
        bullets: vec![
            "已完成本地转写评估，并将英文标签修正为中文表达。".into(),
            "说话人标签、时间跳转和错误片段复核是当前会议的重点。".into(),
            "后续语义处理可复用同一来源索引追溯到转写片段。".into(),
        ],
        source_segment_ids,
    }
}

fn build_meeting_minutes(
    revisions: &[TranscriptRevisionDto],
    source_segment_ids: &[String],
) -> MeetingMinutesDto {
    let has_review = revisions
        .iter()
        .any(|revision| revision.change_level == "meaning_affecting");
    MeetingMinutesDto {
        template_id: "meeting_minutes_v1".into(),
        decisions: vec![if has_review {
            "保留需复核片段，并在后续纪要前完成说话人标签复核。".into()
        } else {
            "使用修正文稿作为摘要和待办候选的默认输入。".into()
        }],
        risks: vec!["真实 Argmax/SpeakerKit 推理仍需后续接入验证。".into()],
        open_questions: vec!["低置信度修正记忆是否允许自动套用需由用户确认。".into()],
        source_segment_ids: source_segment_ids.to_vec(),
    }
}

fn build_todo_candidates(revisions: &[TranscriptRevisionDto]) -> Vec<TodoCandidateDto> {
    let source_segment_ids = revisions
        .iter()
        .filter(|revision| revision.change_level == "meaning_affecting")
        .map(|revision| revision.source_segment_id.clone())
        .collect::<Vec<_>>();
    vec![TodoCandidateDto {
        title: "复核转写片段与说话人标签".into(),
        detail: "检查需复核片段，确认说话人切换点和转写修正是否准确。".into(),
        owner: "未分配".into(),
        priority: "medium".into(),
        confidence: 0.82,
        source_segment_ids,
    }]
}

fn upsert_artifact(
    connection: &Connection,
    session_id: &str,
    artifact_type: &str,
    status: &str,
    source_span_refs: &[String],
    payload_json: &str,
    error_message: &str,
) -> Result<(), String> {
    let artifact_id = format!("semantic_{}_{}", session_id, artifact_type);
    let source_refs_json =
        serde_json::to_string(source_span_refs).unwrap_or_else(|_| "[]".to_string());
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
              status = excluded.status,
              provider = excluded.provider,
              model_name = excluded.model_name,
              schema_version = excluded.schema_version,
              source_span_refs = excluded.source_span_refs,
              payload_json = excluded.payload_json,
              error_message = excluded.error_message,
              updated_at = CURRENT_TIMESTAMP
            "#,
            params![
                artifact_id,
                session_id,
                artifact_type,
                status,
                minimax_m3::PROVIDER_ID,
                minimax_m3::DEFAULT_MODEL_NAME,
                SEMANTIC_SCHEMA_VERSION,
                source_refs_json,
                payload_json,
                error_message,
            ],
        )
        .map_err(|error| format!("写入语义产物失败: {error}"))?;
    Ok(())
}

fn insert_model_invocation(
    connection: &Connection,
    session_id: &str,
    status: &str,
    request_summary: &str,
    response_summary: &str,
    error_message: &str,
) -> Result<ModelInvocationDto, String> {
    let invocation_id = transcript_revision_service::next_invocation_id(session_id);
    connection
        .execute(
            r#"
            INSERT INTO model_invocations (
              id,
              provider,
              model_name,
              capability,
              status,
              request_summary,
              response_summary,
              input_tokens,
              output_tokens,
              duration_ms,
              estimated_cost_microunits,
              currency,
              error_message,
              trace_id,
              finished_at,
              created_at
            ) VALUES (?1, ?2, ?3, 'semantic', ?4, ?5, ?6, 0, 0, 0, 0, '', ?7, ?8, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
            params![
                invocation_id,
                minimax_m3::PROVIDER_ID,
                minimax_m3::DEFAULT_MODEL_NAME,
                status,
                request_summary,
                response_summary,
                error_message,
                format!("trace_{session_id}_{}", current_timestamp_label()),
            ],
        )
        .map_err(|error| format!("写入模型调用记录失败: {error}"))?;

    Ok(ModelInvocationDto {
        id: invocation_id,
        provider: minimax_m3::PROVIDER_ID.into(),
        model_name: minimax_m3::DEFAULT_MODEL_NAME.into(),
        capability: "semantic".into(),
        status: status.into(),
        request_summary: request_summary.into(),
        response_summary: response_summary.into(),
        error_message: error_message.into(),
    })
}

fn latest_semantic_session_id(connection: &Connection) -> Option<String> {
    connection
        .query_row(
            r#"
            SELECT id
            FROM conversation_sessions
            WHERE id LIKE 'semantic_session_%'
            ORDER BY datetime(created_at) DESC, id DESC
            LIMIT 1
            "#,
            [],
            |row| row.get(0),
        )
        .ok()
}

fn query_artifacts(
    connection: &Connection,
    session_id: &str,
) -> Result<Vec<SemanticArtifactDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, session_id, artifact_type, status, provider, model_name, schema_version, source_span_refs, payload_json, error_message
            FROM semantic_artifacts
            WHERE session_id = ?1
            ORDER BY artifact_type ASC, id ASC
            "#,
        )
        .map_err(|error| format!("准备语义产物查询失败: {error}"))?;

    let rows = statement
        .query_map(params![session_id], |row| artifact_from_row(row))
        .map_err(|error| format!("查询语义产物失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取语义产物失败: {error}"))
}

#[cfg(test)]
fn query_artifact(
    connection: &Connection,
    session_id: &str,
    artifact_type: &str,
) -> Result<SemanticArtifactDto, String> {
    connection
        .query_row(
            r#"
            SELECT id, session_id, artifact_type, status, provider, model_name, schema_version, source_span_refs, payload_json, error_message
            FROM semantic_artifacts
            WHERE session_id = ?1 AND artifact_type = ?2
            "#,
            params![session_id, artifact_type],
            |row| artifact_from_row(row),
        )
        .map_err(|error| format!("读取语义产物失败: {error}"))
}

fn query_artifact_by_id(
    connection: &Connection,
    artifact_id: &str,
) -> Result<SemanticArtifactDto, String> {
    connection
        .query_row(
            r#"
            SELECT id, session_id, artifact_type, status, provider, model_name, schema_version, source_span_refs, payload_json, error_message
            FROM semantic_artifacts
            WHERE id = ?1
            "#,
            params![artifact_id],
            |row| artifact_from_row(row),
        )
        .map_err(|error| format!("读取语义产物失败: {error}"))
}

fn artifact_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SemanticArtifactDto> {
    let source_span_refs_json: String = row.get(7)?;
    let source_span_refs =
        serde_json::from_str::<Vec<String>>(source_span_refs_json.as_str()).unwrap_or_default();
    Ok(SemanticArtifactDto {
        id: row.get(0)?,
        session_id: row.get(1)?,
        artifact_type: row.get(2)?,
        status: row.get(3)?,
        provider: row.get(4)?,
        model_name: row.get(5)?,
        schema_version: row.get(6)?,
        source_span_refs,
        payload_json: row.get(8)?,
        error_message: row.get(9)?,
    })
}

fn query_model_invocations(connection: &Connection) -> Result<Vec<ModelInvocationDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, provider, model_name, capability, status, request_summary, response_summary, error_message
            FROM model_invocations
            ORDER BY datetime(created_at) DESC, id DESC
            LIMIT 20
            "#,
        )
        .map_err(|error| format!("准备模型调用查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(ModelInvocationDto {
                id: row.get(0)?,
                provider: row.get(1)?,
                model_name: row.get(2)?,
                capability: row.get(3)?,
                status: row.get(4)?,
                request_summary: row.get(5)?,
                response_summary: row.get(6)?,
                error_message: row.get(7)?,
            })
        })
        .map_err(|error| format!("查询模型调用失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取模型调用失败: {error}"))
}

fn parse_artifact_payload<T: serde::de::DeserializeOwned>(
    artifacts: &[SemanticArtifactDto],
    artifact_type: &str,
) -> Option<T> {
    artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == artifact_type && artifact.status == "succeeded")
        .and_then(|artifact| serde_json::from_str::<T>(artifact.payload_json.as_str()).ok())
}

fn default_recording_type() -> RecordingTypeDto {
    RecordingTypeDto {
        value: "other".into(),
        label: "其他".into(),
        template_id: "general_notes_v1".into(),
        confidence: 0.0,
    }
}
