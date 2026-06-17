use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    current_timestamp_label,
    domain::runtime::{
        ProcessingJobDto, RecoveryTaskDto, RuntimeDashboardDto, RuntimeMetricSummaryDto,
        SegmentTimelineDto, TaskTimelineEventDto,
    },
};

pub(crate) fn record_runtime_metric(
    connection: &Connection,
    audio_segment_id: Option<&str>,
    command_name: &str,
    duration_ms: i64,
    status: &str,
    error_message: &str,
) -> Result<(), String> {
    connection
        .execute(
            r#"
            INSERT INTO runtime_metrics (
              id,
              audio_segment_id,
              command_name,
              started_at,
              duration_ms,
              status,
              error_message
            ) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, ?4, ?5, ?6)
            "#,
            params![
                format!(
                    "metric_{}_{}",
                    command_name.replace(|candidate: char| !candidate.is_ascii_alphanumeric(), "_"),
                    current_timestamp_label()
                ),
                audio_segment_id.unwrap_or(""),
                command_name,
                duration_ms,
                status,
                error_message,
            ],
        )
        .map_err(|error| format!("写入运行指标失败: {error}"))?;
    Ok(())
}

pub(crate) fn query_runtime_dashboard(
    connection: &Connection,
) -> Result<RuntimeDashboardDto, String> {
    Ok(RuntimeDashboardDto {
        recovery_tasks: query_recovery_tasks(connection)?,
        metric_summaries: query_metric_summaries(connection)?,
    })
}

pub(crate) fn retry_processing_job(
    connection: &Connection,
    job_id: &str,
) -> Result<ProcessingJobDto, String> {
    let current = query_processing_job(connection, job_id)?;
    if current.status != "failed" {
        return Err("仅失败处理任务可以重试".to_string());
    }
    if current.retry_count >= current.max_retry_count {
        return Err("处理任务已达到最大重试次数".to_string());
    }

    connection
        .execute(
            r#"
            UPDATE processing_jobs
            SET status = 'pending',
                retry_count = retry_count + 1,
                error_message = '',
                started_at = NULL,
                finished_at = NULL
            WHERE id = ?1
            "#,
            params![job_id],
        )
        .map_err(|error| format!("重试处理任务失败: {error}"))?;

    query_processing_job(connection, job_id)
}

pub(crate) fn query_segment_timeline(
    connection: &Connection,
    audio_segment_id: &str,
) -> Result<SegmentTimelineDto, String> {
    let (file_path, started_at, ended_at, duration_ms, processing_status): (
        String,
        String,
        String,
        i64,
        String,
    ) = connection
        .query_row(
            r#"
            SELECT file_path, started_at, ended_at, duration_ms, processing_status
            FROM audio_segments
            WHERE id = ?1
            "#,
            params![audio_segment_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|error| format!("读取音频片段时间线失败: {error}"))?;

    let mut events = vec![TaskTimelineEventDto {
        id: format!("{audio_segment_id}_audio"),
        stage: "audio".into(),
        title: "音频导入 / 录音切片".into(),
        status: normalize_timeline_status(processing_status.as_str()).into(),
        timestamp: started_at.clone(),
        detail: format!("结束于 {ended_at}，时长 {duration_ms}ms"),
    }];

    append_transcript_job_events(connection, audio_segment_id, &mut events)?;
    append_transcript_segment_events(connection, audio_segment_id, &mut events)?;
    append_semantic_events(connection, audio_segment_id, &mut events)?;
    append_metric_events(connection, audio_segment_id, &mut events)?;

    events.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.stage.cmp(&right.stage))
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(SegmentTimelineDto {
        audio_segment_id: audio_segment_id.into(),
        file_name: Path::new(file_path.as_str())
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("音频片段")
            .into(),
        events,
    })
}

fn query_recovery_tasks(connection: &Connection) -> Result<Vec<RecoveryTaskDto>, String> {
    let mut tasks = query_failed_transcript_jobs(connection)?;
    tasks.extend(query_failed_processing_jobs(connection)?);
    tasks.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.task_id.cmp(&right.task_id))
    });
    Ok(tasks)
}

fn query_failed_transcript_jobs(connection: &Connection) -> Result<Vec<RecoveryTaskDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT
              id,
              audio_segment_id,
              status,
              retry_count,
              max_retry_count,
              IFNULL(error_message, ''),
              provider,
              model_name,
              IFNULL(updated_at, created_at)
            FROM transcript_jobs
            WHERE status = 'failed'
            ORDER BY datetime(IFNULL(updated_at, created_at)) DESC, id DESC
            "#,
        )
        .map_err(|error| format!("准备失败转写任务查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let task_id: String = row.get(0)?;
            let audio_segment_id: String = row.get(1)?;
            Ok(RecoveryTaskDto {
                task_id,
                task_type: "transcript_job".into(),
                target_id: audio_segment_id.clone(),
                audio_segment_id,
                status: row.get(2)?,
                retry_count: row.get(3)?,
                max_retry_count: row.get(4)?,
                error_message: row.get(5)?,
                provider: row.get(6)?,
                model_name: row.get(7)?,
                retry_command: "retry_transcript_job".into(),
                updated_at: row.get(8)?,
            })
        })
        .map_err(|error| format!("查询失败转写任务失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取失败转写任务失败: {error}"))
}

fn query_failed_processing_jobs(connection: &Connection) -> Result<Vec<RecoveryTaskDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT
              id,
              job_type,
              target_id,
              status,
              retry_count,
              max_retry_count,
              IFNULL(error_message, ''),
              IFNULL(finished_at, created_at)
            FROM processing_jobs
            WHERE status = 'failed'
            ORDER BY datetime(IFNULL(finished_at, created_at)) DESC, id DESC
            "#,
        )
        .map_err(|error| format!("准备失败处理任务查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let target_id: String = row.get(2)?;
            Ok(RecoveryTaskDto {
                task_id: row.get(0)?,
                task_type: row.get(1)?,
                target_id: target_id.clone(),
                audio_segment_id: target_id,
                status: row.get(3)?,
                retry_count: row.get(4)?,
                max_retry_count: row.get(5)?,
                error_message: row.get(6)?,
                provider: "internal".into(),
                model_name: "".into(),
                retry_command: "retry_processing_job".into(),
                updated_at: row.get(7)?,
            })
        })
        .map_err(|error| format!("查询失败处理任务失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取失败处理任务失败: {error}"))
}

fn query_metric_summaries(connection: &Connection) -> Result<Vec<RuntimeMetricSummaryDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT command_name, duration_ms, status, IFNULL(error_message, '')
            FROM runtime_metrics
            WHERE datetime(started_at) >= datetime('now', '-7 days')
            ORDER BY command_name ASC, datetime(started_at) DESC, id DESC
            "#,
        )
        .map_err(|error| format!("准备运行指标查询失败: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("查询运行指标失败: {error}"))?;

    let mut grouped: Vec<(String, Vec<(i64, String, String)>)> = Vec::new();
    for row in rows {
        let (command_name, duration_ms, status, error_message) =
            row.map_err(|error| format!("读取运行指标失败: {error}"))?;
        if let Some((_, values)) = grouped
            .iter_mut()
            .find(|(candidate, _)| candidate == &command_name)
        {
            values.push((duration_ms, status, error_message));
        } else {
            grouped.push((command_name, vec![(duration_ms, status, error_message)]));
        }
    }

    let mut summaries = Vec::new();
    for (command_name, rows) in grouped {
        let mut durations = rows.iter().map(|row| row.0).collect::<Vec<_>>();
        durations.sort_unstable();
        let total_count = rows.len() as i64;
        let success_count = rows.iter().filter(|row| row.1 == "succeeded").count() as i64;
        let failed_count = rows.iter().filter(|row| row.1 == "failed").count() as i64;
        let latest_status = rows
            .first()
            .map(|row| row.1.clone())
            .unwrap_or_else(|| "unknown".into());
        let latest_error_message = rows.first().map(|row| row.2.clone()).unwrap_or_default();
        summaries.push(RuntimeMetricSummaryDto {
            command_name,
            total_count,
            success_count,
            failed_count,
            p50_duration_ms: percentile(&durations, 50),
            p95_duration_ms: percentile(&durations, 95),
            latest_status,
            latest_error_message,
        });
    }

    Ok(summaries)
}

fn query_processing_job(connection: &Connection, job_id: &str) -> Result<ProcessingJobDto, String> {
    connection
        .query_row(
            r#"
            SELECT
              id,
              job_type,
              target_id,
              status,
              retry_count,
              max_retry_count,
              IFNULL(error_message, '')
            FROM processing_jobs
            WHERE id = ?1
            "#,
            params![job_id],
            |row| {
                Ok(ProcessingJobDto {
                    id: row.get(0)?,
                    job_type: row.get(1)?,
                    target_id: row.get(2)?,
                    status: row.get(3)?,
                    retry_count: row.get(4)?,
                    max_retry_count: row.get(5)?,
                    error_message: row.get(6)?,
                })
            },
        )
        .map_err(|error| format!("读取处理任务失败: {error}"))
}

fn append_transcript_job_events(
    connection: &Connection,
    audio_segment_id: &str,
    events: &mut Vec<TaskTimelineEventDto>,
) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, status, retry_count, max_retry_count, IFNULL(error_message, ''), provider, model_name, IFNULL(finished_at, IFNULL(started_at, created_at))
            FROM transcript_jobs
            WHERE audio_segment_id = ?1
            ORDER BY datetime(IFNULL(finished_at, IFNULL(started_at, created_at))) ASC, id ASC
            "#,
        )
        .map_err(|error| format!("准备转写任务时间线查询失败: {error}"))?;
    let rows = statement
        .query_map(params![audio_segment_id], |row| {
            Ok(TaskTimelineEventDto {
                id: row.get(0)?,
                stage: "transcription".into(),
                title: "转写任务".into(),
                status: row.get(1)?,
                timestamp: row.get(7)?,
                detail: format!(
                    "{} / {} · retry {}/{}{}",
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    optional_error_suffix(row.get::<_, String>(4)?.as_str())
                ),
            })
        })
        .map_err(|error| format!("查询转写任务时间线失败: {error}"))?;
    events.extend(
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("读取转写任务时间线失败: {error}"))?,
    );
    Ok(())
}

fn append_transcript_segment_events(
    connection: &Connection,
    audio_segment_id: &str,
    events: &mut Vec<TaskTimelineEventDto>,
) -> Result<(), String> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM transcript_segments WHERE audio_segment_id = ?1",
            params![audio_segment_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("统计转写片段失败: {error}"))?;
    if count == 0 {
        return Ok(());
    }
    let created_at: String = connection
        .query_row(
            "SELECT MIN(created_at) FROM transcript_segments WHERE audio_segment_id = ?1",
            params![audio_segment_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取转写片段时间失败: {error}"))?;
    events.push(TaskTimelineEventDto {
        id: format!("{audio_segment_id}_transcript_segments"),
        stage: "transcription".into(),
        title: "时间轴转写片段".into(),
        status: "succeeded".into(),
        timestamp: created_at,
        detail: format!("{count} 条片段已写入 transcript_segments"),
    });
    Ok(())
}

fn append_semantic_events(
    connection: &Connection,
    audio_segment_id: &str,
    events: &mut Vec<TaskTimelineEventDto>,
) -> Result<(), String> {
    let Some(session_id) = connection
        .query_row(
            r#"
            SELECT conversation_session_id
            FROM transcript_segments
            WHERE audio_segment_id = ?1 AND conversation_session_id IS NOT NULL
            ORDER BY datetime(created_at) ASC
            LIMIT 1
            "#,
            params![audio_segment_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("读取时间线会话失败: {error}"))?
    else {
        return Ok(());
    };

    let mut statement = connection
        .prepare(
            r#"
            SELECT id, artifact_type, status, provider, model_name, IFNULL(error_message, ''), updated_at
            FROM semantic_artifacts
            WHERE session_id = ?1
            ORDER BY datetime(updated_at) ASC, id ASC
            "#,
        )
        .map_err(|error| format!("准备语义产物时间线查询失败: {error}"))?;
    let rows = statement
        .query_map(params![session_id], |row| {
            Ok(TaskTimelineEventDto {
                id: row.get(0)?,
                stage: "semantic".into(),
                title: format!("M3 {}", row.get::<_, String>(1)?),
                status: row.get(2)?,
                timestamp: row.get(6)?,
                detail: format!(
                    "{} / {}{}",
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    optional_error_suffix(row.get::<_, String>(5)?.as_str())
                ),
            })
        })
        .map_err(|error| format!("查询语义产物时间线失败: {error}"))?;
    events.extend(
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("读取语义产物时间线失败: {error}"))?,
    );
    Ok(())
}

fn append_metric_events(
    connection: &Connection,
    audio_segment_id: &str,
    events: &mut Vec<TaskTimelineEventDto>,
) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, command_name, duration_ms, status, IFNULL(error_message, ''), started_at
            FROM runtime_metrics
            WHERE audio_segment_id = ?1
            ORDER BY datetime(started_at) ASC, id ASC
            "#,
        )
        .map_err(|error| format!("准备运行指标时间线查询失败: {error}"))?;
    let rows = statement
        .query_map(params![audio_segment_id], |row| {
            Ok(TaskTimelineEventDto {
                id: row.get(0)?,
                stage: "metric".into(),
                title: row.get(1)?,
                status: row.get(3)?,
                timestamp: row.get(5)?,
                detail: format!(
                    "{}ms{}",
                    row.get::<_, i64>(2)?,
                    optional_error_suffix(row.get::<_, String>(4)?.as_str())
                ),
            })
        })
        .map_err(|error| format!("查询运行指标时间线失败: {error}"))?;
    events.extend(
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("读取运行指标时间线失败: {error}"))?,
    );
    Ok(())
}

fn percentile(sorted_values: &[i64], percentile: usize) -> i64 {
    if sorted_values.is_empty() {
        return 0;
    }
    let index = ((sorted_values.len() - 1) * percentile) / 100;
    sorted_values[index]
}

fn optional_error_suffix(error_message: &str) -> String {
    if error_message.trim().is_empty() {
        String::new()
    } else {
        format!(" · {}", error_message.trim())
    }
}

fn normalize_timeline_status(status: &str) -> &'static str {
    match status {
        "transcribed" | "success" => "succeeded",
        "pending" => "pending",
        "failed" => "failed",
        "skipped" => "skipped",
        _ => "succeeded",
    }
}
