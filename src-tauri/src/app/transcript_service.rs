use std::path::Path;

use rusqlite::{params, Connection};

use crate::{
    current_timestamp_label,
    domain::{
        speaker::SpeakerDto,
        transcript::{
            LocalModelStatusDto, TranscriptAudioDto, TranscriptJobDto, TranscriptReviewDto,
            TranscriptSegmentDto,
        },
    },
    providers::asr::local_whisperkit,
};

const DEFAULT_TRANSCRIPT_MODEL: &str = "large-v3-turbo";

pub(crate) fn import_local_audio(
    connection: &Connection,
    file_path: &str,
) -> Result<TranscriptReviewDto, String> {
    let path = Path::new(file_path);
    if file_path.trim().is_empty() {
        return Err("本地音频路径不能为空".to_string());
    }
    if !path.exists() {
        return Err("本地音频文件不存在，请检查路径".to_string());
    }

    let timestamp = current_timestamp_label();
    let audio_id = format!("audio_import_{timestamp}");
    let trace_id = format!("trace_import_{timestamp}");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("imported-audio.wav")
        .to_string();

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
            ) VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 45000, 16000, 1, 1, 0.76, 'transcribed', ?3, CURRENT_TIMESTAMP)
            "#,
            params![audio_id.as_str(), file_path, trace_id.as_str()],
        )
        .map_err(|error| format!("写入导入音频失败: {error}"))?;

    connection
        .execute(
            r#"
            INSERT INTO transcript_jobs (
              id,
              audio_segment_id,
              status,
              retry_count,
              max_retry_count,
              error_message,
              provider,
              model_name,
              started_at,
              finished_at,
              created_at,
              updated_at
            ) VALUES (?1, ?2, 'succeeded', 0, 3, '', ?3, ?4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
            params![
                format!("transcript_job_{timestamp}"),
                audio_id.as_str(),
                local_whisperkit::PROVIDER_ID,
                DEFAULT_TRANSCRIPT_MODEL,
            ],
        )
        .map_err(|error| format!("写入转写任务失败: {error}"))?;

    ensure_default_speakers(connection)?;
    insert_demo_timeline(connection, &audio_id, &trace_id, &file_name)?;
    get_transcript_review(connection)
}

pub(crate) fn get_transcript_review(
    connection: &Connection,
) -> Result<TranscriptReviewDto, String> {
    let audio = latest_audio(connection)?;
    let Some(audio) = audio else {
        return Ok(TranscriptReviewDto {
            audio: TranscriptAudioDto {
                id: String::new(),
                file_name: "暂无音频".into(),
                duration_ms: 0,
                status: "empty".into(),
                provider: local_whisperkit::PROVIDER_ID.into(),
                model_name: DEFAULT_TRANSCRIPT_MODEL.into(),
                offline_available: true,
            },
            segments: Vec::new(),
            speakers: query_speakers(connection)?,
            jobs: query_transcript_jobs(connection)?,
            model_status: query_local_model_status(connection)?,
        });
    };

    Ok(TranscriptReviewDto {
        segments: query_segments(connection, &audio.id)?,
        speakers: query_speakers(connection)?,
        jobs: query_transcript_jobs(connection)?,
        model_status: query_local_model_status(connection)?,
        audio,
    })
}

pub(crate) fn rename_speaker(
    connection: &Connection,
    speaker_id: &str,
    label: &str,
) -> Result<SpeakerDto, String> {
    let next_label = label.trim();
    if next_label.is_empty() {
        return Err("说话人名称不能为空".to_string());
    }

    connection
        .execute(
            r#"
            UPDATE speakers
            SET label = ?1, display_name = ?1, corrected = 1, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?2
            "#,
            params![next_label, speaker_id],
        )
        .map_err(|error| format!("更新说话人名称失败: {error}"))?;

    query_speaker(connection, speaker_id)?.ok_or_else(|| "未找到说话人".to_string())
}

pub(crate) fn mark_transcript_segment(
    connection: &Connection,
    segment_id: &str,
    _issue_type: &str,
    reason: &str,
) -> Result<TranscriptSegmentDto, String> {
    let review_reason = reason.trim();
    connection
        .execute(
            r#"
            UPDATE transcript_segments
            SET review_status = 'flagged', review_reason = ?1
            WHERE id = ?2
            "#,
            params![review_reason, segment_id],
        )
        .map_err(|error| format!("标注转写片段失败: {error}"))?;

    query_segment(connection, segment_id)?.ok_or_else(|| "未找到转写片段".to_string())
}

pub(crate) fn retry_transcript_job(
    connection: &Connection,
    job_id: &str,
) -> Result<TranscriptJobDto, String> {
    connection
        .execute(
            r#"
            UPDATE transcript_jobs
            SET status = 'queued',
                retry_count = retry_count + 1,
                error_message = '',
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
              AND status = 'failed'
              AND retry_count < max_retry_count
            "#,
            params![job_id],
        )
        .map_err(|error| format!("重试转写任务失败: {error}"))?;

    query_transcript_job(connection, job_id)?.ok_or_else(|| "未找到可重试转写任务".to_string())
}

fn ensure_default_speakers(connection: &Connection) -> Result<(), String> {
    for (id, label, color) in [
        ("speaker_1", "Speaker 1", "#2f7df6"),
        ("speaker_2", "Speaker 2", "#34a853"),
    ] {
        connection
            .execute(
                r#"
                INSERT OR IGNORE INTO speakers (
                  id,
                  label,
                  display_name,
                  color,
                  corrected,
                  created_at,
                  updated_at
                ) VALUES (?1, ?2, ?2, ?3, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                "#,
                params![id, label, color],
            )
            .map_err(|error| format!("初始化说话人失败: {error}"))?;
    }
    Ok(())
}

fn insert_demo_timeline(
    connection: &Connection,
    audio_id: &str,
    trace_id: &str,
    file_name: &str,
) -> Result<(), String> {
    let rows = [
        (
            "speaker_1",
            0,
            12_500,
            format!("已导入 {file_name}，本地转写评估开始。"),
            0.91,
        ),
        (
            "speaker_2",
            12_500,
            27_000,
            "请检查 speaker label、时间跳转和错误片段标注。".to_string(),
            0.87,
        ),
        (
            "speaker_1",
            27_000,
            45_000,
            "当前为离线评估样例，真实 Argmax 输出可替换同一 AsrOutput 契约。".to_string(),
            0.84,
        ),
    ];

    for (index, (speaker_id, start_ms, end_ms, text, confidence)) in rows.into_iter().enumerate() {
        let segment_id = format!("transcript_{audio_id}_{index}");
        let speaker_segment_id = format!("speaker_segment_{audio_id}_{index}");
        connection
            .execute(
                r#"
                INSERT INTO transcript_segments (
                  id,
                  audio_segment_id,
                  speaker_id,
                  start_ms,
                  end_ms,
                  text,
                  confidence,
                  language,
                  status,
                  provider,
                  provider_model_name,
                  review_status,
                  review_reason,
                  trace_id,
                  created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'zh-CN', 'success', ?8, ?9, 'normal', '', ?10, CURRENT_TIMESTAMP)
                "#,
                params![
                    segment_id,
                    audio_id,
                    speaker_id,
                    start_ms,
                    end_ms,
                    text,
                    confidence,
                    local_whisperkit::PROVIDER_ID,
                    DEFAULT_TRANSCRIPT_MODEL,
                    trace_id,
                ],
            )
            .map_err(|error| format!("写入转写片段失败: {error}"))?;

        connection
            .execute(
                r#"
                INSERT INTO speaker_segments (
                  id,
                  speaker_id,
                  audio_segment_id,
                  start_ms,
                  end_ms,
                  confidence,
                  corrected,
                  created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, CURRENT_TIMESTAMP)
                "#,
                params![
                    speaker_segment_id,
                    speaker_id,
                    audio_id,
                    start_ms,
                    end_ms,
                    confidence,
                ],
            )
            .map_err(|error| format!("写入说话人片段失败: {error}"))?;
    }

    Ok(())
}

fn latest_audio(connection: &Connection) -> Result<Option<TranscriptAudioDto>, String> {
    let row = connection
        .query_row(
            r#"
            SELECT
              audio_segments.id,
              audio_segments.file_path,
              audio_segments.duration_ms,
              IFNULL(transcript_jobs.status, audio_segments.processing_status),
              IFNULL(transcript_jobs.provider, ?1),
              IFNULL(transcript_jobs.model_name, ?2)
            FROM audio_segments
            LEFT JOIN transcript_jobs ON transcript_jobs.audio_segment_id = audio_segments.id
            ORDER BY datetime(audio_segments.created_at) DESC, audio_segments.id DESC
            LIMIT 1
            "#,
            params![local_whisperkit::PROVIDER_ID, DEFAULT_TRANSCRIPT_MODEL],
            |row| {
                let path: String = row.get(1)?;
                let file_name = Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("imported-audio.wav")
                    .to_string();
                Ok(TranscriptAudioDto {
                    id: row.get(0)?,
                    file_name,
                    duration_ms: row.get(2)?,
                    status: row.get(3)?,
                    provider: row.get(4)?,
                    model_name: row.get(5)?,
                    offline_available: true,
                })
            },
        )
        .ok();
    Ok(row)
}

fn query_segments(
    connection: &Connection,
    audio_id: &str,
) -> Result<Vec<TranscriptSegmentDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT
              transcript_segments.id,
              transcript_segments.audio_segment_id,
              transcript_segments.speaker_id,
              IFNULL(speakers.label, transcript_segments.speaker_id),
              transcript_segments.start_ms,
              transcript_segments.end_ms,
              transcript_segments.text,
              transcript_segments.confidence,
              transcript_segments.provider,
              transcript_segments.review_status,
              transcript_segments.review_reason
            FROM transcript_segments
            LEFT JOIN speakers ON speakers.id = transcript_segments.speaker_id
            WHERE transcript_segments.audio_segment_id = ?1
            ORDER BY transcript_segments.start_ms ASC, transcript_segments.id ASC
            "#,
        )
        .map_err(|error| format!("准备转写片段查询失败: {error}"))?;

    let rows = statement
        .query_map(params![audio_id], |row| {
            Ok(TranscriptSegmentDto {
                id: row.get(0)?,
                audio_segment_id: row.get(1)?,
                speaker_id: row.get(2)?,
                speaker_label: row.get(3)?,
                start_ms: row.get(4)?,
                end_ms: row.get(5)?,
                text: row.get(6)?,
                confidence: row.get(7)?,
                provider: row.get(8)?,
                review_status: row.get(9)?,
                review_reason: row.get(10)?,
            })
        })
        .map_err(|error| format!("查询转写片段失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取转写片段失败: {error}"))
}

fn query_segment(
    connection: &Connection,
    segment_id: &str,
) -> Result<Option<TranscriptSegmentDto>, String> {
    let row = connection
        .query_row(
            r#"
            SELECT
              transcript_segments.id,
              transcript_segments.audio_segment_id,
              transcript_segments.speaker_id,
              IFNULL(speakers.label, transcript_segments.speaker_id),
              transcript_segments.start_ms,
              transcript_segments.end_ms,
              transcript_segments.text,
              transcript_segments.confidence,
              transcript_segments.provider,
              transcript_segments.review_status,
              transcript_segments.review_reason
            FROM transcript_segments
            LEFT JOIN speakers ON speakers.id = transcript_segments.speaker_id
            WHERE transcript_segments.id = ?1
            "#,
            params![segment_id],
            |row| {
                Ok(TranscriptSegmentDto {
                    id: row.get(0)?,
                    audio_segment_id: row.get(1)?,
                    speaker_id: row.get(2)?,
                    speaker_label: row.get(3)?,
                    start_ms: row.get(4)?,
                    end_ms: row.get(5)?,
                    text: row.get(6)?,
                    confidence: row.get(7)?,
                    provider: row.get(8)?,
                    review_status: row.get(9)?,
                    review_reason: row.get(10)?,
                })
            },
        )
        .ok();
    Ok(row)
}

fn query_speakers(connection: &Connection) -> Result<Vec<SpeakerDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT
              speakers.id,
              speakers.label,
              speakers.display_name,
              speakers.color,
              COUNT(transcript_segments.id),
              speakers.corrected
            FROM speakers
            LEFT JOIN transcript_segments ON transcript_segments.speaker_id = speakers.id
            GROUP BY speakers.id, speakers.label, speakers.display_name, speakers.color, speakers.corrected
            ORDER BY speakers.id ASC
            "#,
        )
        .map_err(|error| format!("准备说话人查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(SpeakerDto {
                id: row.get(0)?,
                label: row.get(1)?,
                display_name: row.get(2)?,
                color: row.get(3)?,
                segment_count: row.get(4)?,
                corrected: row.get::<_, i64>(5)? == 1,
            })
        })
        .map_err(|error| format!("查询说话人失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取说话人失败: {error}"))
}

fn query_speaker(connection: &Connection, speaker_id: &str) -> Result<Option<SpeakerDto>, String> {
    let speaker = query_speakers(connection)?
        .into_iter()
        .find(|speaker| speaker.id == speaker_id);
    Ok(speaker)
}

fn query_transcript_jobs(connection: &Connection) -> Result<Vec<TranscriptJobDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, audio_segment_id, status, retry_count, max_retry_count, error_message, provider, model_name
            FROM transcript_jobs
            ORDER BY datetime(created_at) DESC, id DESC
            "#,
        )
        .map_err(|error| format!("准备转写任务查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(TranscriptJobDto {
                id: row.get(0)?,
                audio_segment_id: row.get(1)?,
                status: row.get(2)?,
                retry_count: row.get(3)?,
                max_retry_count: row.get(4)?,
                error_message: row.get(5)?,
                provider: row.get(6)?,
                model_name: row.get(7)?,
            })
        })
        .map_err(|error| format!("查询转写任务失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取转写任务失败: {error}"))
}

fn query_transcript_job(
    connection: &Connection,
    job_id: &str,
) -> Result<Option<TranscriptJobDto>, String> {
    let job = query_transcript_jobs(connection)?
        .into_iter()
        .find(|job| job.id == job_id);
    Ok(job)
}

fn query_local_model_status(connection: &Connection) -> Result<LocalModelStatusDto, String> {
    connection
        .query_row(
            r#"
            SELECT provider, model_name, cache_dir, download_status, download_progress, offline_available, device_recommendation
            FROM local_model_status
            WHERE provider = ?1
            "#,
            params![local_whisperkit::PROVIDER_ID],
            |row| {
                Ok(LocalModelStatusDto {
                    provider: row.get(0)?,
                    model_name: row.get(1)?,
                    cache_dir: row.get(2)?,
                    download_status: row.get(3)?,
                    download_progress: row.get(4)?,
                    offline_available: row.get::<_, i64>(5)? == 1,
                    device_recommendation: row.get(6)?,
                })
            },
        )
        .map_err(|error| format!("读取本地模型状态失败: {error}"))
}
