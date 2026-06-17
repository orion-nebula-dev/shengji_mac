use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    current_timestamp_label,
    domain::{
        artifact::TodoCandidateDto as ArtifactTodoCandidateDto,
        todo::{AcceptTodoCandidateCommand, TodoCandidateDto, TodoDto, UpdateTodoCandidateCommand},
    },
    providers::semantic::minimax_m3,
};

const VALID_TODO_STATUSES: [&str; 4] = ["open", "in_progress", "done", "dismissed"];
const VALID_TODO_PRIORITIES: [&str; 3] = ["low", "medium", "high"];

pub(crate) fn sync_todo_candidates(
    connection: &Connection,
) -> Result<Vec<TodoCandidateDto>, String> {
    let Some(artifact) = latest_todo_extraction_artifact(connection)? else {
        return query_todo_candidates(connection);
    };

    let source_span_refs = parse_string_array(artifact.source_span_refs.as_str());
    let parsed_candidates =
        serde_json::from_str::<Vec<ArtifactTodoCandidateDto>>(artifact.payload_json.as_str())
            .map_err(|error| format!("解析待办候选失败: {error}"))?;

    for (index, candidate) in parsed_candidates.iter().enumerate() {
        let source_refs = if candidate.source_segment_ids.is_empty() {
            source_span_refs.clone()
        } else {
            candidate.source_segment_ids.clone()
        };
        let source_refs_json =
            serde_json::to_string(&source_refs).unwrap_or_else(|_| "[]".to_string());
        let source_text = source_text_for_refs(connection, source_refs.as_slice())?;
        let candidate_id = format!("todo_candidate_{}_{}", artifact.id, index);
        let dedup_key = todo_dedup_key(artifact.session_id.as_str(), candidate.title.as_str());
        connection
            .execute(
                r#"
                INSERT INTO todo_candidates (
                  id,
                  session_id,
                  artifact_id,
                  title,
                  detail,
                  owner,
                  due_at,
                  priority,
                  confidence,
                  source_span_refs,
                  source_text,
                  status,
                  todo_id,
                  dedup_key,
                  created_at,
                  updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', ?7, ?8, ?9, ?10, 'proposed', '', ?11, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT(id) DO UPDATE SET
                  artifact_id = excluded.artifact_id,
                  session_id = excluded.session_id,
                  title = excluded.title,
                  detail = excluded.detail,
                  owner = excluded.owner,
                  priority = excluded.priority,
                  confidence = excluded.confidence,
                  source_span_refs = excluded.source_span_refs,
                  source_text = excluded.source_text,
                  updated_at = CURRENT_TIMESTAMP
                "#,
                params![
                    candidate_id,
                    artifact.session_id,
                    artifact.id,
                    candidate.title,
                    candidate.detail,
                    candidate.owner,
                    candidate.priority,
                    candidate.confidence,
                    source_refs_json,
                    source_text,
                    dedup_key,
                ],
            )
            .map_err(|error| format!("写入待办候选失败: {error}"))?;
    }

    query_todo_candidates(connection)
}

pub(crate) fn query_todo_candidates(
    connection: &Connection,
) -> Result<Vec<TodoCandidateDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT
              id,
              session_id,
              artifact_id,
              title,
              detail,
              owner,
              due_at,
              priority,
              confidence,
              status,
              source_span_refs,
              source_text,
              todo_id
            FROM todo_candidates
            ORDER BY
              CASE status
                WHEN 'proposed' THEN 0
                WHEN 'accepted' THEN 1
                WHEN 'dismissed' THEN 2
                ELSE 3
              END,
              datetime(created_at) DESC,
              id ASC
            "#,
        )
        .map_err(|error| format!("准备待办候选查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let source_refs_json: String = row.get(10)?;
            Ok(TodoCandidateDto {
                id: row.get(0)?,
                session_id: row.get(1)?,
                artifact_id: row.get(2)?,
                title: row.get(3)?,
                detail: row.get(4)?,
                owner: row.get(5)?,
                due_at: row.get(6)?,
                priority: row.get(7)?,
                confidence: row.get(8)?,
                status: row.get(9)?,
                source_span_refs: parse_string_array(source_refs_json.as_str()),
                source_text: row.get(11)?,
                todo_id: row.get(12)?,
            })
        })
        .map_err(|error| format!("查询待办候选失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取待办候选失败: {error}"))
}

pub(crate) fn accept_todo_candidate(
    connection: &Connection,
    command: AcceptTodoCandidateCommand,
) -> Result<TodoDto, String> {
    let candidate = query_todo_candidate(connection, command.candidate_id.as_str())?;
    if !candidate.todo_id.is_empty() {
        return query_todo(connection, candidate.todo_id.as_str());
    }

    let title = first_non_empty(command.title.as_str(), candidate.title.as_str());
    let note = first_non_empty(command.detail.as_str(), candidate.detail.as_str());
    let owner = command.owner.trim();
    let due_at = command.due_at.trim();
    let priority = first_non_empty(command.priority.as_str(), candidate.priority.as_str());
    let dedup_key = todo_dedup_key(candidate.session_id.as_str(), title);

    if let Some(existing_id) = existing_todo_id_by_dedup_key(connection, dedup_key.as_str())? {
        connection
            .execute(
                r#"
                UPDATE todo_candidates
                SET status = 'accepted',
                    todo_id = ?1,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?2
                "#,
                params![existing_id, candidate.id],
            )
            .map_err(|error| format!("更新待办候选状态失败: {error}"))?;
        return query_todo(connection, existing_id.as_str());
    }

    let todo_id = format!("todo_v07_{}", current_timestamp_label());
    let source_refs_json =
        serde_json::to_string(&candidate.source_span_refs).unwrap_or_else(|_| "[]".to_string());
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
            ) VALUES (?1, ?2, ?3, ?4, 'open', CURRENT_TIMESTAMP, ?5, ?6, ?7, CURRENT_TIMESTAMP, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                todo_id,
                candidate.session_id,
                title,
                note,
                candidate.source_text,
                minimax_m3::DEFAULT_MODEL_NAME,
                format!("trace_{}_accepted", candidate.id),
                owner,
                due_at,
                priority,
                source_refs_json,
                candidate.id,
                dedup_key,
            ],
        )
        .map_err(|error| format!("写入正式 Todo 失败: {error}"))?;

    connection
        .execute(
            r#"
            UPDATE todo_candidates
            SET status = 'accepted',
                todo_id = ?1,
                title = ?2,
                detail = ?3,
                owner = ?4,
                due_at = ?5,
                priority = ?6,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?7
            "#,
            params![todo_id, title, note, owner, due_at, priority, candidate.id],
        )
        .map_err(|error| format!("更新待办候选状态失败: {error}"))?;

    query_todo(connection, todo_id.as_str())
}

pub(crate) fn update_todo_candidate(
    connection: &Connection,
    command: UpdateTodoCandidateCommand,
) -> Result<TodoCandidateDto, String> {
    let candidate = query_todo_candidate(connection, command.candidate_id.as_str())?;
    if candidate.status != "proposed" {
        return Err("仅待确认候选可以编辑".to_string());
    }

    let title = first_non_empty(command.title.as_str(), candidate.title.as_str());
    if title.trim().is_empty() {
        return Err("候选标题不能为空".to_string());
    }
    let detail = first_non_empty(command.detail.as_str(), candidate.detail.as_str());
    let priority = first_non_empty(command.priority.as_str(), candidate.priority.as_str());
    if !VALID_TODO_PRIORITIES.contains(&priority) {
        return Err("不支持的候选优先级".to_string());
    }
    let dedup_key = todo_dedup_key(candidate.session_id.as_str(), title);

    connection
        .execute(
            r#"
            UPDATE todo_candidates
            SET title = ?1,
                detail = ?2,
                owner = ?3,
                due_at = ?4,
                priority = ?5,
                dedup_key = ?6,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?7 AND status = 'proposed'
            "#,
            params![
                title,
                detail,
                command.owner.trim(),
                command.due_at.trim(),
                priority,
                dedup_key,
                candidate.id,
            ],
        )
        .map_err(|error| format!("更新待办候选失败: {error}"))?;

    query_todo_candidate(connection, command.candidate_id.as_str())
}

pub(crate) fn dismiss_todo_candidate(
    connection: &Connection,
    candidate_id: &str,
) -> Result<TodoCandidateDto, String> {
    let updated_count = connection
        .execute(
            r#"
            UPDATE todo_candidates
            SET status = 'dismissed',
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1 AND status = 'proposed'
            "#,
            params![candidate_id],
        )
        .map_err(|error| format!("忽略待办候选失败: {error}"))?;

    if updated_count == 0 {
        return Err("未找到可忽略的待办候选".to_string());
    }

    query_todo_candidate(connection, candidate_id)
}

pub(crate) fn update_todo_status(
    connection: &Connection,
    todo_id: &str,
    status: &str,
) -> Result<TodoDto, String> {
    if !VALID_TODO_STATUSES.contains(&status) {
        return Err("不支持的 Todo 状态".to_string());
    }

    let updated_count = connection
        .execute(
            r#"
            UPDATE todos
            SET status = ?1,
                completed_at = CASE WHEN ?1 = 'done' THEN CURRENT_TIMESTAMP ELSE NULL END,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?2
            "#,
            params![status, todo_id],
        )
        .map_err(|error| format!("更新 Todo 状态失败: {error}"))?;

    if updated_count == 0 {
        return Err("未找到 Todo".to_string());
    }

    query_todo(connection, todo_id)
}

pub(crate) fn query_todo(connection: &Connection, todo_id: &str) -> Result<TodoDto, String> {
    connection
        .query_row(
            r#"
            SELECT
              id,
              title,
              note,
              status,
              created_at,
              conversation_session_id,
              IFNULL(source_text, ''),
              IFNULL(owner, ''),
              IFNULL(due_at, ''),
              IFNULL(priority, 'medium'),
              IFNULL(source_span_refs, '[]'),
              IFNULL(candidate_id, '')
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
                    source_span_refs: parse_string_array(source_refs_json.as_str()),
                    candidate_id: row.get(11)?,
                })
            },
        )
        .map_err(|error| format!("读取 Todo 失败: {error}"))
}

fn query_todo_candidate(
    connection: &Connection,
    candidate_id: &str,
) -> Result<TodoCandidateDto, String> {
    query_todo_candidates(connection)?
        .into_iter()
        .find(|candidate| candidate.id == candidate_id)
        .ok_or_else(|| "未找到待办候选".to_string())
}

fn latest_todo_extraction_artifact(
    connection: &Connection,
) -> Result<Option<TodoExtractionArtifact>, String> {
    connection
        .query_row(
            r#"
            SELECT id, session_id, source_span_refs, payload_json
            FROM semantic_artifacts
            WHERE artifact_type = 'todo_extraction' AND status = 'succeeded'
            ORDER BY datetime(updated_at) DESC, id DESC
            LIMIT 1
            "#,
            [],
            |row| {
                Ok(TodoExtractionArtifact {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    source_span_refs: row.get(2)?,
                    payload_json: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取待办语义产物失败: {error}"))
}

fn source_text_for_refs(connection: &Connection, refs: &[String]) -> Result<String, String> {
    let mut parts = Vec::new();
    for source_ref in refs {
        if let Some(text) = connection
            .query_row(
                "SELECT text FROM transcript_segments WHERE id = ?1",
                params![source_ref],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("读取待办来源片段失败: {error}"))?
        {
            parts.push(format!("{source_ref}: {text}"));
        }
    }
    Ok(parts.join("\n"))
}

fn existing_todo_id_by_dedup_key(
    connection: &Connection,
    dedup_key: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT id FROM todos WHERE dedup_key = ?1 LIMIT 1",
            params![dedup_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("查询重复 Todo 失败: {error}"))
}

fn todo_dedup_key(session_id: &str, title: &str) -> String {
    let normalized_title = title
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    format!("{session_id}::{normalized_title}")
}

fn parse_string_array(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

fn first_non_empty<'a>(candidate: &'a str, fallback: &'a str) -> &'a str {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        fallback.trim()
    } else {
        trimmed
    }
}

struct TodoExtractionArtifact {
    id: String,
    session_id: String,
    source_span_refs: String,
    payload_json: String,
}
