use rusqlite::{params, Connection};

use crate::{
    app::workspace_builder::SemanticWorkspace,
    domain::correction::{CorrectionPatternDto, DeletedCorrectionPatternDto, TranscriptRevisionDto},
};

pub(crate) fn build_revisions(
    workspace: &SemanticWorkspace,
) -> Vec<TranscriptRevisionDto> {
    workspace
        .segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            let (revised_text, change_level, correction_type, reason_summary) =
                revise_segment_text(segment.text.as_str(), index);
            TranscriptRevisionDto {
                id: format!("revision_{}_{}", workspace.session_id, index),
                session_id: workspace.session_id.clone(),
                source_segment_id: segment.id.clone(),
                speaker_label: segment.speaker_label.clone(),
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                original_text: segment.text.clone(),
                revised_text,
                change_level,
                correction_type,
                reason_summary,
                status: "proposed".into(),
            }
        })
        .collect()
}

pub(crate) fn upsert_default_correction_patterns(
    connection: &Connection,
) -> Result<Vec<CorrectionPatternDto>, String> {
    let patterns = [
        (
            "pattern_argmax_contract",
            "Argmax 输出",
            "Argmax ASR 输出",
            "domain_term",
            0.92,
        ),
        (
            "pattern_speaker_label",
            "speaker label",
            "说话人标签",
            "speaker_alias",
            0.86,
        ),
    ];

    for (id, phrase, replacement, pattern_type, confidence) in patterns {
        connection
            .execute(
                r#"
                INSERT INTO transcript_correction_patterns (
                  id,
                  phrase,
                  replacement,
                  pattern_type,
                  scope,
                  confidence,
                  enabled,
                  created_at,
                  updated_at
                ) VALUES (?1, ?2, ?3, ?4, 'local', ?5, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT(id) DO UPDATE SET
                  replacement = excluded.replacement,
                  confidence = excluded.confidence,
                  updated_at = CURRENT_TIMESTAMP
                "#,
                params![id, phrase, replacement, pattern_type, confidence],
            )
            .map_err(|error| format!("写入修正记忆失败: {error}"))?;
    }

    query_correction_patterns(connection)
}

pub(crate) fn query_correction_patterns(
    connection: &Connection,
) -> Result<Vec<CorrectionPatternDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, phrase, replacement, pattern_type, scope, confidence, enabled
            FROM transcript_correction_patterns
            ORDER BY enabled DESC, id ASC
            "#,
        )
        .map_err(|error| format!("准备修正记忆查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(CorrectionPatternDto {
                id: row.get(0)?,
                phrase: row.get(1)?,
                replacement: row.get(2)?,
                pattern_type: row.get(3)?,
                scope: row.get(4)?,
                confidence: row.get(5)?,
                enabled: row.get::<_, i64>(6)? == 1,
            })
        })
        .map_err(|error| format!("查询修正记忆失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取修正记忆失败: {error}"))
}

pub(crate) fn set_correction_pattern_enabled(
    connection: &Connection,
    pattern_id: &str,
    enabled: bool,
) -> Result<CorrectionPatternDto, String> {
    let updated_count = connection
        .execute(
            r#"
            UPDATE transcript_correction_patterns
            SET enabled = ?1, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?2
            "#,
            params![if enabled { 1 } else { 0 }, pattern_id],
        )
        .map_err(|error| format!("更新修正记忆状态失败: {error}"))?;

    if updated_count == 0 {
        return Err("未找到修正记忆".to_string());
    }

    query_correction_patterns(connection)?
        .into_iter()
        .find(|pattern| pattern.id == pattern_id)
        .ok_or_else(|| "未找到修正记忆".to_string())
}

pub(crate) fn delete_correction_pattern(
    connection: &Connection,
    pattern_id: &str,
) -> Result<DeletedCorrectionPatternDto, String> {
    let deleted_count = connection
        .execute(
            "DELETE FROM transcript_correction_patterns WHERE id = ?1",
            params![pattern_id],
        )
        .map_err(|error| format!("删除修正记忆失败: {error}"))?;

    if deleted_count == 0 {
        return Err("未找到可删除的修正记忆".to_string());
    }

    Ok(DeletedCorrectionPatternDto {
        deleted_id: pattern_id.to_string(),
    })
}

fn revise_segment_text(text: &str, index: usize) -> (String, String, String, String) {
    if text.contains("speaker label") {
        return (
            text.replace("speaker label", "说话人标签"),
            "meaning_affecting".into(),
            "speaker_inconsistency".into(),
            "将英文 speaker label 归一为中文说话人标签，便于纪要追溯。".into(),
        );
    }

    if text.contains("Argmax 输出") {
        return (
            text.replace("Argmax 输出", "Argmax ASR 输出"),
            "wording".into(),
            "domain_term".into(),
            "补全 ASR 领域术语，降低后续语义模板歧义。".into(),
        );
    }

    if index == 0 {
        return (
            text.replace("本地转写评估开始。", "本地转写评估已开始。"),
            "punctuation".into(),
            "punctuation".into(),
            "优化句式和标点，使修正文稿更适合摘要输入。".into(),
        );
    }

    (
        text.to_string(),
        "none".into(),
        "none".into(),
        "无需修正，保留原始转写。".into(),
    )
}

pub(crate) fn revision_payload_json(revisions: &[TranscriptRevisionDto]) -> String {
    serde_json::to_string(revisions).unwrap_or_else(|_| "[]".to_string())
}
