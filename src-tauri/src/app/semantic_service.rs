use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    app::{transcript_revision_service, workspace_builder},
    current_timestamp_label,
    domain::{
        artifact::{
            AddResearchToMindMapCommand, ConvertResearchToTodoCommand, DeepResearchDraftDto,
            GenerateTranslationCommand, MeetingMinutesDto, MindMapDto, MindMapEdgeDto,
            MindMapExportDto, MindMapNodeDto, ModelInvocationDto, MomentDto, RecordingTypeDto,
            SemanticArtifactDto, SemanticWorkbenchDto, StartResearchFromSegmentCommand, SummaryDto,
            SummaryTranslationDto, TodoCandidateDto, ToggleMindMapNodeCommand,
            TranscriptTranslationDto, TranslationArtifactDto, UpdateMindMapNodeCommand,
        },
        correction::{CorrectionPatternDto, DeletedCorrectionPatternDto, TranscriptRevisionDto},
        todo::TodoDto,
    },
    providers::semantic::minimax_m3,
};

const SEMANTIC_SCHEMA_VERSION: &str = "v0.6";
const MIND_MAP_SCHEMA_VERSION: &str = "v0.8";
const VALUE_DISCOVERY_SCHEMA_VERSION: &str = "v0.9";
const TRANSLATION_SCHEMA_VERSION: &str = "v1.1";

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
        translations: parse_all_artifact_payload::<TranslationArtifactDto>(
            artifacts.as_slice(),
            "translation",
        ),
        mind_map: latest_mind_map_payload(connection, &session_id)?,
        moments: parse_artifact_payload::<Vec<MomentDto>>(artifacts.as_slice(), "moment")
            .unwrap_or_default(),
        deep_research: parse_all_artifact_payload::<DeepResearchDraftDto>(
            artifacts.as_slice(),
            "deep_research",
        ),
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

pub(crate) fn generate_translation(
    connection: &Connection,
    command: GenerateTranslationCommand,
) -> Result<SemanticArtifactDto, String> {
    let target_language = normalize_target_language(command.target_language.as_str())?;
    let session_id = latest_semantic_session_id(connection)
        .filter(|candidate| !candidate.is_empty())
        .ok_or_else(|| "暂无可翻译的语义会话，请先生成摘要和转写修正。".to_string())?;
    let artifacts = query_artifacts(connection, &session_id)?;
    let revisions = parse_artifact_payload::<Vec<TranscriptRevisionDto>>(
        artifacts.as_slice(),
        "transcript_revision",
    )
    .unwrap_or_default();
    if revisions.is_empty() {
        return Err("暂无可翻译的转写片段。".into());
    }
    let summary = parse_artifact_payload::<SummaryDto>(artifacts.as_slice(), "summary")
        .unwrap_or_else(|| build_summary(&revisions));
    let translation = build_translation_artifact(target_language.as_str(), &revisions, &summary);
    let payload_json = serde_json::to_string(&translation)
        .map_err(|error| format!("序列化翻译产物失败: {error}"))?;

    insert_model_invocation(
        connection,
        &session_id,
        "succeeded",
        "使用修正文稿与摘要生成 v1.1 翻译产物",
        "生成 translation artifact，包含转写翻译、摘要翻译和来源追溯",
        "",
    )?;

    let artifact_id = format!(
        "semantic_{}_translation_{}",
        session_id,
        sanitize_identifier(target_language.as_str())
    );
    upsert_artifact_with_id(
        connection,
        artifact_id.as_str(),
        &session_id,
        "translation",
        "succeeded",
        translation.source_span_refs.as_slice(),
        payload_json.as_str(),
        "",
        TRANSLATION_SCHEMA_VERSION,
    )?;

    query_artifact_by_id(connection, artifact_id.as_str())
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

pub(crate) fn generate_mind_map(connection: &Connection) -> Result<SemanticArtifactDto, String> {
    let session_id = latest_semantic_session_id(connection)
        .filter(|candidate| !candidate.is_empty())
        .ok_or_else(|| "暂无可生成脑图的语义会话".to_string())?;
    let artifacts = query_artifacts(connection, &session_id)?;
    let revisions = parse_artifact_payload::<Vec<TranscriptRevisionDto>>(
        artifacts.as_slice(),
        "transcript_revision",
    )
    .unwrap_or_default();
    let summary = parse_artifact_payload::<SummaryDto>(artifacts.as_slice(), "summary")
        .unwrap_or_else(|| SummaryDto {
            title: "语义脑图".into(),
            basis: "基于修正文稿和摘要生成。".into(),
            bullets: Vec::new(),
            source_segment_ids: revisions
                .iter()
                .map(|revision| revision.source_segment_id.clone())
                .collect(),
        });
    let meeting_minutes =
        parse_artifact_payload::<MeetingMinutesDto>(artifacts.as_slice(), "meeting_minutes")
            .unwrap_or_else(|| MeetingMinutesDto {
                template_id: "meeting_minutes_v1".into(),
                decisions: Vec::new(),
                risks: Vec::new(),
                open_questions: Vec::new(),
                source_segment_ids: summary.source_segment_ids.clone(),
            });

    let mind_map = build_mind_map(&summary, &meeting_minutes, &revisions, false, 1, "");
    let payload_json =
        serde_json::to_string(&mind_map).map_err(|error| format!("序列化脑图产物失败: {error}"))?;
    let artifact_id = next_mind_map_artifact_id(connection, &session_id, false)?;
    let source_refs = mind_map.source_spans.clone();
    insert_artifact_with_id(
        connection,
        &artifact_id,
        &session_id,
        "mind_map",
        "succeeded",
        source_refs.as_slice(),
        payload_json.as_str(),
        "",
        MIND_MAP_SCHEMA_VERSION,
    )?;
    insert_model_invocation(
        connection,
        &session_id,
        "succeeded",
        "基于修正文稿、摘要和来源索引生成 v0.8 思维脑图",
        "生成 semantic_artifacts(type='mind_map')",
        "",
    )?;
    query_artifact_by_id(connection, &artifact_id)
}

pub(crate) fn update_mind_map_node(
    connection: &Connection,
    command: UpdateMindMapNodeCommand,
) -> Result<SemanticArtifactDto, String> {
    if command.label.trim().is_empty() {
        return Err("脑图节点标题不能为空".to_string());
    }
    let source_artifact = query_artifact_by_id(connection, command.artifact_id.as_str())?;
    let mut mind_map = parse_mind_map_artifact(&source_artifact)?;
    let node = mind_map
        .nodes
        .iter_mut()
        .find(|candidate| candidate.id == command.node_id)
        .ok_or_else(|| "未找到脑图节点".to_string())?;
    node.label = command.label.trim().to_string();
    node.note = command.note.trim().to_string();
    mind_map.edited = true;
    mind_map.version += 1;
    mind_map.parent_artifact_id = source_artifact.id.clone();
    insert_edited_mind_map(connection, &source_artifact, &mind_map)
}

pub(crate) fn toggle_mind_map_node(
    connection: &Connection,
    command: ToggleMindMapNodeCommand,
) -> Result<SemanticArtifactDto, String> {
    let source_artifact = query_artifact_by_id(connection, command.artifact_id.as_str())?;
    let mut mind_map = parse_mind_map_artifact(&source_artifact)?;
    let node = mind_map
        .nodes
        .iter_mut()
        .find(|candidate| candidate.id == command.node_id)
        .ok_or_else(|| "未找到脑图节点".to_string())?;
    node.collapsed = command.collapsed;
    mind_map.edited = true;
    mind_map.version += 1;
    mind_map.parent_artifact_id = source_artifact.id.clone();
    insert_edited_mind_map(connection, &source_artifact, &mind_map)
}

pub(crate) fn export_mind_map(
    connection: &Connection,
    artifact_id: &str,
    format: &str,
) -> Result<MindMapExportDto, String> {
    let artifact = query_artifact_by_id(connection, artifact_id)?;
    let mind_map = parse_mind_map_artifact(&artifact)?;
    match format {
        "markdown" => Ok(MindMapExportDto {
            format: "markdown".into(),
            file_name: format!("{}-mind-map.md", artifact.session_id),
            content: mind_map_to_markdown(&mind_map),
        }),
        "json" => Ok(MindMapExportDto {
            format: "json".into(),
            file_name: format!("{}-mind-map.json", artifact.session_id),
            content: serde_json::to_string_pretty(&mind_map)
                .map_err(|error| format!("导出脑图 JSON 失败: {error}"))?,
        }),
        _ => Err("脑图仅支持 markdown 或 json 导出".to_string()),
    }
}

pub(crate) fn generate_value_discovery(
    connection: &Connection,
) -> Result<SemanticArtifactDto, String> {
    let session_id = latest_semantic_session_id(connection)
        .filter(|candidate| !candidate.is_empty())
        .ok_or_else(|| "暂无可生成价值发现的语义会话".to_string())?;
    let artifacts = query_artifacts(connection, &session_id)?;
    let revisions = parse_artifact_payload::<Vec<TranscriptRevisionDto>>(
        artifacts.as_slice(),
        "transcript_revision",
    )
    .unwrap_or_default();
    if revisions.is_empty() {
        return Err("暂无可用于价值发现的修正文稿".to_string());
    }

    let summary = parse_artifact_payload::<SummaryDto>(artifacts.as_slice(), "summary")
        .unwrap_or_else(|| build_summary(&revisions));
    let meeting_minutes =
        parse_artifact_payload::<MeetingMinutesDto>(artifacts.as_slice(), "meeting_minutes")
            .unwrap_or_else(|| {
                let source_segment_ids = revisions
                    .iter()
                    .map(|revision| revision.source_segment_id.clone())
                    .collect::<Vec<_>>();
                build_meeting_minutes(&revisions, &source_segment_ids)
            });
    let moments = build_moments(&revisions, &meeting_minutes);
    let moment_source_refs = moments
        .iter()
        .flat_map(|moment| moment.source_span_refs.clone())
        .fold(Vec::<String>::new(), |mut refs, source| {
            if !refs.iter().any(|candidate| candidate == &source) {
                refs.push(source);
            }
            refs
        });
    let moment_payload = serde_json::to_string(&moments)
        .map_err(|error| format!("序列化 Moment 产物失败: {error}"))?;
    upsert_artifact_with_schema(
        connection,
        &session_id,
        "moment",
        "succeeded",
        moment_source_refs.as_slice(),
        moment_payload.as_str(),
        "",
        VALUE_DISCOVERY_SCHEMA_VERSION,
    )?;

    let research = build_deep_research_draft(
        &session_id,
        "auto",
        "哪些风险和决策值得继续深入研究？",
        &summary,
        &meeting_minutes,
        &revisions,
    );
    let research_payload =
        serde_json::to_string(&research).map_err(|error| format!("序列化研究草稿失败: {error}"))?;
    upsert_artifact_with_schema(
        connection,
        &session_id,
        "deep_research",
        "succeeded",
        research.source_span_refs.as_slice(),
        research_payload.as_str(),
        "",
        VALUE_DISCOVERY_SCHEMA_VERSION,
    )?;

    insert_model_invocation(
        connection,
        &session_id,
        "succeeded",
        "基于修正文稿、纪要和来源索引生成 v0.9 Moment 与 Deep Research 草稿",
        "生成 semantic_artifacts(type='moment' | 'deep_research')",
        "",
    )?;

    query_artifact(connection, &session_id, "moment")
}

pub(crate) fn start_research_from_segment(
    connection: &Connection,
    command: StartResearchFromSegmentCommand,
) -> Result<SemanticArtifactDto, String> {
    let segment_id = command.segment_id.trim();
    if segment_id.is_empty() {
        return Err("请选择要研究的转写片段".to_string());
    }
    let session_id = latest_semantic_session_id(connection)
        .filter(|candidate| !candidate.is_empty())
        .ok_or_else(|| "暂无可发起研究的语义会话".to_string())?;
    let artifacts = query_artifacts(connection, &session_id)?;
    let revisions = parse_artifact_payload::<Vec<TranscriptRevisionDto>>(
        artifacts.as_slice(),
        "transcript_revision",
    )
    .unwrap_or_default();
    let source_revision = revisions
        .iter()
        .find(|revision| revision.source_segment_id == segment_id)
        .ok_or_else(|| "未找到对应转写片段".to_string())?;
    let summary = parse_artifact_payload::<SummaryDto>(artifacts.as_slice(), "summary")
        .unwrap_or_else(|| build_summary(&revisions));
    let source_segment_ids = revisions
        .iter()
        .map(|revision| revision.source_segment_id.clone())
        .collect::<Vec<_>>();
    let meeting_minutes =
        parse_artifact_payload::<MeetingMinutesDto>(artifacts.as_slice(), "meeting_minutes")
            .unwrap_or_else(|| build_meeting_minutes(&revisions, &source_segment_ids));
    let question = if command.question.trim().is_empty() {
        format!("片段“{}”是否需要继续研究？", source_revision.revised_text)
    } else {
        command.question.trim().to_string()
    };
    let mut research = build_deep_research_draft(
        &session_id,
        segment_id,
        question.as_str(),
        &summary,
        &meeting_minutes,
        std::slice::from_ref(source_revision),
    );
    research.source_span_refs = vec![source_revision.source_segment_id.clone()];
    research.background = format!(
        "来自 {} {}-{}ms：{}",
        source_revision.speaker_label,
        source_revision.start_ms,
        source_revision.end_ms,
        source_revision.revised_text
    );
    let payload_json = serde_json::to_string(&research)
        .map_err(|error| format!("序列化片段研究草稿失败: {error}"))?;
    let artifact_id = next_research_artifact_id(connection, &session_id)?;
    insert_artifact_with_id(
        connection,
        &artifact_id,
        &session_id,
        "deep_research",
        "succeeded",
        research.source_span_refs.as_slice(),
        payload_json.as_str(),
        "",
        VALUE_DISCOVERY_SCHEMA_VERSION,
    )?;
    insert_model_invocation(
        connection,
        &session_id,
        "succeeded",
        "从单个转写片段发起 v0.9 Deep Research 草稿",
        "生成 semantic_artifacts(type='deep_research')",
        "",
    )?;
    query_artifact_by_id(connection, &artifact_id)
}

pub(crate) fn convert_research_to_todo(
    connection: &Connection,
    command: ConvertResearchToTodoCommand,
) -> Result<TodoDto, String> {
    let artifact = query_artifact_by_id(connection, command.artifact_id.as_str())?;
    let research = parse_research_from_artifact(&artifact, command.research_id.as_str())?;
    let dedup_key = format!("research_todo_{}_{}", artifact.session_id, research.id);
    if let Some(existing) = existing_todo_by_dedup_key(connection, dedup_key.as_str())? {
        return Ok(existing);
    }

    let todo_id = format!("todo_v09_{}", current_timestamp_label());
    let source_refs_json =
        serde_json::to_string(&research.source_span_refs).unwrap_or_else(|_| "[]".to_string());
    connection
        .execute(
            r#"
            INSERT INTO todos (
              id,
              conversation_session_id,
              title,
              note,
              status,
              created_at,
              source_text,
              extraction_model_name,
              trace_id,
              updated_at,
              owner,
              due_at,
              priority,
              source_span_refs,
              candidate_id,
              dedup_key
            ) VALUES (?1, ?2, ?3, ?4, 'open', CURRENT_TIMESTAMP, ?5, ?6, ?7, CURRENT_TIMESTAMP, '', '', 'medium', ?8, ?9, ?10)
            "#,
            params![
                todo_id,
                artifact.session_id,
                format!("研究：{}", research.question),
                research.next_steps.join("；"),
                research.background,
                minimax_m3::DEFAULT_MODEL_NAME,
                format!("trace_{}_research_todo", research.id),
                source_refs_json,
                research.id,
                dedup_key,
            ],
        )
        .map_err(|error| format!("研究草稿转 Todo 失败: {error}"))?;

    let mut updated_research = research;
    updated_research.converted_todo_id = todo_id.clone();
    update_research_artifact_payload(connection, &artifact, &updated_research)?;
    query_todo_by_id(connection, todo_id.as_str())
}

pub(crate) fn add_research_to_mind_map(
    connection: &Connection,
    command: AddResearchToMindMapCommand,
) -> Result<SemanticArtifactDto, String> {
    let research_artifact = query_artifact_by_id(connection, command.artifact_id.as_str())?;
    let mut research =
        parse_research_from_artifact(&research_artifact, command.research_id.as_str())?;
    let latest_mind_map = latest_mind_map_artifact(connection, &research_artifact.session_id)?
        .ok_or_else(|| "暂无可追加研究节点的脑图".to_string())?;
    let mut mind_map = parse_mind_map_artifact(&latest_mind_map)?;
    let node_id = format!("research_{}", sanitize_identifier(research.id.as_str()));
    if !mind_map.nodes.iter().any(|node| node.id == node_id) {
        mind_map.nodes.push(MindMapNodeDto {
            id: node_id.clone(),
            label: format!("研究：{}", research.question),
            kind: "research".into(),
            note: research.next_steps.join("；"),
            source_span_refs: research.source_span_refs.clone(),
            collapsed: false,
        });
        mind_map.edges.push(MindMapEdgeDto {
            id: format!("edge_root_{node_id}"),
            from: mind_map.root.clone(),
            to: node_id.clone(),
            label: "研究".into(),
        });
    }
    for source in &research.source_span_refs {
        if !mind_map
            .source_spans
            .iter()
            .any(|candidate| candidate == source)
        {
            mind_map.source_spans.push(source.clone());
        }
    }
    mind_map.edited = true;
    mind_map.version += 1;
    mind_map.parent_artifact_id = latest_mind_map.id.clone();
    research.mind_map_node_id = node_id;
    update_research_artifact_payload(connection, &research_artifact, &research)?;
    insert_edited_mind_map(connection, &latest_mind_map, &mind_map)
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

fn build_translation_artifact(
    target_language: &str,
    revisions: &[TranscriptRevisionDto],
    summary: &SummaryDto,
) -> TranslationArtifactDto {
    let transcript_translations = revisions
        .iter()
        .map(|revision| TranscriptTranslationDto {
            source_segment_id: revision.source_segment_id.clone(),
            speaker_label: revision.speaker_label.clone(),
            start_ms: revision.start_ms,
            end_ms: revision.end_ms,
            original_text: revision.revised_text.clone(),
            translated_text: deterministic_translate(
                target_language,
                revision.revised_text.as_str(),
            ),
        })
        .collect::<Vec<_>>();
    let source_span_refs = revisions
        .iter()
        .map(|revision| revision.source_segment_id.clone())
        .collect::<Vec<_>>();

    TranslationArtifactDto {
        target_language: target_language.into(),
        transcript_translations,
        summary_translation: SummaryTranslationDto {
            source_artifact_type: "summary".into(),
            original_title: summary.title.clone(),
            translated_title: deterministic_translate(target_language, summary.title.as_str()),
            original_basis: summary.basis.clone(),
            translated_basis: deterministic_translate(target_language, summary.basis.as_str()),
            translated_bullets: summary
                .bullets
                .iter()
                .map(|bullet| deterministic_translate(target_language, bullet.as_str()))
                .collect(),
        },
        source_span_refs,
    }
}

fn deterministic_translate(target_language: &str, text: &str) -> String {
    format!("[{target_language}] {text}")
}

fn normalize_target_language(target_language: &str) -> Result<String, String> {
    let normalized = target_language.trim();
    if normalized.is_empty() {
        return Err("目标语言不能为空。".into());
    }
    let safe = normalized
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .collect::<String>();
    if safe.is_empty() {
        Err("目标语言格式无效。".into())
    } else {
        Ok(safe)
    }
}

fn build_moments(
    revisions: &[TranscriptRevisionDto],
    meeting_minutes: &MeetingMinutesDto,
) -> Vec<MomentDto> {
    let fallback = revisions.first();
    let mut moments = Vec::new();
    let patterns = [
        (
            "key_point",
            "关键观点",
            "修正文稿已经形成稳定语义输入，可继续承载摘要、Todo 和脑图。",
            "可作为后续价值发现的主线素材。",
            0.86,
        ),
        (
            "decision",
            "决策点",
            meeting_minutes
                .decisions
                .first()
                .map(String::as_str)
                .unwrap_or("使用修正文稿作为后续语义产物的默认输入。"),
            "可转为执行检查项。",
            0.9,
        ),
        (
            "risk",
            "风险信号",
            meeting_minutes
                .risks
                .first()
                .map(String::as_str)
                .unwrap_or("真实模型推理和来源追溯仍需要验证。"),
            "建议进入 Deep Research 草稿继续拆解。",
            0.88,
        ),
        (
            "disagreement",
            "分歧点",
            meeting_minutes
                .open_questions
                .first()
                .map(String::as_str)
                .unwrap_or("低置信度修正是否自动套用仍待确认。"),
            "需要补充判断标准并明确责任人。",
            0.8,
        ),
        (
            "highlight",
            "高价值片段",
            "说话人标签、时间跳转和错误片段复核可形成可复用工作流。",
            "可沉淀为脑图节点或版本验收项。",
            0.84,
        ),
    ];

    for (index, (moment_type, title, summary, action_hint, importance)) in
        patterns.into_iter().enumerate()
    {
        let revision = revisions.get(index).or(fallback);
        let (start_ms, end_ms, source_span_refs) = revision
            .map(|revision| {
                (
                    revision.start_ms,
                    revision.end_ms,
                    vec![revision.source_segment_id.clone()],
                )
            })
            .unwrap_or((0, 0, meeting_minutes.source_segment_ids.clone()));
        moments.push(MomentDto {
            id: format!("moment_{}", index + 1),
            title: title.into(),
            moment_type: moment_type.into(),
            summary: summary.into(),
            importance,
            start_ms,
            end_ms,
            source_span_refs,
            action_hint: action_hint.into(),
        });
    }

    moments
}

fn build_deep_research_draft(
    session_id: &str,
    source_key: &str,
    question: &str,
    summary: &SummaryDto,
    meeting_minutes: &MeetingMinutesDto,
    revisions: &[TranscriptRevisionDto],
) -> DeepResearchDraftDto {
    let mut source_span_refs = revisions
        .iter()
        .map(|revision| revision.source_segment_id.clone())
        .collect::<Vec<_>>();
    if source_span_refs.is_empty() {
        source_span_refs = summary.source_segment_ids.clone();
    }
    if source_span_refs.is_empty() {
        source_span_refs = meeting_minutes.source_segment_ids.clone();
    }
    DeepResearchDraftDto {
        id: format!(
            "research_{}_{}",
            sanitize_identifier(session_id),
            sanitize_identifier(source_key)
        ),
        question: question.into(),
        background: format!(
            "{}；{}",
            summary.basis,
            summary
                .bullets
                .first()
                .map(String::as_str)
                .unwrap_or("暂无额外摘要背景")
        ),
        hypotheses: vec![
            "风险来自真实模型接入、转写质量和来源追溯之间的耦合。".into(),
            "如果先固化片段来源和验收证据，可以降低后续产品化返工。".into(),
        ],
        search_directions: vec![
            "复查同会话中的低置信度修正与失败任务日志。".into(),
            "对照 MiniMax M3 语义产物的来源片段覆盖率。".into(),
            "整理可转为 Todo 或脑图节点的执行项。".into(),
        ],
        next_steps: vec![
            "确认最高风险片段的责任人与复核口径。".into(),
            "把可执行研究结论转为 Todo 并绑定来源片段。".into(),
        ],
        source_span_refs,
        converted_todo_id: String::new(),
        mind_map_node_id: String::new(),
    }
}

fn build_mind_map(
    summary: &SummaryDto,
    meeting_minutes: &MeetingMinutesDto,
    revisions: &[TranscriptRevisionDto],
    edited: bool,
    version: i64,
    parent_artifact_id: &str,
) -> MindMapDto {
    let mut source_spans = summary.source_segment_ids.clone();
    for revision in revisions {
        if !source_spans
            .iter()
            .any(|source| source == &revision.source_segment_id)
        {
            source_spans.push(revision.source_segment_id.clone());
        }
    }
    if source_spans.is_empty() {
        source_spans = meeting_minutes.source_segment_ids.clone();
    }

    let mut nodes = vec![
        MindMapNodeDto {
            id: "root".into(),
            label: summary.title.clone(),
            kind: "root".into(),
            note: summary.basis.clone(),
            source_span_refs: source_spans.clone(),
            collapsed: false,
        },
        MindMapNodeDto {
            id: "summary".into(),
            label: "核心摘要".into(),
            kind: "summary".into(),
            note: summary.bullets.join("；"),
            source_span_refs: summary.source_segment_ids.clone(),
            collapsed: false,
        },
    ];

    for (index, decision) in meeting_minutes.decisions.iter().enumerate() {
        nodes.push(MindMapNodeDto {
            id: format!("decision_{}", index + 1),
            label: decision.clone(),
            kind: "decision".into(),
            note: "从类型化纪要决策项生成。".into(),
            source_span_refs: meeting_minutes.source_segment_ids.clone(),
            collapsed: false,
        });
    }

    for revision in revisions
        .iter()
        .filter(|revision| revision.change_level != "none")
    {
        nodes.push(MindMapNodeDto {
            id: format!("source_{}", revision.source_segment_id),
            label: revision.revised_text.clone(),
            kind: "source".into(),
            note: revision.reason_summary.clone(),
            source_span_refs: vec![revision.source_segment_id.clone()],
            collapsed: false,
        });
    }

    let mut edges = vec![MindMapEdgeDto {
        id: "edge_root_summary".into(),
        from: "root".into(),
        to: "summary".into(),
        label: "概括".into(),
    }];
    for node in nodes
        .iter()
        .filter(|node| node.id != "root" && node.id != "summary")
    {
        edges.push(MindMapEdgeDto {
            id: format!("edge_root_{}", node.id),
            from: "root".into(),
            to: node.id.clone(),
            label: match node.kind.as_str() {
                "decision" => "决策".into(),
                "source" => "来源".into(),
                _ => "关联".into(),
            },
        });
    }

    MindMapDto {
        root: "root".into(),
        nodes,
        edges,
        summary: format!("基于修正文稿与摘要生成：{}", summary.basis),
        source_spans,
        edited,
        version,
        parent_artifact_id: parent_artifact_id.into(),
    }
}

fn parse_mind_map_artifact(artifact: &SemanticArtifactDto) -> Result<MindMapDto, String> {
    if artifact.artifact_type != "mind_map" {
        return Err("该语义产物不是思维脑图".to_string());
    }
    serde_json::from_str::<MindMapDto>(artifact.payload_json.as_str())
        .map_err(|error| format!("解析脑图产物失败: {error}"))
}

fn insert_edited_mind_map(
    connection: &Connection,
    source_artifact: &SemanticArtifactDto,
    mind_map: &MindMapDto,
) -> Result<SemanticArtifactDto, String> {
    let artifact_id = next_mind_map_artifact_id(connection, &source_artifact.session_id, true)?;
    let payload_json =
        serde_json::to_string(mind_map).map_err(|error| format!("序列化脑图编辑失败: {error}"))?;
    insert_artifact_with_id(
        connection,
        &artifact_id,
        &source_artifact.session_id,
        "mind_map",
        "succeeded",
        mind_map.source_spans.as_slice(),
        payload_json.as_str(),
        "",
        MIND_MAP_SCHEMA_VERSION,
    )?;
    query_artifact_by_id(connection, &artifact_id)
}

fn mind_map_to_markdown(mind_map: &MindMapDto) -> String {
    let mut lines = vec![
        "# 语义脑图".to_string(),
        String::new(),
        format!("- 摘要：{}", mind_map.summary),
        format!("- 版本：{}", mind_map.version),
        format!(
            "- 来源：{}",
            if mind_map.source_spans.is_empty() {
                "暂无来源".to_string()
            } else {
                mind_map.source_spans.join("、")
            }
        ),
        String::new(),
    ];
    for node in &mind_map.nodes {
        let marker = if node.id == mind_map.root {
            "##"
        } else {
            "###"
        };
        lines.push(format!("{marker} {}", node.label));
        if !node.note.trim().is_empty() {
            lines.push(node.note.clone());
        }
        if !node.source_span_refs.is_empty() {
            lines.push(format!("来源：{}", node.source_span_refs.join("、")));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

fn next_mind_map_artifact_id(
    connection: &Connection,
    session_id: &str,
    edited: bool,
) -> Result<String, String> {
    let count = connection
        .query_row(
            "SELECT COUNT(1) FROM semantic_artifacts WHERE session_id = ?1 AND artifact_type = 'mind_map'",
            params![session_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("计算脑图版本失败: {error}"))?;
    let suffix = if edited { "edited" } else { "generated" };
    Ok(format!(
        "semantic_{}_mind_map_{}_v{}",
        session_id,
        suffix,
        count + 1
    ))
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

fn upsert_artifact_with_schema(
    connection: &Connection,
    session_id: &str,
    artifact_type: &str,
    status: &str,
    source_span_refs: &[String],
    payload_json: &str,
    error_message: &str,
    schema_version: &str,
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
                schema_version,
                source_refs_json,
                payload_json,
                error_message,
            ],
        )
        .map_err(|error| format!("写入语义产物失败: {error}"))?;
    Ok(())
}

fn insert_artifact_with_id(
    connection: &Connection,
    artifact_id: &str,
    session_id: &str,
    artifact_type: &str,
    status: &str,
    source_span_refs: &[String],
    payload_json: &str,
    error_message: &str,
    schema_version: &str,
) -> Result<(), String> {
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
            "#,
            params![
                artifact_id,
                session_id,
                artifact_type,
                status,
                minimax_m3::PROVIDER_ID,
                minimax_m3::DEFAULT_MODEL_NAME,
                schema_version,
                source_refs_json,
                payload_json,
                error_message,
            ],
        )
        .map_err(|error| format!("写入语义产物版本失败: {error}"))?;
    Ok(())
}

fn upsert_artifact_with_id(
    connection: &Connection,
    artifact_id: &str,
    session_id: &str,
    artifact_type: &str,
    status: &str,
    source_span_refs: &[String],
    payload_json: &str,
    error_message: &str,
    schema_version: &str,
) -> Result<(), String> {
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
                schema_version,
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
    let invocation_id = next_model_invocation_id(connection, session_id)?;
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

fn next_model_invocation_id(connection: &Connection, session_id: &str) -> Result<String, String> {
    let count = connection
        .query_row(
            "SELECT COUNT(1) FROM model_invocations WHERE id LIKE ?1",
            params![format!("model_invocation_{}_%", session_id)],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("计算模型调用序号失败: {error}"))?;
    Ok(format!("model_invocation_{}_v{}", session_id, count + 1))
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

fn parse_all_artifact_payload<T: serde::de::DeserializeOwned>(
    artifacts: &[SemanticArtifactDto],
    artifact_type: &str,
) -> Vec<T> {
    artifacts
        .iter()
        .filter(|artifact| {
            artifact.artifact_type == artifact_type && artifact.status == "succeeded"
        })
        .filter_map(|artifact| serde_json::from_str::<T>(artifact.payload_json.as_str()).ok())
        .collect()
}

fn latest_mind_map_payload(
    connection: &Connection,
    session_id: &str,
) -> Result<Option<MindMapDto>, String> {
    if session_id.is_empty() {
        return Ok(None);
    }
    let artifact = connection
        .query_row(
            r#"
            SELECT id, session_id, artifact_type, status, provider, model_name, schema_version, source_span_refs, payload_json, error_message
            FROM semantic_artifacts
            WHERE session_id = ?1
              AND artifact_type = 'mind_map'
              AND status = 'succeeded'
            ORDER BY datetime(created_at) DESC, id DESC
            LIMIT 1
            "#,
            params![session_id],
            |row| artifact_from_row(row),
        )
        .optional()
        .map_err(|error| format!("查询最新脑图失败: {error}"))?;

    match artifact {
        Some(artifact) => serde_json::from_str::<MindMapDto>(artifact.payload_json.as_str())
            .map(Some)
            .map_err(|error| format!("读取最新脑图失败: {error}")),
        None => Ok(None),
    }
}

fn latest_mind_map_artifact(
    connection: &Connection,
    session_id: &str,
) -> Result<Option<SemanticArtifactDto>, String> {
    connection
        .query_row(
            r#"
            SELECT id, session_id, artifact_type, status, provider, model_name, schema_version, source_span_refs, payload_json, error_message
            FROM semantic_artifacts
            WHERE session_id = ?1
              AND artifact_type = 'mind_map'
              AND status = 'succeeded'
            ORDER BY datetime(created_at) DESC, id DESC
            LIMIT 1
            "#,
            params![session_id],
            |row| artifact_from_row(row),
        )
        .optional()
        .map_err(|error| format!("查询最新脑图失败: {error}"))
}

fn next_research_artifact_id(connection: &Connection, session_id: &str) -> Result<String, String> {
    let count = connection
        .query_row(
            "SELECT COUNT(1) FROM semantic_artifacts WHERE session_id = ?1 AND artifact_type = 'deep_research'",
            params![session_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("计算研究草稿版本失败: {error}"))?;
    Ok(format!(
        "semantic_{}_deep_research_v{}",
        session_id,
        count + 1
    ))
}

fn parse_research_from_artifact(
    artifact: &SemanticArtifactDto,
    research_id: &str,
) -> Result<DeepResearchDraftDto, String> {
    if artifact.artifact_type != "deep_research" {
        return Err("该语义产物不是研究草稿".to_string());
    }
    let research = serde_json::from_str::<DeepResearchDraftDto>(artifact.payload_json.as_str())
        .map_err(|error| format!("解析研究草稿失败: {error}"))?;
    if research.id != research_id {
        return Err("研究草稿 ID 与产物不匹配".to_string());
    }
    Ok(research)
}

fn update_research_artifact_payload(
    connection: &Connection,
    artifact: &SemanticArtifactDto,
    research: &DeepResearchDraftDto,
) -> Result<(), String> {
    let payload_json = serde_json::to_string(research)
        .map_err(|error| format!("序列化研究草稿状态失败: {error}"))?;
    let source_refs_json =
        serde_json::to_string(&research.source_span_refs).unwrap_or_else(|_| "[]".to_string());
    connection
        .execute(
            r#"
            UPDATE semantic_artifacts
            SET payload_json = ?1,
                source_span_refs = ?2,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?3
            "#,
            params![payload_json, source_refs_json, artifact.id],
        )
        .map_err(|error| format!("更新研究草稿状态失败: {error}"))?;
    Ok(())
}

fn existing_todo_by_dedup_key(
    connection: &Connection,
    dedup_key: &str,
) -> Result<Option<TodoDto>, String> {
    let todo_id = connection
        .query_row(
            "SELECT id FROM todos WHERE dedup_key = ?1 LIMIT 1",
            params![dedup_key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("查询研究 Todo 去重失败: {error}"))?;
    match todo_id {
        Some(todo_id) => query_todo_by_id(connection, todo_id.as_str()).map(Some),
        None => Ok(None),
    }
}

fn query_todo_by_id(connection: &Connection, todo_id: &str) -> Result<TodoDto, String> {
    connection
        .query_row(
            r#"
            SELECT id, title, note, status, created_at, conversation_session_id, source_text, owner, due_at, priority, source_span_refs, candidate_id
            FROM todos
            WHERE id = ?1
            "#,
            params![todo_id],
            |row| {
                let source_refs_json: String = row.get(10)?;
                Ok(TodoDto {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    note: row.get(2)?,
                    status: row.get(3)?,
                    created_at: row.get(4)?,
                    conversation_session_id: row.get(5)?,
                    source_text: row.get(6)?,
                    owner: row.get(7)?,
                    due_at: row.get(8)?,
                    priority: row.get(9)?,
                    source_span_refs: serde_json::from_str::<Vec<String>>(
                        source_refs_json.as_str(),
                    )
                    .unwrap_or_default(),
                    candidate_id: row.get(11)?,
                })
            },
        )
        .map_err(|error| format!("读取研究 Todo 失败: {error}"))
}

fn sanitize_identifier(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    sanitized.trim_matches('_').to_string()
}

fn default_recording_type() -> RecordingTypeDto {
    RecordingTypeDto {
        value: "other".into(),
        label: "其他".into(),
        template_id: "general_notes_v1".into(),
        confidence: 0.0,
    }
}
