use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::{
    current_timestamp_label,
    domain::export::{
        ExportBundleDto, ExportItemDto, GenerateExportBundleCommand, ShareSnapshotDto,
    },
    providers::export::local_file,
};

#[derive(Debug, Clone)]
struct ExportTranscriptSegment {
    id: String,
    speaker_label: String,
    start_ms: i64,
    end_ms: i64,
    text: String,
}

#[derive(Debug, Clone)]
struct ExportArtifact {
    id: String,
    artifact_type: String,
    status: String,
    provider: String,
    model_name: String,
    source_span_refs: Vec<String>,
    payload_json: String,
    error_message: String,
}

#[derive(Debug, Clone)]
struct ExportTodo {
    id: String,
    title: String,
    note: String,
    status: String,
    owner: String,
    due_at: String,
    priority: String,
    source_span_refs: Vec<String>,
}

pub(crate) fn generate_export_bundle(
    connection: &Connection,
    command: GenerateExportBundleCommand,
) -> Result<ExportBundleDto, String> {
    let session_id = latest_exportable_session_id(connection)?
        .ok_or_else(|| "暂无可导出的会话，请先完成转写与语义生成".to_string())?;
    let transcript_segments = query_transcript_segments(connection, session_id.as_str())?;
    let artifacts = query_artifacts(connection, session_id.as_str())?;
    let todos = query_todos(connection, session_id.as_str())?;
    let source_span_refs = collect_source_span_refs(&artifacts, &todos);

    let bundle_id = format!("export_bundle_{}_{}", session_id, current_timestamp_label());
    let formats = normalize_formats(command.formats);
    let target_languages = normalize_target_languages(command.target_languages);
    let mut items = Vec::new();
    let mut snapshot = None;

    if target_languages.is_empty() {
        for format in formats {
            match format.as_str() {
                "markdown" => {
                    let content = render_markdown(
                        session_id.as_str(),
                        &transcript_segments,
                        &artifacts,
                        &todos,
                    );
                    items.push(build_item(
                        &bundle_id,
                        session_id.as_str(),
                        "markdown",
                        "声记会话导出.md",
                        "text/markdown; charset=utf-8",
                        content,
                        source_span_refs.clone(),
                    ));
                }
                "srt" => {
                    let content = render_srt(&transcript_segments);
                    items.push(build_item(
                        &bundle_id,
                        session_id.as_str(),
                        "srt",
                        "声记字幕导出.srt",
                        "application/x-subrip; charset=utf-8",
                        content,
                        transcript_segments
                            .iter()
                            .map(|segment| segment.id.clone())
                            .collect(),
                    ));
                }
                "json" => {
                    let content = render_json(
                        session_id.as_str(),
                        &transcript_segments,
                        &artifacts,
                        &todos,
                    )?;
                    items.push(build_item(
                        &bundle_id,
                        session_id.as_str(),
                        "json",
                        "声记结构化导出.json",
                        "application/json; charset=utf-8",
                        content,
                        source_span_refs.clone(),
                    ));
                }
                "snapshot" => {
                    let snapshot_dto = ShareSnapshotDto {
                        id: format!("{}_snapshot", bundle_id),
                        file_name: "声记分享快照.html".into(),
                        title: "声记分享快照".into(),
                        html: render_snapshot_html(
                            session_id.as_str(),
                            &transcript_segments,
                            &artifacts,
                            &todos,
                        ),
                        source_span_refs: source_span_refs.clone(),
                        privacy_summary: privacy_summary(),
                    };
                    items.push(build_item(
                        &bundle_id,
                        session_id.as_str(),
                        "snapshot",
                        snapshot_dto.file_name.as_str(),
                        "text/html; charset=utf-8",
                        snapshot_dto.html.clone(),
                        source_span_refs.clone(),
                    ));
                    snapshot = Some(snapshot_dto);
                }
                _ => {
                    return Err(format!("暂不支持的导出格式: {format}"));
                }
            }
        }
    } else {
        for target_language in target_languages {
            let translation =
                translation_payload_for_language(&artifacts, target_language.as_str())
                    .ok_or_else(|| format!("缺少 {target_language} 翻译产物，请先生成翻译。"))?;
            let language_source_refs = translation_source_span_refs(&translation);
            for format in &formats {
                match format.as_str() {
                    "markdown" => {
                        items.push(build_item(
                            &bundle_id,
                            session_id.as_str(),
                            format!("markdown_{target_language}").as_str(),
                            format!("声记多语言导出-{target_language}.md").as_str(),
                            "text/markdown; charset=utf-8",
                            render_multilingual_markdown(
                                session_id.as_str(),
                                target_language.as_str(),
                                &translation,
                            ),
                            language_source_refs.clone(),
                        ));
                    }
                    "srt" => {
                        items.push(build_item(
                            &bundle_id,
                            session_id.as_str(),
                            format!("srt_{target_language}").as_str(),
                            format!("声记多语言字幕-{target_language}.srt").as_str(),
                            "application/x-subrip; charset=utf-8",
                            render_multilingual_srt(&translation),
                            language_source_refs.clone(),
                        ));
                    }
                    "json" => {
                        items.push(build_item(
                            &bundle_id,
                            session_id.as_str(),
                            format!("json_{target_language}").as_str(),
                            format!("声记多语言结构化导出-{target_language}.json").as_str(),
                            "application/json; charset=utf-8",
                            render_multilingual_json(
                                session_id.as_str(),
                                target_language.as_str(),
                                &translation,
                                &artifacts,
                            )?,
                            language_source_refs.clone(),
                        ));
                    }
                    "snapshot" => {
                        let snapshot_dto = ShareSnapshotDto {
                            id: format!("{}_snapshot_{}", bundle_id, target_language),
                            file_name: format!("声记多语言分享快照-{target_language}.html"),
                            title: format!("声记 Multilingual 分享快照 · {target_language}"),
                            html: render_multilingual_snapshot_html(
                                session_id.as_str(),
                                target_language.as_str(),
                                &translation,
                            ),
                            source_span_refs: language_source_refs.clone(),
                            privacy_summary: privacy_summary(),
                        };
                        items.push(build_item(
                            &bundle_id,
                            session_id.as_str(),
                            format!("snapshot_{target_language}").as_str(),
                            snapshot_dto.file_name.as_str(),
                            "text/html; charset=utf-8",
                            snapshot_dto.html.clone(),
                            language_source_refs.clone(),
                        ));
                        snapshot = Some(snapshot_dto);
                    }
                    _ => {
                        return Err(format!("暂不支持的导出格式: {format}"));
                    }
                }
            }
        }
    }

    for item in &items {
        record_export_item(connection, session_id.as_str(), item)?;
    }

    Ok(ExportBundleDto {
        id: bundle_id,
        session_id,
        provider: local_file::PROVIDER_ID.into(),
        status: "succeeded".into(),
        privacy_summary: privacy_summary(),
        items,
        snapshot,
    })
}

fn latest_exportable_session_id(connection: &Connection) -> Result<Option<String>, String> {
    connection
        .query_row(
            r#"
            SELECT conversation_sessions.id
            FROM conversation_sessions
            WHERE EXISTS (
              SELECT 1
              FROM semantic_artifacts
              WHERE semantic_artifacts.session_id = conversation_sessions.id
            )
            ORDER BY datetime(conversation_sessions.created_at) DESC, conversation_sessions.id DESC
            LIMIT 1
            "#,
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("查询可导出会话失败: {error}"))
}

fn query_transcript_segments(
    connection: &Connection,
    session_id: &str,
) -> Result<Vec<ExportTranscriptSegment>, String> {
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
            WHERE transcript_segments.conversation_session_id = ?1
            ORDER BY transcript_segments.start_ms ASC, transcript_segments.id ASC
            "#,
        )
        .map_err(|error| format!("准备转写导出查询失败: {error}"))?;

    let rows = statement
        .query_map(params![session_id], |row| {
            Ok(ExportTranscriptSegment {
                id: row.get(0)?,
                speaker_label: row.get(1)?,
                start_ms: row.get(2)?,
                end_ms: row.get(3)?,
                text: row.get(4)?,
            })
        })
        .map_err(|error| format!("查询转写导出失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取转写导出失败: {error}"))
}

fn query_artifacts(
    connection: &Connection,
    session_id: &str,
) -> Result<Vec<ExportArtifact>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, artifact_type, status, provider, model_name, source_span_refs, payload_json, error_message
            FROM semantic_artifacts
            WHERE session_id = ?1
            ORDER BY artifact_type ASC, id ASC
            "#,
        )
        .map_err(|error| format!("准备语义产物导出查询失败: {error}"))?;

    let rows = statement
        .query_map(params![session_id], |row| {
            let source_span_refs_json: String = row.get(5)?;
            Ok(ExportArtifact {
                id: row.get(0)?,
                artifact_type: row.get(1)?,
                status: row.get(2)?,
                provider: row.get(3)?,
                model_name: row.get(4)?,
                source_span_refs: serde_json::from_str::<Vec<String>>(
                    source_span_refs_json.as_str(),
                )
                .unwrap_or_default(),
                payload_json: row.get(6)?,
                error_message: row.get(7)?,
            })
        })
        .map_err(|error| format!("查询语义产物导出失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取语义产物导出失败: {error}"))
}

fn query_todos(connection: &Connection, session_id: &str) -> Result<Vec<ExportTodo>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, title, note, status, IFNULL(owner, ''), IFNULL(due_at, ''), priority, IFNULL(source_span_refs, '[]')
            FROM todos
            WHERE conversation_session_id = ?1
            ORDER BY datetime(created_at) ASC, id ASC
            "#,
        )
        .map_err(|error| format!("准备 Todo 导出查询失败: {error}"))?;

    let rows = statement
        .query_map(params![session_id], |row| {
            let source_span_refs_json: String = row.get(7)?;
            Ok(ExportTodo {
                id: row.get(0)?,
                title: row.get(1)?,
                note: row.get(2)?,
                status: row.get(3)?,
                owner: row.get(4)?,
                due_at: row.get(5)?,
                priority: row.get(6)?,
                source_span_refs: serde_json::from_str::<Vec<String>>(
                    source_span_refs_json.as_str(),
                )
                .unwrap_or_default(),
            })
        })
        .map_err(|error| format!("查询 Todo 导出失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取 Todo 导出失败: {error}"))
}

fn normalize_formats(formats: Vec<String>) -> Vec<String> {
    let requested = if formats.is_empty() {
        vec![
            "markdown".into(),
            "srt".into(),
            "json".into(),
            "snapshot".into(),
        ]
    } else {
        formats
    };

    let mut normalized = Vec::new();
    for format in requested {
        let value = format.trim().to_ascii_lowercase();
        if !value.is_empty() && !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    normalized
}

fn normalize_target_languages(target_languages: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for target_language in target_languages {
        let value = target_language
            .trim()
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
            .collect::<String>();
        if !value.is_empty() && !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    normalized
}

fn build_item(
    bundle_id: &str,
    session_id: &str,
    format: &str,
    file_name: &str,
    mime_type: &str,
    content: String,
    source_span_refs: Vec<String>,
) -> ExportItemDto {
    ExportItemDto {
        id: format!("{bundle_id}_{format}_{session_id}"),
        format: format.into(),
        file_name: file_name.into(),
        mime_type: mime_type.into(),
        content,
        status: "succeeded".into(),
        source_span_refs,
        error_message: String::new(),
    }
}

fn render_markdown(
    session_id: &str,
    transcript_segments: &[ExportTranscriptSegment],
    artifacts: &[ExportArtifact],
    todos: &[ExportTodo],
) -> String {
    let mut output = String::new();
    output.push_str("# 声记会话导出\n\n");
    output.push_str(&format!("- 会话：`{session_id}`\n"));
    output.push_str("- 导出边界：本地生成，不包含完整本地音频路径或 API Key。\n\n");

    output.push_str("## 摘要\n\n");
    if let Some(summary) = artifact_payload(artifacts, "summary") {
        output.push_str(&render_summary(&summary));
    } else {
        output.push_str("暂无摘要。\n");
    }
    output.push('\n');

    output.push_str("## 纪要\n\n");
    if let Some(minutes) = artifact_payload(artifacts, "meeting_minutes") {
        output.push_str(&render_minutes(&minutes));
    } else {
        output.push_str("暂无纪要。\n");
    }
    output.push('\n');

    output.push_str("## Todo\n\n");
    if todos.is_empty() {
        output.push_str("- 暂无 Todo。\n");
    } else {
        for todo in todos {
            output.push_str(&format!(
                "- [{}] {}（{}，{}）{}\n",
                if todo.status == "done" { "x" } else { " " },
                todo.title,
                todo.priority,
                value_or(todo.due_at.as_str(), "无截止时间"),
                if todo.note.is_empty() {
                    String::new()
                } else {
                    format!("：{}", todo.note)
                }
            ));
        }
    }
    output.push('\n');

    output.push_str("## 脑图\n\n");
    if let Some(mind_map) = artifact_payload(artifacts, "mind_map") {
        output.push_str(&render_mind_map(&mind_map));
    } else {
        output.push_str("暂无脑图。\n");
    }
    output.push('\n');

    output.push_str("## 转写\n\n");
    for segment in transcript_segments {
        output.push_str(&format!(
            "- `{}` {}：{}\n",
            format_time_range(segment.start_ms, segment.end_ms),
            value_or(segment.speaker_label.as_str(), "说话人"),
            segment.text
        ));
    }
    output.push('\n');

    output.push_str("## AI 产物来源\n\n");
    for artifact in artifacts {
        output.push_str(&format!(
            "- {}：{} / {} / {}{}\n",
            artifact.artifact_type,
            artifact.status,
            artifact.provider,
            artifact.model_name,
            if artifact.error_message.is_empty() {
                String::new()
            } else {
                format!(" / {}", artifact.error_message)
            }
        ));
    }

    output
}

fn render_summary(value: &Value) -> String {
    let mut output = String::new();
    if let Some(title) = value.get("title").and_then(Value::as_str) {
        output.push_str(&format!("### {title}\n\n"));
    }
    if let Some(basis) = value.get("basis").and_then(Value::as_str) {
        output.push_str(basis);
        output.push_str("\n\n");
    }
    if let Some(bullets) = value.get("bullets").and_then(Value::as_array) {
        for bullet in bullets.iter().filter_map(Value::as_str) {
            output.push_str(&format!("- {bullet}\n"));
        }
    }
    output
}

fn render_minutes(value: &Value) -> String {
    let mut output = String::new();
    for (label, key) in [
        ("决策", "decisions"),
        ("风险", "risks"),
        ("开放问题", "openQuestions"),
    ] {
        output.push_str(&format!("### {label}\n\n"));
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            for item in items.iter().filter_map(Value::as_str) {
                output.push_str(&format!("- {item}\n"));
            }
        }
        output.push('\n');
    }
    output
}

fn render_mind_map(value: &Value) -> String {
    let mut output = String::new();
    if let Some(summary) = value.get("summary").and_then(Value::as_str) {
        output.push_str(summary);
        output.push_str("\n\n");
    }
    if let Some(nodes) = value.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            let label = node.get("label").and_then(Value::as_str).unwrap_or("节点");
            let note = node.get("note").and_then(Value::as_str).unwrap_or("");
            output.push_str(&format!("- {label}"));
            if !note.is_empty() {
                output.push_str(&format!("：{note}"));
            }
            output.push('\n');
        }
    }
    output
}

fn render_srt(transcript_segments: &[ExportTranscriptSegment]) -> String {
    transcript_segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            format!(
                "{}\n{} --> {}\n{}：{}\n",
                index + 1,
                format_srt_time(segment.start_ms),
                format_srt_time(segment.end_ms.max(segment.start_ms + 1_000)),
                value_or(segment.speaker_label.as_str(), "说话人"),
                segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_json(
    session_id: &str,
    transcript_segments: &[ExportTranscriptSegment],
    artifacts: &[ExportArtifact],
    todos: &[ExportTodo],
) -> Result<String, String> {
    let value = json!({
        "sessionId": session_id,
        "provider": local_file::PROVIDER_ID,
        "privacySummary": privacy_summary(),
        "transcriptSegments": transcript_segments.iter().map(|segment| json!({
            "id": segment.id,
            "speakerLabel": segment.speaker_label,
            "startMs": segment.start_ms,
            "endMs": segment.end_ms,
            "text": segment.text,
        })).collect::<Vec<_>>(),
        "semanticArtifacts": artifacts.iter().map(|artifact| json!({
            "id": artifact.id,
            "artifactType": artifact.artifact_type,
            "status": artifact.status,
            "provider": artifact.provider,
            "modelName": artifact.model_name,
            "sourceSpanRefs": artifact.source_span_refs,
            "payload": serde_json::from_str::<Value>(artifact.payload_json.as_str()).unwrap_or(Value::Null),
            "errorMessage": artifact.error_message,
        })).collect::<Vec<_>>(),
        "todos": todos.iter().map(|todo| json!({
            "id": todo.id,
            "title": todo.title,
            "note": todo.note,
            "status": todo.status,
            "owner": todo.owner,
            "dueAt": todo.due_at,
            "priority": todo.priority,
            "sourceSpanRefs": todo.source_span_refs,
        })).collect::<Vec<_>>(),
        "exportsGeneratedAt": current_timestamp_label(),
    });
    serde_json::to_string_pretty(&value).map_err(|error| format!("生成 JSON 导出失败: {error}"))
}

fn render_multilingual_markdown(
    session_id: &str,
    target_language: &str,
    translation: &Value,
) -> String {
    let mut output = String::new();
    output.push_str("# ShengJi Multilingual Export\n\n");
    output.push_str(&format!("- Session: `{session_id}`\n"));
    output.push_str(&format!("- Target language: `{target_language}`\n"));
    output.push_str("- Privacy: generated locally from translation artifacts.\n\n");

    output.push_str("## Summary Translation\n\n");
    if let Some(summary) = translation.get("summaryTranslation") {
        if let Some(title) = summary.get("translatedTitle").and_then(Value::as_str) {
            output.push_str(&format!("### {title}\n\n"));
        }
        if let Some(basis) = summary.get("translatedBasis").and_then(Value::as_str) {
            output.push_str(basis);
            output.push_str("\n\n");
        }
        if let Some(bullets) = summary.get("translatedBullets").and_then(Value::as_array) {
            for bullet in bullets.iter().filter_map(Value::as_str) {
                output.push_str(&format!("- {bullet}\n"));
            }
        }
    }
    output.push('\n');

    output.push_str("## Transcript Translation\n\n");
    if let Some(segments) = translation
        .get("transcriptTranslations")
        .and_then(Value::as_array)
    {
        for segment in segments {
            let source_segment_id = segment
                .get("sourceSegmentId")
                .and_then(Value::as_str)
                .unwrap_or("");
            let speaker_label = segment
                .get("speakerLabel")
                .and_then(Value::as_str)
                .unwrap_or("Speaker");
            let translated_text = segment
                .get("translatedText")
                .and_then(Value::as_str)
                .unwrap_or("");
            output.push_str(&format!(
                "- Source segment `{source_segment_id}` · {speaker_label}: {translated_text}\n"
            ));
        }
    }

    output
}

fn render_multilingual_srt(translation: &Value) -> String {
    translation
        .get("transcriptTranslations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, segment)| {
            let start_ms = segment.get("startMs").and_then(Value::as_i64).unwrap_or(0);
            let end_ms = segment
                .get("endMs")
                .and_then(Value::as_i64)
                .unwrap_or(start_ms + 1_000);
            let speaker_label = segment
                .get("speakerLabel")
                .and_then(Value::as_str)
                .unwrap_or("Speaker");
            let translated_text = segment
                .get("translatedText")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!(
                "{}\n{} --> {}\n{}: {}\n",
                index + 1,
                format_srt_time(start_ms),
                format_srt_time(end_ms.max(start_ms + 1_000)),
                speaker_label,
                translated_text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_multilingual_json(
    session_id: &str,
    target_language: &str,
    translation: &Value,
    artifacts: &[ExportArtifact],
) -> Result<String, String> {
    let value = json!({
        "sessionId": session_id,
        "targetLanguage": target_language,
        "provider": local_file::PROVIDER_ID,
        "privacySummary": privacy_summary(),
        "translations": translation,
        "sourceArtifacts": artifacts.iter().filter(|artifact| {
            matches!(artifact.artifact_type.as_str(), "summary" | "transcript_revision" | "translation")
        }).map(|artifact| json!({
            "id": artifact.id,
            "artifactType": artifact.artifact_type,
            "status": artifact.status,
            "provider": artifact.provider,
            "modelName": artifact.model_name,
            "sourceSpanRefs": artifact.source_span_refs,
        })).collect::<Vec<_>>(),
        "exportsGeneratedAt": current_timestamp_label(),
    });
    serde_json::to_string_pretty(&value)
        .map_err(|error| format!("生成多语言 JSON 导出失败: {error}"))
}

fn render_snapshot_html(
    session_id: &str,
    transcript_segments: &[ExportTranscriptSegment],
    artifacts: &[ExportArtifact],
    todos: &[ExportTodo],
) -> String {
    let summary = artifact_payload(artifacts, "summary")
        .and_then(|value| {
            value
                .get("basis")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "暂无摘要".into());
    let todo_items = if todos.is_empty() {
        "<li>暂无 Todo</li>".to_string()
    } else {
        todos
            .iter()
            .map(|todo| {
                format!(
                    "<li><strong>{}</strong><span>{}</span></li>",
                    escape_html(todo.title.as_str()),
                    escape_html(todo.status.as_str())
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let transcript = transcript_segments
        .iter()
        .take(8)
        .map(|segment| {
            format!(
                "<li><time>{}</time><span>{}</span></li>",
                escape_html(format_time_range(segment.start_ms, segment.end_ms).as_str()),
                escape_html(segment.text.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>声记分享快照</title>
  <style>
    body {{ margin: 0; font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif; background: #f5f5f7; color: #1d1d1f; }}
    main {{ max-width: 920px; margin: 0 auto; padding: 40px 24px; }}
    header {{ padding: 32px 0 24px; border-bottom: 1px solid #d2d2d7; }}
    h1 {{ margin: 0 0 8px; font-size: 34px; letter-spacing: 0; }}
    section {{ padding: 24px 0; border-bottom: 1px solid #e5e5ea; }}
    ul {{ margin: 0; padding-left: 18px; }}
    li {{ margin: 8px 0; }}
    time {{ display: inline-block; min-width: 120px; color: #6e6e73; }}
    .meta {{ color: #6e6e73; font-size: 14px; }}
  </style>
</head>
<body>
  <main>
    <header>
      <h1>声记分享快照</h1>
      <p class="meta">会话 {session_id} · {privacy}</p>
    </header>
    <section>
      <h2>摘要</h2>
      <p>{summary}</p>
    </section>
    <section>
      <h2>Todo</h2>
      <ul>{todo_items}</ul>
    </section>
    <section>
      <h2>转写片段</h2>
      <ul>{transcript}</ul>
    </section>
  </main>
</body>
</html>"#,
        session_id = escape_html(session_id),
        privacy = escape_html(privacy_summary().as_str()),
        summary = escape_html(summary.as_str()),
        todo_items = todo_items,
        transcript = transcript,
    )
}

fn render_multilingual_snapshot_html(
    session_id: &str,
    target_language: &str,
    translation: &Value,
) -> String {
    let summary = translation
        .get("summaryTranslation")
        .and_then(|value| value.get("translatedBasis"))
        .and_then(Value::as_str)
        .unwrap_or("No translated summary yet.");
    let transcript = translation
        .get("transcriptTranslations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(8)
        .map(|segment| {
            let source_segment_id = segment
                .get("sourceSegmentId")
                .and_then(Value::as_str)
                .unwrap_or("");
            let translated_text = segment
                .get("translatedText")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!(
                "<li><time>{}</time><span>{}</span></li>",
                escape_html(source_segment_id),
                escape_html(translated_text)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ShengJi Multilingual Snapshot</title>
  <style>
    body {{ margin: 0; font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif; background: #f5f5f7; color: #1d1d1f; }}
    main {{ max-width: 920px; margin: 0 auto; padding: 40px 24px; }}
    header {{ padding: 32px 0 24px; border-bottom: 1px solid #d2d2d7; }}
    h1 {{ margin: 0 0 8px; font-size: 34px; letter-spacing: 0; }}
    section {{ padding: 24px 0; border-bottom: 1px solid #e5e5ea; }}
    ul {{ margin: 0; padding-left: 18px; }}
    li {{ margin: 8px 0; }}
    time {{ display: inline-block; min-width: 180px; color: #6e6e73; }}
    .meta {{ color: #6e6e73; font-size: 14px; }}
  </style>
</head>
<body>
  <main>
    <header>
      <h1>ShengJi Multilingual Snapshot</h1>
      <p class="meta">Session {session_id} · Language {target_language} · Multilingual local export</p>
    </header>
    <section>
      <h2>Summary Translation</h2>
      <p>{summary}</p>
    </section>
    <section>
      <h2>Transcript Translation</h2>
      <ul>{transcript}</ul>
    </section>
  </main>
</body>
</html>"#,
        session_id = escape_html(session_id),
        target_language = escape_html(target_language),
        summary = escape_html(summary),
        transcript = transcript,
    )
}

fn artifact_payload<'a>(artifacts: &'a [ExportArtifact], artifact_type: &str) -> Option<Value> {
    artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == artifact_type && artifact.status == "succeeded")
        .and_then(|artifact| serde_json::from_str::<Value>(artifact.payload_json.as_str()).ok())
}

fn translation_payload_for_language(
    artifacts: &[ExportArtifact],
    target_language: &str,
) -> Option<Value> {
    artifacts
        .iter()
        .filter(|artifact| {
            artifact.artifact_type == "translation" && artifact.status == "succeeded"
        })
        .filter_map(|artifact| serde_json::from_str::<Value>(artifact.payload_json.as_str()).ok())
        .find(|value| {
            value
                .get("targetLanguage")
                .and_then(Value::as_str)
                .map(|candidate| candidate == target_language)
                .unwrap_or(false)
        })
}

fn translation_source_span_refs(translation: &Value) -> Vec<String> {
    translation
        .get("sourceSpanRefs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn collect_source_span_refs(artifacts: &[ExportArtifact], todos: &[ExportTodo]) -> Vec<String> {
    let mut refs = Vec::new();
    for artifact in artifacts {
        for source_ref in &artifact.source_span_refs {
            if !refs.contains(source_ref) {
                refs.push(source_ref.clone());
            }
        }
    }
    for todo in todos {
        for source_ref in &todo.source_span_refs {
            if !refs.contains(source_ref) {
                refs.push(source_ref.clone());
            }
        }
    }
    refs
}

fn record_export_item(
    connection: &Connection,
    session_id: &str,
    item: &ExportItemDto,
) -> Result<(), String> {
    let source_span_refs_json =
        serde_json::to_string(&item.source_span_refs).unwrap_or_else(|_| "[]".to_string());
    let content_preview = item.content.chars().take(512).collect::<String>();
    connection
        .execute(
            r#"
            INSERT INTO external_exports (
              id,
              session_id,
              export_type,
              format,
              status,
              provider,
              file_name,
              mime_type,
              content_preview,
              source_span_refs,
              error_message,
              created_at,
              updated_at
            ) VALUES (?1, ?2, 'session_bundle', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
            params![
                item.id,
                session_id,
                item.format,
                item.status,
                local_file::PROVIDER_ID,
                item.file_name,
                item.mime_type,
                content_preview,
                source_span_refs_json,
                item.error_message,
            ],
        )
        .map_err(|error| format!("记录导出结果失败: {error}"))?;
    Ok(())
}

fn privacy_summary() -> String {
    "本地生成导出包与分享快照；不上传音频、完整路径、API Key 或外部账号信息。".into()
}

fn format_time_range(start_ms: i64, end_ms: i64) -> String {
    format!(
        "{} - {}",
        format_plain_time(start_ms),
        format_plain_time(end_ms)
    )
}

fn format_plain_time(milliseconds: i64) -> String {
    let total_seconds = milliseconds.max(0) / 1_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn format_srt_time(milliseconds: i64) -> String {
    let safe_milliseconds = milliseconds.max(0);
    let millis = safe_milliseconds % 1_000;
    let total_seconds = safe_milliseconds / 1_000;
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let hours = total_minutes / 60;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

fn value_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
