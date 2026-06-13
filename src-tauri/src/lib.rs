use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

mod app;
mod commands;
mod domain;
mod infra;
mod jobs;
mod providers;

use app::settings_service;
use infra::sqlite::{initialize_database, open_connection};

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    recordings_dir: PathBuf,
    recorder: Arc<Mutex<Option<RecordingController>>>,
}

struct RecordingController {
    stop_tx: mpsc::Sender<RecorderControl>,
    join_handle: JoinHandle<Result<RecordingResult, String>>,
}

enum RecorderControl {
    Stop,
}

enum WriterMessage {
    Samples(Vec<i16>),
    Stop,
}

struct RecordingResult {
    file_path: PathBuf,
    sample_rate: u32,
    channels: u16,
    started_at_label: String,
    duration_ms: i64,
    trace_id: String,
    summary: RecordingSummary,
}

struct RecordingSummary {
    sample_count: u64,
    total_energy: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopContext {
    runtime: String,
    platform: String,
    recorder_status: String,
    storage_status: String,
    models_status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsDto {
    record_enabled: bool,
    language: String,
    chunk_seconds: i64,
    idle_trigger_seconds: i64,
    provider_mode: String,
    asr_provider_type: String,
    speaker_provider_type: String,
    todo_provider_type: String,
    semantic_provider_type: String,
    embedding_provider_type: String,
    export_provider_type: String,
    asr_submit_url: String,
    asr_query_url: String,
    asr_resource_id: String,
    asr_model_name: String,
    asr_api_key_masked: String,
    semantic_base_url: String,
    semantic_model_name: String,
    semantic_api_key_masked: String,
    allow_cloud_fallback: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TodoDto {
    id: String,
    title: String,
    note: String,
    status: String,
    created_at: String,
    conversation_session_id: String,
    source_text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SessionDto {
    id: String,
    merged_text: String,
    started_at: String,
    ended_at: String,
    trigger_reason: String,
    extraction_status: String,
    extraction_provider_used: String,
    extraction_fallback_used: bool,
    extraction_fallback_reason: String,
    transcript_count: i64,
    related_todo_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatusDto {
    runtime_label: String,
    current_session_status: String,
    last_slice_at: String,
    last_extraction_at: String,
    last_extraction_summary: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapData {
    settings: SettingsDto,
    todos: Vec<TodoDto>,
    sessions: Vec<SessionDto>,
    runtime: RuntimeStatusDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingActionResult {
    message: String,
    runtime: RuntimeStatusDto,
    latest_session: Option<SessionDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessingActionResult {
    message: String,
    runtime: RuntimeStatusDto,
    latest_session: Option<SessionDto>,
    todos: Vec<TodoDto>,
    sessions: Vec<SessionDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelTestRequest {
    provider: String,
    settings: SettingsDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelTestResult {
    provider: String,
    success: bool,
    status_code: u16,
    message: String,
    response_excerpt: String,
}

#[derive(Debug)]
struct AudioSegmentRecord {
    id: String,
    file_path: String,
    trace_id: String,
}

#[derive(Debug)]
struct TranscriptRecord {
    id: String,
    text: String,
    trace_id: String,
}

fn current_timestamp_label() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    millis.to_string()
}

const HTTP_REQUEST_TIMEOUT_SECONDS: u64 = 30;
const HTTP_MAX_RETRY_ATTEMPTS: usize = 3;
const MANUAL_FLUSH_COOLDOWN_SECONDS: i64 = 10;
const DEFAULT_ASR_PROVIDER_TYPE: &str = "local_whisperkit";
const DEFAULT_SPEAKER_PROVIDER_TYPE: &str = "local_speakerkit";
const DEFAULT_SEMANTIC_PROVIDER_TYPE: &str = "minimax_m3";
const DEFAULT_EMBEDDING_PROVIDER_TYPE: &str = "reserved";
const DEFAULT_EXPORT_PROVIDER_TYPE: &str = "local_file";
const DEFAULT_TODO_PROVIDER_TYPE: &str = "semantic_m3";
const DEFAULT_SEMANTIC_BASE_URL: &str = "https://api.minimax.io/v1/responses";
const DEFAULT_SEMANTIC_MODEL_NAME: &str = "MiniMax-M3";
fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| format!("创建 HTTP 客户端失败: {error}"))
}

fn clip_text(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

fn sleep_before_retry(attempt: usize) {
    let delay_seconds = attempt as u64;
    thread::sleep(std::time::Duration::from_secs(delay_seconds));
}

fn should_retry_http_status(status: u16) -> bool {
    status == 429 || status >= 500
}

fn normalize_asr_provider_type(provider_type: &str) -> String {
    match provider_type.trim() {
        "" | "local" | "local_whisperkit" => DEFAULT_ASR_PROVIDER_TYPE.to_string(),
        "cloud" | "cloud_volc" => "cloud_volc".to_string(),
        other => other.to_string(),
    }
}

fn is_local_asr_provider(provider_type: &str) -> bool {
    matches!(
        normalize_asr_provider_type(provider_type).as_str(),
        DEFAULT_ASR_PROVIDER_TYPE
    )
}

fn normalize_todo_provider_type(provider_type: &str) -> String {
    let _ = provider_type;
    DEFAULT_TODO_PROVIDER_TYPE.to_string()
}

fn is_placeholder_session_text(text: &str) -> bool {
    let normalized = text.trim();
    normalized.is_empty()
        || normalized.contains("当前用于验证录音链路骨架")
        || normalized.contains("手动刷新会话于")
}

fn ensure_manual_flush_allowed(connection: &Connection) -> Result<(), String> {
    let latest_gap_seconds = connection
        .query_row(
            r#"
            SELECT CAST((julianday('now') - julianday(created_at)) * 86400 AS INTEGER)
            FROM conversation_sessions
            WHERE trigger_reason = 'manual'
            ORDER BY datetime(created_at) DESC
            LIMIT 1
            "#,
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok();

    if let Some(gap_seconds) = latest_gap_seconds {
        if gap_seconds < MANUAL_FLUSH_COOLDOWN_SECONDS {
            return Err(format!(
                "手动刷新过于频繁，请在 {} 秒后再试",
                MANUAL_FLUSH_COOLDOWN_SECONDS - gap_seconds
            ));
        }
    }

    Ok(())
}

fn query_todos(connection: &Connection) -> Result<Vec<TodoDto>, String> {
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

fn query_sessions(connection: &Connection) -> Result<Vec<SessionDto>, String> {
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

fn latest_session(connection: &Connection) -> Result<Option<SessionDto>, String> {
    Ok(query_sessions(connection)?.into_iter().next())
}

fn query_runtime_status(connection: &Connection) -> Result<RuntimeStatusDto, String> {
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

fn insert_processing_job(
    connection: &Connection,
    job_type: &str,
    target_id: &str,
    trace_id: &str,
) -> Result<(), String> {
    connection
        .execute(
            r#"
      INSERT INTO processing_jobs (
        id,
        job_type,
        target_id,
        status,
        retry_count,
        max_retry_count,
        trace_id,
        created_at
      ) VALUES (?1, ?2, ?3, 'pending', 0, 3, ?4, CURRENT_TIMESTAMP)
      "#,
            params![
                format!("job_{}_{}", job_type, current_timestamp_label()),
                job_type,
                target_id,
                trace_id
            ],
        )
        .map_err(|error| format!("写入处理任务失败: {error}"))?;
    Ok(())
}

fn update_processing_job(
    connection: &Connection,
    job_id: &str,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), String> {
    connection
        .execute(
            r#"
            UPDATE processing_jobs
            SET
              status = ?1,
              error_message = ?2,
              started_at = CASE
                WHEN ?1 = 'running' AND started_at IS NULL THEN CURRENT_TIMESTAMP
                ELSE started_at
              END,
              finished_at = CASE
                WHEN ?1 IN ('success', 'failed') THEN CURRENT_TIMESTAMP
                ELSE finished_at
              END
            WHERE id = ?3
            "#,
            params![status, error_message, job_id],
        )
        .map_err(|error| format!("更新处理任务状态失败: {error}"))?;
    Ok(())
}

fn maybe_create_idle_session(connection: &Connection) -> Result<Option<SessionDto>, String> {
    let settings = settings_service::load_settings(connection)?;
    let latest_segment: Option<(String, String)> = connection
        .query_row(
            r#"
      SELECT started_at, ended_at
      FROM audio_segments
      ORDER BY datetime(created_at) DESC
      LIMIT 1
      "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    let Some((started_at, ended_at)) = latest_segment else {
        return Ok(None);
    };

    let session_id = format!("session_idle_{}", current_timestamp_label());
    let trace_id = format!("trace_idle_{}", current_timestamp_label());
    let merged_text = format!(
        "检测到连续 {} 秒无有效录音，已基于最近录音片段创建待提取会话。",
        settings.idle_trigger_seconds
    );

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
      ) VALUES (?1, ?2, ?3, ?4, ?5, 'idle_timeout', 0, 'pending', 'pending', 0, '', ?6, CURRENT_TIMESTAMP)
      "#,
            params![
                session_id.as_str(),
                merged_text.as_str(),
                started_at,
                ended_at,
                settings.idle_trigger_seconds,
                trace_id.as_str()
            ],
        )
        .map_err(|error| format!("创建空闲触发会话失败: {error}"))?;

    insert_processing_job(connection, "todo_extraction", &session_id, &trace_id)?;
    latest_session(connection)
}

fn insert_audio_segment(
    connection: &Connection,
    file_path: &PathBuf,
    started_at_label: &str,
    duration_ms: i64,
    sample_rate: u32,
    channels: u16,
    total_energy: u64,
    sample_count: u64,
    trace_id: &str,
) -> Result<(), String> {
    let has_effective_voice = if sample_count == 0 {
        0
    } else {
        let avg_energy = total_energy as f64 / sample_count as f64;
        if avg_energy > 200.0 {
            1
        } else {
            0
        }
    };
    let voice_energy_score = if sample_count == 0 {
        0.0
    } else {
        total_energy as f64 / sample_count as f64
    };
    let segment_id = format!("audio_real_{}", current_timestamp_label());
    let processing_status = if has_effective_voice == 1 {
        "pending"
    } else {
        "skipped"
    };

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
      ) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP)
      "#,
            params![
                segment_id,
                file_path.to_string_lossy().to_string(),
                started_at_label,
                duration_ms,
                i64::from(sample_rate),
                i64::from(channels),
                has_effective_voice,
                voice_energy_score,
                processing_status,
                trace_id
            ],
        )
        .map_err(|error| format!("写入真实录音切片失败: {error}"))?;

    if has_effective_voice == 1 {
        insert_processing_job(connection, "transcription", &segment_id, trace_id)?;
    }
    Ok(())
}

fn convert_u16_to_i16(input: &[u16]) -> Vec<i16> {
    input
        .iter()
        .map(|value| (*value as i32 - 32768) as i16)
        .collect::<Vec<_>>()
}

fn convert_f32_to_i16(input: &[f32]) -> Vec<i16> {
    input
        .iter()
        .map(|value| (value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect::<Vec<_>>()
}

fn spawn_recording_controller(recordings_dir: PathBuf) -> Result<RecordingController, String> {
    fs::create_dir_all(&recordings_dir).map_err(|error| format!("创建录音目录失败: {error}"))?;

    let (stop_tx, stop_rx) = mpsc::channel::<RecorderControl>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    let join_handle = thread::spawn(move || -> Result<RecordingResult, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "未找到默认输入设备，请检查麦克风权限".to_string())?;
        let supported_config = device
            .default_input_config()
            .map_err(|error| format!("读取输入设备配置失败: {error}"))?;

        let sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels();
        let config: cpal::StreamConfig = supported_config.clone().into();
        let started_at_label = current_timestamp_label();
        let trace_id = format!("trace_real_{}", current_timestamp_label());
        let file_path = recordings_dir.join(format!("recording_{started_at_label}.wav"));
        let started_at_instant = Instant::now();

        let (writer_tx, writer_rx) = mpsc::channel::<WriterMessage>();
        let writer_output_path = file_path.clone();
        let writer_handle = thread::spawn(move || -> Result<RecordingSummary, String> {
            let spec = hound::WavSpec {
                channels,
                sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };

            let mut writer = hound::WavWriter::create(&writer_output_path, spec)
                .map_err(|error| format!("创建录音 WAV 文件失败: {error}"))?;
            let mut sample_count = 0_u64;
            let mut total_energy = 0_u64;

            loop {
                match writer_rx.recv() {
                    Ok(WriterMessage::Samples(samples)) => {
                        for sample in samples {
                            total_energy += sample.unsigned_abs() as u64;
                            sample_count += 1;
                            writer
                                .write_sample(sample)
                                .map_err(|error| format!("写入录音样本失败: {error}"))?;
                        }
                    }
                    Ok(WriterMessage::Stop) | Err(_) => {
                        writer
                            .finalize()
                            .map_err(|error| format!("结束录音文件失败: {error}"))?;
                        break;
                    }
                }
            }

            Ok(RecordingSummary {
                sample_count,
                total_energy,
            })
        });

        let error_callback = |error| {
            log::error!("录音流错误: {error}");
        };

        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::I16 => {
                let callback_tx = writer_tx.clone();
                device
                    .build_input_stream(
                        &config,
                        move |data: &[i16], _| {
                            let _ = callback_tx.send(WriterMessage::Samples(data.to_vec()));
                        },
                        error_callback,
                        None,
                    )
                    .map_err(|error| format!("构建 i16 输入流失败: {error}"))?
            }
            cpal::SampleFormat::U16 => {
                let callback_tx = writer_tx.clone();
                device
                    .build_input_stream(
                        &config,
                        move |data: &[u16], _| {
                            let _ =
                                callback_tx.send(WriterMessage::Samples(convert_u16_to_i16(data)));
                        },
                        error_callback,
                        None,
                    )
                    .map_err(|error| format!("构建 u16 输入流失败: {error}"))?
            }
            cpal::SampleFormat::F32 => {
                let callback_tx = writer_tx.clone();
                device
                    .build_input_stream(
                        &config,
                        move |data: &[f32], _| {
                            let _ =
                                callback_tx.send(WriterMessage::Samples(convert_f32_to_i16(data)));
                        },
                        error_callback,
                        None,
                    )
                    .map_err(|error| format!("构建 f32 输入流失败: {error}"))?
            }
            other => {
                return Err(format!("当前不支持的输入采样格式: {other:?}"));
            }
        };

        stream
            .play()
            .map_err(|error| format!("启动录音流失败: {error}"))?;
        let _ = ready_tx.send(Ok(()));

        match stop_rx.recv() {
            Ok(RecorderControl::Stop) | Err(_) => {}
        }

        drop(stream);
        let _ = writer_tx.send(WriterMessage::Stop);
        let summary = writer_handle
            .join()
            .map_err(|_| "录音写入线程异常退出".to_string())??;

        Ok(RecordingResult {
            file_path,
            sample_rate,
            channels,
            started_at_label,
            duration_ms: started_at_instant.elapsed().as_millis() as i64,
            trace_id,
            summary,
        })
    });

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(RecordingController {
            stop_tx,
            join_handle,
        }),
        Ok(Err(error)) => {
            let _ = join_handle.join();
            Err(error)
        }
        Err(_) => {
            let _ = join_handle.join();
            Err("录音线程初始化失败".to_string())
        }
    }
}

fn is_recording(state: &AppState) -> Result<bool, String> {
    state
        .recorder
        .lock()
        .map(|guard| guard.is_some())
        .map_err(|_| "录音状态锁定失败".to_string())
}

fn transcribe_audio_file(settings: &SettingsDto, file_path: &str) -> Result<String, String> {
    if is_local_asr_provider(&settings.asr_provider_type) {
        let message = "本地 ASR 尚未接入，无法执行纯本地语音转写";
        if !settings.allow_cloud_fallback {
            return Err(message.to_string());
        }
        log::warn!("{message}，将按配置使用云端 ASR 兜底");
    }

    if settings.asr_api_key_masked.trim().is_empty()
        || settings.asr_resource_id.trim().is_empty()
        || settings.asr_model_name.trim().is_empty()
    {
        return Err("云端 ASR 配置不完整，无法执行转写兜底".to_string());
    }

    let file_bytes = fs::read(file_path).map_err(|error| format!("读取录音文件失败: {error}"))?;
    let audio_data = {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        STANDARD.encode(file_bytes)
    };
    let client = build_http_client()?;
    let request_id = format!("transcribe-{}", current_timestamp_label());
    let endpoint = "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash";

    let try_resources = [
        settings.asr_resource_id.trim().to_string(),
        "volc.bigasr.auc_turbo".to_string(),
    ];

    let mut last_error = String::new();
    for resource_id in try_resources {
        if resource_id.is_empty() {
            continue;
        }

        for attempt in 1..=HTTP_MAX_RETRY_ATTEMPTS {
            let response = client
                .post(endpoint)
                .header("Content-Type", "application/json")
                .header("X-Api-Key", settings.asr_api_key_masked.trim())
                .header("X-Api-Resource-Id", resource_id.as_str())
                .header("X-Api-Request-Id", &request_id)
                .header("X-Api-Sequence", "-1")
                .json(&serde_json::json!({
                    "user": { "uid": "smart-todo-local-recording" },
                    "audio": { "data": audio_data },
                    "request": {
                        "model_name": "bigmodel"
                    }
                }))
                .send();

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    last_error = format!("调用 ASR 极速版失败: {error}");
                    if attempt < HTTP_MAX_RETRY_ATTEMPTS {
                        sleep_before_retry(attempt);
                        continue;
                    }
                    break;
                }
            };

            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "读取 ASR 响应失败".to_string());
            if status / 100 != 2 {
                last_error = format!("HTTP {status}: {}", clip_text(&body, 300));
                if should_retry_http_status(status) && attempt < HTTP_MAX_RETRY_ATTEMPTS {
                    sleep_before_retry(attempt);
                    continue;
                }
                break;
            }

            let value: serde_json::Value = serde_json::from_str(&body)
                .map_err(|error| format!("解析 ASR 响应失败: {error}"))?;
            if let Some(text) = value
                .get("result")
                .and_then(|entry| entry.get("text"))
                .and_then(|entry| entry.as_str())
            {
                return Ok(text.to_string());
            }
            return Err("ASR 返回成功，但未找到 result.text".to_string());
        }
    }

    Err(format!("ASR 转写失败: {last_error}"))
}

fn create_session_from_transcript(
    connection: &Connection,
    transcript: &TranscriptRecord,
    settings: &SettingsDto,
) -> Result<String, String> {
    let session_id = format!("session_transcript_{}", current_timestamp_label());
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
            ) VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?3, 'forced_flush', 1, 'pending', 'pending', 0, '', ?4, CURRENT_TIMESTAMP)
            "#,
            params![
                session_id.as_str(),
                transcript.text.as_str(),
                settings.idle_trigger_seconds,
                transcript.trace_id.as_str()
            ],
        )
        .map_err(|error| format!("创建转写会话失败: {error}"))?;

    connection
        .execute(
            "UPDATE transcript_segments SET conversation_session_id = ?1 WHERE id = ?2",
            params![session_id.as_str(), transcript.id.as_str()],
        )
        .map_err(|error| format!("绑定转写与会话关系失败: {error}"))?;

    insert_processing_job(
        connection,
        "todo_extraction",
        &session_id,
        &transcript.trace_id,
    )?;
    Ok(session_id)
}

fn process_pending_jobs_internal(connection: &Connection) -> Result<String, String> {
    let settings = settings_service::load_settings(connection)?;
    let mut summary = Vec::new();

    let mut transcription_statement = connection
        .prepare(
            r#"
            SELECT id, target_id
            FROM processing_jobs
            WHERE job_type = 'transcription' AND status = 'pending'
            ORDER BY datetime(created_at) ASC
            "#,
        )
        .map_err(|error| format!("准备转写任务查询失败: {error}"))?;

    let transcription_jobs = transcription_statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("查询转写任务失败: {error}"))?;

    for job in transcription_jobs {
        let (job_id, audio_segment_id) =
            job.map_err(|error| format!("读取转写任务失败: {error}"))?;
        update_processing_job(connection, &job_id, "running", None)?;

        let result = (|| -> Result<String, String> {
            let record = connection
                .query_row(
                    "SELECT id, file_path, trace_id FROM audio_segments WHERE id = ?1",
                    params![audio_segment_id.as_str()],
                    |row| {
                        Ok(AudioSegmentRecord {
                            id: row.get(0)?,
                            file_path: row.get(1)?,
                            trace_id: row.get(2)?,
                        })
                    },
                )
                .map_err(|error| format!("读取音频切片失败: {error}"))?;

            let text = transcribe_audio_file(&settings, &record.file_path)?;
            let normalized_text = text.trim().to_string();
            if normalized_text.is_empty() {
                connection
                    .execute(
                        "UPDATE audio_segments SET processing_status = 'skipped' WHERE id = ?1",
                        params![record.id.as_str()],
                    )
                    .map_err(|error| format!("更新空白转写状态失败: {error}"))?;
                return Ok("empty_transcript".to_string());
            }
            let transcript_id = format!("transcript_{}", current_timestamp_label());
            connection
                .execute(
                    r#"
                    INSERT INTO transcript_segments (
                      id,
                      audio_segment_id,
                      text,
                      language,
                      status,
                      provider_model_name,
                      trace_id,
                      created_at
                    ) VALUES (?1, ?2, ?3, ?4, 'success', ?5, ?6, CURRENT_TIMESTAMP)
                    "#,
                    params![
                        transcript_id.as_str(),
                        record.id.as_str(),
                        normalized_text.as_str(),
                        settings.language.as_str(),
                        settings.asr_model_name.as_str(),
                        record.trace_id.as_str()
                    ],
                )
                .map_err(|error| format!("写入转写结果失败: {error}"))?;

            connection
                .execute(
                    "UPDATE audio_segments SET processing_status = 'transcribed' WHERE id = ?1",
                    params![record.id.as_str()],
                )
                .map_err(|error| format!("更新音频切片状态失败: {error}"))?;

            let transcript = TranscriptRecord {
                id: transcript_id,
                text: normalized_text,
                trace_id: record.trace_id.clone(),
            };
            let session_id = create_session_from_transcript(connection, &transcript, &settings)?;
            Ok(session_id)
        })();

        match result {
            Ok(session_id) => {
                update_processing_job(connection, &job_id, "success", None)?;
                if session_id == "empty_transcript" {
                    summary.push(format!(
                        "音频切片 {audio_segment_id} 未识别到有效文本，已跳过"
                    ));
                } else {
                    summary.push(format!("已完成转写并生成会话 {session_id}"));
                }
            }
            Err(error) => {
                connection
                    .execute(
                        "UPDATE audio_segments SET processing_status = 'failed' WHERE id = ?1",
                        params![audio_segment_id.as_str()],
                    )
                    .map_err(|db_error| format!("标记转写失败状态失败: {db_error}"))?;
                update_processing_job(connection, &job_id, "failed", Some(error.as_str()))?;
                summary.push(format!("转写失败: {}", clip_text(&error, 80)));
            }
        }
    }

    let mut extraction_statement = connection
        .prepare(
            r#"
            SELECT id, target_id
            FROM processing_jobs
            WHERE job_type = 'todo_extraction' AND status = 'pending'
            ORDER BY datetime(created_at) ASC
            "#,
        )
        .map_err(|error| format!("准备 Todo 提取任务查询失败: {error}"))?;

    let extraction_jobs = extraction_statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("查询 Todo 提取任务失败: {error}"))?;

    for job in extraction_jobs {
        let (job_id, session_id) =
            job.map_err(|error| format!("读取 Todo 提取任务失败: {error}"))?;
        update_processing_job(connection, &job_id, "running", None)?;

        match jobs::todo_extraction::generate_for_session(connection, &settings, &session_id) {
            Ok(count) => {
                update_processing_job(connection, &job_id, "success", None)?;
                summary.push(format!("已从会话 {session_id} 生成 {count} 条 Todo"));
            }
            Err(error) => {
                connection
                    .execute(
                        "UPDATE conversation_sessions SET extraction_status = 'failed', extraction_provider_used = ?1, extraction_fallback_used = ?2, extraction_fallback_reason = ?3 WHERE id = ?4",
                        params![
                            normalize_todo_provider_type(&settings.todo_provider_type).as_str(),
                            0,
                            clip_text(&error, 200),
                            session_id.as_str()
                        ],
                    )
                    .map_err(|db_error| format!("标记会话提取失败状态失败: {db_error}"))?;
                update_processing_job(connection, &job_id, "failed", Some(error.as_str()))?;
                summary.push(format!("Todo 提取失败: {}", clip_text(&error, 80)));
            }
        }
    }

    if summary.is_empty() {
        Ok("暂无待处理任务".into())
    } else {
        Ok(summary.join("；"))
    }
}

pub fn process_pending_jobs_once_for_cli(db_path: &str) -> Result<String, String> {
    let db_path = PathBuf::from(db_path);
    initialize_database(&db_path)?;
    let connection = open_connection(&db_path)?;
    process_pending_jobs_internal(&connection)
}

fn test_asr_provider(settings: &SettingsDto) -> Result<ModelTestResult, String> {
    let local_asr_unavailable = is_local_asr_provider(&settings.asr_provider_type);
    if local_asr_unavailable && !settings.allow_cloud_fallback {
        return Ok(ModelTestResult {
            provider: "asr".into(),
            success: false,
            status_code: 503,
            message: "当前为纯本地 ASR 模式，但本地 ASR 尚未接入".into(),
            response_excerpt: "关闭云端兜底后，语音转写不会调用云端服务。".into(),
        });
    }

    if settings.asr_submit_url.trim().is_empty()
        || settings.asr_query_url.trim().is_empty()
        || settings.asr_resource_id.trim().is_empty()
        || settings.asr_model_name.trim().is_empty()
        || settings.asr_api_key_masked.trim().is_empty()
    {
        return Ok(ModelTestResult {
            provider: "asr".into(),
            success: false,
            status_code: 0,
            message: "ASR 模型配置不完整".into(),
            response_excerpt: "".into(),
        });
    }

    let client = build_http_client()?;
    let request_id = format!("codex-{}", current_timestamp_label());
    let submit_response = client
        .post(settings.asr_submit_url.trim())
        .header("Content-Type", "application/json")
        .header("X-Api-Key", settings.asr_api_key_masked.trim())
        .header("X-Api-Resource-Id", settings.asr_resource_id.trim())
        .header("X-Api-Request-Id", &request_id)
        .header("X-Api-Sequence", "-1")
        .json(&serde_json::json!({
            "user": {
                "uid": "smart-todo-connectivity-test"
            },
            "audio": {
                "url": "https://lf3-static.bytednsdoc.com/obj/eden-cn/lm_hz_ihsph/ljhwZthlaukjlkulzlp/console/bigtts/zh_female_cancan_mars_bigtts.mp3",
                "format": "mp3",
                "codec": "raw",
                "rate": 16000,
                "bits": 16,
                "channel": 1
            },
            "request": {
                "model_name": settings.asr_model_name.trim(),
                "enable_itn": true,
                "enable_punc": false,
                "enable_ddc": false,
                "enable_speaker_info": false,
                "enable_channel_split": false,
                "show_utterances": false,
                "vad_segment": false,
                "sensitive_words_filter": ""
            }
        }))
        .send()
        .map_err(|error| format!("ASR 提交请求失败: {error}"))?;

    let submit_status = submit_response.status().as_u16();
    let submit_body = submit_response
        .text()
        .unwrap_or_else(|_| "读取响应正文失败".to_string());

    if submit_status / 100 != 2 {
        return Ok(ModelTestResult {
            provider: "asr".into(),
            success: false,
            status_code: submit_status,
            message: format!("ASR 提交请求失败，HTTP {submit_status}"),
            response_excerpt: clip_text(&submit_body, 400),
        });
    }

    let query_response = client
        .post(settings.asr_query_url.trim())
        .header("Content-Type", "application/json")
        .header("X-Api-Key", settings.asr_api_key_masked.trim())
        .header("X-Api-Resource-Id", settings.asr_resource_id.trim())
        .header("X-Api-Request-Id", &request_id)
        .json(&serde_json::json!({}))
        .send()
        .map_err(|error| format!("ASR 查询请求失败: {error}"))?;

    let status_code = query_response.status().as_u16();
    let body = query_response
        .text()
        .unwrap_or_else(|_| "读取响应正文失败".to_string());
    let success = status_code / 100 == 2;

    Ok(ModelTestResult {
        provider: "asr".into(),
        success,
        status_code,
        message: if success {
            if local_asr_unavailable {
                "本地 ASR 尚未接入，云端兜底 ASR 测试成功".into()
            } else {
                "ASR 提交与查询测试成功".into()
            }
        } else {
            format!("ASR 查询请求失败，HTTP {status_code}")
        },
        response_excerpt: clip_text(&body, 400),
    })
}

#[tauri::command]
fn get_desktop_context(state: tauri::State<'_, AppState>) -> Result<DesktopContext, String> {
    let recording = is_recording(&state)?;
    let provider_count = providers::provider_catalog().len();

    Ok(DesktopContext {
        runtime: "tauri".into(),
        platform: std::env::consts::OS.into(),
        recorder_status: if recording {
            "真实麦克风录音中".into()
        } else {
            "录音已停止，可启动真实麦克风录音".into()
        },
        storage_status: format!(
            "SQLite 已接入 settings / audio_segments / sessions / semantic_artifacts / model_invocations / todos；{} 个 provider 边界已注册",
            provider_count
        ),
        models_status: "Todo 语义入口已固定为 MiniMax M3；旧本地 Todo 路径已移除".into(),
    })
}

#[tauri::command]
fn get_bootstrap_data(state: tauri::State<'_, AppState>) -> Result<BootstrapData, String> {
    let connection = open_connection(&state.db_path)?;
    Ok(BootstrapData {
        settings: settings_service::load_settings(&connection)?,
        todos: query_todos(&connection)?,
        sessions: query_sessions(&connection)?,
        runtime: query_runtime_status(&connection)?,
    })
}

#[tauri::command]
fn toggle_todo_status(
    todo_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<TodoDto, String> {
    let connection = open_connection(&state.db_path)?;

    let current_status: String = connection
        .query_row(
            "SELECT status FROM todos WHERE id = ?1",
            params![todo_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取 Todo 状态失败: {error}"))?;

    let next_status = if current_status == "pending" {
        "completed"
    } else {
        "pending"
    };

    connection
        .execute(
            r#"
      UPDATE todos
      SET
        status = ?1,
        completed_at = CASE WHEN ?1 = 'completed' THEN CURRENT_TIMESTAMP ELSE NULL END,
        updated_at = CURRENT_TIMESTAMP
      WHERE id = ?2
      "#,
            params![next_status, todo_id.as_str()],
        )
        .map_err(|error| format!("更新 Todo 状态失败: {error}"))?;

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
        IFNULL(source_text, '')
      FROM todos
      WHERE id = ?1
      "#,
            params![todo_id.as_str()],
            |row| {
                Ok(TodoDto {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    note: row.get(2)?,
                    status: row.get(3)?,
                    created_at: row.get(4)?,
                    conversation_session_id: row.get(5)?,
                    source_text: row.get(6)?,
                })
            },
        )
        .map_err(|error| format!("读取更新后的 Todo 失败: {error}"))
}

#[tauri::command]
fn flush_current_session(state: tauri::State<'_, AppState>) -> Result<SessionDto, String> {
    let connection = open_connection(&state.db_path)?;
    ensure_manual_flush_allowed(&connection)?;
    let timestamp = current_timestamp_label();
    let session_id = format!("session_manual_{timestamp}");
    let trace_id = format!("trace_manual_{timestamp}");
    let merged_text = "手动刷新会话，当前未绑定真实转写文稿。".to_string();

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
      ) VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', 0, 'pending', 'pending', 0, '', ?3, CURRENT_TIMESTAMP)
      "#,
      params![session_id.as_str(), merged_text.as_str(), trace_id.as_str()],
    )
    .map_err(|error| format!("写入手动会话失败: {error}"))?;
    insert_processing_job(&connection, "todo_extraction", &session_id, &trace_id)?;
    let _ = process_pending_jobs_internal(&connection)?;

    latest_session(&connection)?.ok_or_else(|| "未找到刚创建的会话".to_string())
}

#[tauri::command]
fn process_pending_jobs(
    state: tauri::State<'_, AppState>,
) -> Result<ProcessingActionResult, String> {
    let connection = open_connection(&state.db_path)?;
    let message = process_pending_jobs_internal(&connection)?;
    Ok(ProcessingActionResult {
        message,
        runtime: query_runtime_status(&connection)?,
        latest_session: latest_session(&connection)?,
        todos: query_todos(&connection)?,
        sessions: query_sessions(&connection)?,
    })
}

#[tauri::command]
fn start_recording(state: tauri::State<'_, AppState>) -> Result<RecordingActionResult, String> {
    let mut recorder_guard = state
        .recorder
        .lock()
        .map_err(|_| "录音状态锁定失败".to_string())?;
    if recorder_guard.is_some() {
        let connection = open_connection(&state.db_path)?;
        return Ok(RecordingActionResult {
            message: "录音已在进行中".into(),
            runtime: query_runtime_status(&connection)?,
            latest_session: latest_session(&connection)?,
        });
    }

    let controller = spawn_recording_controller(state.recordings_dir.clone())?;
    let connection = open_connection(&state.db_path)?;
    settings_service::set_record_enabled(&connection, true)?;
    *recorder_guard = Some(controller);

    Ok(RecordingActionResult {
        message: "已启动真实麦克风录音".into(),
        runtime: query_runtime_status(&connection)?,
        latest_session: latest_session(&connection)?,
    })
}

#[tauri::command]
fn stop_recording(state: tauri::State<'_, AppState>) -> Result<RecordingActionResult, String> {
    let controller = state
        .recorder
        .lock()
        .map_err(|_| "录音状态锁定失败".to_string())?
        .take();

    let Some(controller) = controller else {
        let connection = open_connection(&state.db_path)?;
        return Ok(RecordingActionResult {
            message: "当前没有进行中的录音".into(),
            runtime: query_runtime_status(&connection)?,
            latest_session: latest_session(&connection)?,
        });
    };

    controller
        .stop_tx
        .send(RecorderControl::Stop)
        .map_err(|error| format!("发送停止录音指令失败: {error}"))?;
    let result = controller
        .join_handle
        .join()
        .map_err(|_| "录音线程异常退出".to_string())??;

    let connection = open_connection(&state.db_path)?;
    insert_audio_segment(
        &connection,
        &result.file_path,
        &result.started_at_label,
        result.duration_ms,
        result.sample_rate,
        result.channels,
        result.summary.total_energy,
        result.summary.sample_count,
        &result.trace_id,
    )?;
    settings_service::set_record_enabled(&connection, false)?;
    let processing_summary = process_pending_jobs_internal(&connection)?;

    Ok(RecordingActionResult {
        message: format!(
            "录音已停止，已保存本地 WAV 文件：{}。{}",
            result.file_path.to_string_lossy(),
            processing_summary
        ),
        runtime: query_runtime_status(&connection)?,
        latest_session: latest_session(&connection)?,
    })
}

#[tauri::command]
fn simulate_audio_slice(
    has_effective_voice: bool,
    state: tauri::State<'_, AppState>,
) -> Result<RecordingActionResult, String> {
    let connection = open_connection(&state.db_path)?;
    let settings = settings_service::load_settings(&connection)?;
    let timestamp = current_timestamp_label();
    let segment_id = format!("audio_sim_{timestamp}");
    let trace_id = format!("trace_sim_{timestamp}");

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
      ) VALUES (
        ?1,
        ?2,
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP,
        ?3,
        16000,
        1,
        ?4,
        ?5,
        ?6,
        ?7,
        CURRENT_TIMESTAMP
      )
      "#,
            params![
                segment_id.as_str(),
                format!("/mock/{segment_id}.wav"),
                settings.chunk_seconds * 1000,
                if has_effective_voice { 1 } else { 0 },
                if has_effective_voice { 0.82 } else { 0.05 },
                if has_effective_voice {
                    "transcribed"
                } else {
                    "skipped"
                },
                trace_id.as_str()
            ],
        )
        .map_err(|error| format!("写入模拟切片失败: {error}"))?;

    if has_effective_voice {
        insert_processing_job(&connection, "transcription", &segment_id, &trace_id)?;
    }

    let latest_session = if has_effective_voice {
        None
    } else {
        maybe_create_idle_session(&connection)?
    };
    let processing_summary = process_pending_jobs_internal(&connection)?;

    Ok(RecordingActionResult {
        message: if has_effective_voice {
            format!("已写入一条有效录音切片。{processing_summary}")
        } else {
            format!("已写入一条静默切片，并检查空闲触发会话。{processing_summary}")
        },
        runtime: query_runtime_status(&connection)?,
        latest_session,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("解析应用数据目录失败: {error}"))?;
            let recordings_dir = app_data_dir.join("recordings");
            let db_path = app_data_dir.join("smart-todo.sqlite");

            initialize_database(&db_path)?;
            fs::create_dir_all(&recordings_dir)
                .map_err(|error| format!("创建录音目录失败: {error}"))?;
            app.manage(AppState {
                db_path,
                recordings_dir,
                recorder: Arc::new(Mutex::new(None)),
            });

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_desktop_context,
            get_bootstrap_data,
            commands::settings::save_settings,
            commands::model_test::test_model_connection,
            toggle_todo_status,
            flush_current_session,
            process_pending_jobs,
            start_recording,
            stop_recording,
            simulate_audio_slice
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_initialize_v04_provider_and_semantic_schema() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-schema-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取默认设置");

        assert_eq!(settings.asr_provider_type, "local_whisperkit");
        assert_eq!(settings.speaker_provider_type, "local_speakerkit");
        assert_eq!(settings.todo_provider_type, DEFAULT_TODO_PROVIDER_TYPE);
        assert_eq!(settings.semantic_provider_type, "minimax_m3");
        assert_eq!(settings.embedding_provider_type, "reserved");
        assert_eq!(settings.export_provider_type, "local_file");
        assert_eq!(settings.semantic_base_url, DEFAULT_SEMANTIC_BASE_URL);
        assert_eq!(settings.semantic_model_name, "MiniMax-M3");

        for (table, required_columns) in [
            (
                "semantic_artifacts",
                vec![
                    "id",
                    "session_id",
                    "artifact_type",
                    "status",
                    "provider",
                    "model_name",
                    "schema_version",
                    "source_span_refs",
                    "payload_json",
                    "error_message",
                ],
            ),
            (
                "model_invocations",
                vec![
                    "id",
                    "provider",
                    "model_name",
                    "capability",
                    "status",
                    "request_summary",
                    "response_summary",
                    "input_tokens",
                    "output_tokens",
                    "duration_ms",
                    "estimated_cost_microunits",
                    "currency",
                    "error_message",
                    "started_at",
                    "finished_at",
                ],
            ),
        ] {
            let columns = table_columns(&connection, table);
            for required_column in required_columns {
                assert!(
                    columns.iter().any(|column| column == required_column),
                    "{table} 应包含字段 {required_column}，实际字段为 {columns:?}"
                );
            }
        }

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_allow_all_minimax_m3_artifact_types_in_semantic_artifacts() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-artifact-type-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let session_id = "session_minimax_artifact_types";
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
                ) VALUES (?1, '测试 MiniMax M3 artifact 类型落库。', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', 1, 'pending', 'pending', 0, '', 'trace_minimax_artifact_types', CURRENT_TIMESTAMP)
                "#,
                params![session_id],
            )
            .expect("应能准备测试会话");

        for artifact_type in providers::semantic::minimax_m3::SUPPORTED_ARTIFACT_TYPES {
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
                      payload_json
                    ) VALUES (?1, ?2, ?3, 'pending', ?4, ?5, 'v0.4', '[]', '{}')
                    "#,
                    params![
                        format!("artifact_{artifact_type}"),
                        session_id,
                        artifact_type,
                        providers::semantic::minimax_m3::PROVIDER_ID,
                        providers::semantic::minimax_m3::DEFAULT_MODEL_NAME,
                    ],
                )
                .unwrap_or_else(|error| {
                    panic!("semantic_artifacts 应允许 MiniMax M3 artifact_type={artifact_type}: {error}")
                });
        }

        let inserted_count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM semantic_artifacts WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .expect("应能统计 artifact 数量");
        assert_eq!(
            inserted_count as usize,
            providers::semantic::minimax_m3::SUPPORTED_ARTIFACT_TYPES.len()
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_migrate_semantic_artifact_type_constraint_to_minimax_contract() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-artifact-migration-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        {
            let connection = open_connection(&db_path).expect("应能打开旧库测试数据库");
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE semantic_artifacts (
                      id TEXT PRIMARY KEY,
                      session_id TEXT NOT NULL,
                      artifact_type TEXT NOT NULL
                        CHECK (artifact_type IN ('summary', 'todo_extraction', 'mind_map', 'moment', 'deep_research', 'translation')),
                      status TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'running', 'succeeded', 'failed')),
                      provider TEXT NOT NULL,
                      model_name TEXT NOT NULL,
                      schema_version TEXT NOT NULL DEFAULT 'v0.4',
                      source_span_refs TEXT NOT NULL DEFAULT '[]',
                      payload_json TEXT NOT NULL DEFAULT '{}',
                      error_message TEXT NOT NULL DEFAULT '',
                      created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                      updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
                    );

                    INSERT INTO semantic_artifacts (
                      id,
                      session_id,
                      artifact_type,
                      status,
                      provider,
                      model_name,
                      schema_version,
                      source_span_refs,
                      payload_json
                    ) VALUES (
                      'legacy_summary_artifact',
                      'legacy_session',
                      'summary',
                      'pending',
                      'minimax_m3',
                      'MiniMax-M3',
                      'v0.4',
                      '[]',
                      '{}'
                    ),
                    (
                      'legacy_translation_artifact',
                      'legacy_session',
                      'translation',
                      'pending',
                      'minimax_m3',
                      'MiniMax-M3',
                      'v0.4',
                      '[]',
                      '{}'
                    );
                    "#,
                )
                .expect("应能创建旧 semantic_artifacts 表");
        }

        initialize_database(&db_path).expect("应能迁移旧 semantic_artifacts 类型约束");
        let connection = open_connection(&db_path).expect("应能打开迁移后的数据库");
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
                ) VALUES ('legacy_session', '旧会话', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', 1, 'pending', 'pending', 0, '', 'trace_legacy_session', CURRENT_TIMESTAMP)
                "#,
                [],
            )
            .expect("应能准备迁移后父会话");
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
                  payload_json
                ) VALUES ('new_transcript_revision_artifact', 'legacy_session', 'transcript_revision', 'pending', 'minimax_m3', 'MiniMax-M3', 'v0.4', '[]', '{}')
                "#,
                [],
            )
            .expect("迁移后应允许 transcript_revision");

        let legacy_count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM semantic_artifacts WHERE id = 'legacy_summary_artifact'",
                [],
                |row| row.get(0),
            )
            .expect("应能查询迁移前 artifact");
        assert_eq!(legacy_count, 1);

        let legacy_translation_count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM semantic_artifacts WHERE id = 'legacy_translation_artifact'",
                [],
                |row| row.get(0),
            )
            .expect("应能查询迁移前 translation artifact");
        assert_eq!(legacy_translation_count, 1);

        let index_count: i64 = connection
            .query_row(
                r#"
                SELECT COUNT(1)
                FROM sqlite_master
                WHERE type = 'index'
                  AND name IN ('idx_semantic_artifacts_session_type', 'idx_semantic_artifacts_status')
                "#,
                [],
                |row| row.get(0),
            )
            .expect("应能查询迁移后的索引");
        assert_eq!(index_count, 2);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_migrate_legacy_settings_to_v04_provider_defaults() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-migration-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        {
            let connection = open_connection(&db_path).expect("应能打开旧库测试数据库");
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE app_settings (
                        id TEXT PRIMARY KEY,
                        record_enabled INTEGER NOT NULL DEFAULT 0,
                        language TEXT NOT NULL DEFAULT 'zh-CN',
                        chunk_seconds INTEGER NOT NULL DEFAULT 30,
                        idle_trigger_seconds INTEGER NOT NULL DEFAULT 20,
                        provider_mode TEXT NOT NULL DEFAULT 'local',
                        asr_provider_type TEXT NOT NULL DEFAULT 'local',
                        todo_provider_type TEXT NOT NULL DEFAULT 'embedded_local'
                    );

                    INSERT INTO app_settings (
                        id,
                        record_enabled,
                        language,
                        chunk_seconds,
                        idle_trigger_seconds,
                        provider_mode,
                        asr_provider_type,
                        todo_provider_type
                    ) VALUES (
                        'default',
                        0,
                        'zh-CN',
                        30,
                        20,
                        'local',
                        'local',
                        'embedded_local'
                    );
                    "#,
                )
                .expect("应能准备旧版本设置表");
        }

        initialize_database(&db_path).expect("应能迁移旧版本数据库");
        let connection = open_connection(&db_path).expect("应能打开迁移后的数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取迁移后的设置");

        assert_eq!(settings.asr_provider_type, "local_whisperkit");
        assert_eq!(settings.speaker_provider_type, "local_speakerkit");
        assert_eq!(settings.todo_provider_type, DEFAULT_TODO_PROVIDER_TYPE);
        assert_eq!(settings.semantic_provider_type, "minimax_m3");
        assert_eq!(settings.semantic_base_url, DEFAULT_SEMANTIC_BASE_URL);
        assert_eq!(settings.semantic_model_name, DEFAULT_SEMANTIC_MODEL_NAME);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_sqlite_infra_database_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-sqlite-infra-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        infra::sqlite::initialize_database(&db_path).expect("infra sqlite 应能初始化数据库");
        let connection =
            infra::sqlite::open_connection(&db_path).expect("infra sqlite 应能打开数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取默认设置");

        assert_eq!(settings.asr_provider_type, DEFAULT_ASR_PROVIDER_TYPE);
        assert_eq!(
            settings.semantic_provider_type,
            DEFAULT_SEMANTIC_PROVIDER_TYPE
        );
        assert!(
            table_columns(&connection, "semantic_artifacts")
                .iter()
                .any(|column| column == "artifact_type"),
            "infra sqlite 初始化应创建 semantic_artifacts 表"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_settings_service_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-settings-service-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let mut settings = app::settings_service::load_settings(&connection)
            .expect("settings service 应能读取设置");

        settings.language = "en-US".into();
        settings.semantic_model_name = "MiniMax-M3-Service-Test".into();
        app::settings_service::save_settings(&connection, &settings)
            .expect("settings service 应能保存设置");

        let persisted = app::settings_service::load_settings(&connection)
            .expect("settings service 应能回读设置");
        assert_eq!(persisted.language, "en-US");
        assert_eq!(persisted.semantic_model_name, "MiniMax-M3-Service-Test");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_normalize_all_todo_provider_inputs_to_minimax_m3() {
        for provider_type in [
            "",
            "semantic_m3",
            "embedded_local",
            "legacy_local_llm",
            "cloud",
        ] {
            assert_eq!(
                normalize_todo_provider_type(provider_type),
                DEFAULT_TODO_PROVIDER_TYPE,
                "Todo provider 输入 {provider_type:?} 应统一收敛到 MiniMax M3"
            );
        }
    }

    #[test]
    fn should_register_semantic_todo_artifact_by_default() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-semantic-todo-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取默认设置");
        let session_id = "session_semantic_todo_test";

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
                ) VALUES (?1, '下午联系客户并同步合同状态。', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', 1, 'pending', 'pending', 0, '', 'trace_semantic_todo_test', CURRENT_TIMESTAMP)
                "#,
                params![session_id],
            )
            .expect("应能准备测试会话");

        let inserted =
            jobs::todo_extraction::generate_for_session(&connection, &settings, session_id)
                .expect("默认 Todo 路径应登记语义产物边界");
        assert_eq!(inserted, 0);

        let artifact: (String, String, String, String) = connection
            .query_row(
                "SELECT provider, model_name, status, payload_json FROM semantic_artifacts WHERE session_id = ?1 AND artifact_type = 'todo_extraction'",
                params![session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("应登记 Todo 语义产物");
        assert_eq!(artifact.0, DEFAULT_SEMANTIC_PROVIDER_TYPE);
        assert_eq!(artifact.1, DEFAULT_SEMANTIC_MODEL_NAME);
        assert_eq!(artifact.2, "pending");
        assert!(artifact.3.contains("v0.4_semantic_provider"));

        let todo_count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM todos WHERE conversation_session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .expect("应能查询 Todo 数量");
        assert_eq!(todo_count, 0);

        let provider_used: String = connection
            .query_row(
                "SELECT extraction_provider_used FROM conversation_sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .expect("应能读取会话 provider");
        assert_eq!(provider_used, DEFAULT_SEMANTIC_PROVIDER_TYPE);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_todo_extraction_job_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-jobs-boundary-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取默认设置");
        let session_id = "session_jobs_boundary_test";

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
                ) VALUES (?1, '明天复盘架构拆分计划并同步风险。', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'forced_flush', 1, 'pending', 'pending', 0, '', 'trace_jobs_boundary_test', CURRENT_TIMESTAMP)
                "#,
                params![session_id],
            )
            .expect("应能准备测试会话");

        let inserted =
            jobs::todo_extraction::generate_for_session(&connection, &settings, session_id)
                .expect("jobs todo_extraction 应能登记语义产物边界");
        assert_eq!(inserted, 0);

        let artifact_count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM semantic_artifacts WHERE session_id = ?1 AND artifact_type = 'todo_extraction' AND provider = ?2",
                params![session_id, DEFAULT_SEMANTIC_PROVIDER_TYPE],
                |row| row.get(0),
            )
            .expect("应能查询 Todo 语义产物");
        assert_eq!(artifact_count, 1);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_skip_placeholder_manual_session_in_todo_extraction_job() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-jobs-skip-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取默认设置");
        let session_id = "session_jobs_skip_test";

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
                ) VALUES (?1, '手动刷新会话，当前未绑定真实转写文稿。', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', 0, 'pending', 'pending', 0, '', 'trace_jobs_skip_test', CURRENT_TIMESTAMP)
                "#,
                params![session_id],
            )
            .expect("应能准备手动占位会话");

        let inserted =
            jobs::todo_extraction::generate_for_session(&connection, &settings, session_id)
                .expect("jobs todo_extraction 应跳过占位会话");
        assert_eq!(inserted, 0);

        let artifact_count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM semantic_artifacts WHERE session_id = ?1 AND artifact_type = 'todo_extraction'",
                params![session_id],
                |row| row.get(0),
            )
            .expect("应能查询 Todo 语义产物数量");
        assert_eq!(artifact_count, 0);

        let provider_used: String = connection
            .query_row(
                "SELECT extraction_provider_used FROM conversation_sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .expect("应能读取会话提取 provider");
        assert_eq!(provider_used, "skipped");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_replace_existing_todo_extraction_artifact_for_session() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-jobs-replace-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取默认设置");
        let session_id = "session_jobs_replace_test";

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
                ) VALUES (?1, '请安排复盘会议并同步会议纪要。', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'forced_flush', 1, 'pending', 'pending', 0, '', 'trace_jobs_replace_test', CURRENT_TIMESTAMP)
                "#,
                params![session_id],
            )
            .expect("应能准备可提取会话");

        jobs::todo_extraction::generate_for_session(&connection, &settings, session_id)
            .expect("首次应登记 Todo 语义产物");
        jobs::todo_extraction::generate_for_session(&connection, &settings, session_id)
            .expect("再次登记应替换旧 Todo 语义产物");

        let artifact_count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM semantic_artifacts WHERE session_id = ?1 AND artifact_type = 'todo_extraction'",
                params![session_id],
                |row| row.get(0),
            )
            .expect("应能查询 Todo 语义产物数量");
        assert_eq!(artifact_count, 1);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_persist_v04_provider_settings_round_trip() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-settings-roundtrip-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let mut settings = settings_service::load_settings(&connection).expect("应能读取默认设置");

        settings.asr_provider_type = "cloud_volc".into();
        settings.todo_provider_type = "legacy_local_llm".into();
        settings.semantic_base_url = "https://m3.example.test/v1/responses".into();
        settings.semantic_model_name = "MiniMax-M3-Test".into();
        settings.semantic_api_key_masked = "sk-test-****".into();
        settings.export_provider_type = "local_file".into();

        settings_service::save_settings(&connection, &settings).expect("应能保存 v0.4 设置");
        let persisted = settings_service::load_settings(&connection).expect("应能读取保存后的设置");

        assert_eq!(persisted.asr_provider_type, "cloud_volc");
        assert_eq!(persisted.todo_provider_type, DEFAULT_TODO_PROVIDER_TYPE);
        assert_eq!(
            persisted.semantic_base_url,
            "https://m3.example.test/v1/responses"
        );
        assert_eq!(persisted.semantic_model_name, "MiniMax-M3-Test");
        assert_eq!(persisted.semantic_api_key_masked, "sk-test-****");
        assert_eq!(persisted.export_provider_type, "local_file");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_model_test_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-model-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取默认设置");

        let result = commands::model_test::test_model_connection_payload(ModelTestRequest {
            provider: "todo".into(),
            settings,
        })
        .expect("model_test command boundary 应能处理 Todo 语义入口测试");

        assert!(result.success);
        assert_eq!(result.provider, "todo");
        assert!(result.message.contains("MiniMax M3"));
        assert_eq!(
            result.response_excerpt,
            "semantic_artifacts(type='todo_extraction')"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_guard_local_asr_model_test_without_cloud_fallback() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-model-command-asr-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let mut settings = settings_service::load_settings(&connection).expect("应能读取默认设置");
        settings.asr_provider_type = DEFAULT_ASR_PROVIDER_TYPE.into();
        settings.allow_cloud_fallback = false;

        let result = commands::model_test::test_model_connection_payload(ModelTestRequest {
            provider: "asr".into(),
            settings,
        })
        .expect("model_test command boundary 应能处理本地 ASR guard");

        assert!(!result.success);
        assert_eq!(result.provider, "asr");
        assert_eq!(result.status_code, 503);
        assert!(result.message.contains("纯本地 ASR"));
        assert!(result.response_excerpt.contains("不会调用云端服务"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_reject_unknown_model_test_provider_at_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-model-command-unknown-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取默认设置");

        let error = commands::model_test::test_model_connection_payload(ModelTestRequest {
            provider: "embedding".into(),
            settings,
        })
        .expect_err("未知模型测试类型应返回错误");

        assert!(error.contains("不支持的模型测试类型: embedding"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_save_settings_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-save-settings-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let mut settings = settings_service::load_settings(&connection).expect("应能读取默认设置");
        settings.language = "en-US".into();
        settings.todo_provider_type = "cloud".into();
        settings.semantic_model_name = "MiniMax-M3-Command-Test".into();

        let saved = commands::settings::save_settings_payload(&db_path, settings)
            .expect("settings command boundary 应能保存并回读设置");

        assert_eq!(saved.language, "en-US");
        assert_eq!(saved.todo_provider_type, DEFAULT_TODO_PROVIDER_TYPE);
        assert_eq!(saved.semantic_model_name, "MiniMax-M3-Command-Test");

        let persisted = settings_service::load_settings(&connection).expect("应能读取保存后的设置");
        assert_eq!(persisted.language, "en-US");
        assert_eq!(persisted.todo_provider_type, DEFAULT_TODO_PROVIDER_TYPE);
        assert_eq!(persisted.semantic_model_name, "MiniMax-M3-Command-Test");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_v04_provider_module_boundaries() {
        let asr_descriptor = providers::asr::local_whisperkit::descriptor();
        assert_eq!(asr_descriptor.id, "local_whisperkit");
        assert_eq!(
            asr_descriptor.capability,
            domain::provider::ProviderCapability::Asr
        );
        assert_eq!(
            asr_descriptor.locality,
            domain::provider::ProviderLocality::Local
        );
        assert!(
            asr_descriptor.privacy_boundary.contains("音频默认留在本机"),
            "本地 ASR provider 必须声明本地隐私边界"
        );
        assert_eq!(
            providers::asr::local_whisperkit::LocalWhisperKitProvider::default().provider_id(),
            "local_whisperkit"
        );

        let speaker_descriptor = providers::speaker::local_speakerkit::descriptor();
        assert_eq!(speaker_descriptor.id, "local_speakerkit");
        assert_eq!(
            speaker_descriptor.capability,
            domain::provider::ProviderCapability::Speaker
        );
        assert_eq!(
            providers::speaker::local_speakerkit::LocalSpeakerKitProvider::default().provider_id(),
            "local_speakerkit"
        );

        let semantic_descriptor = providers::semantic::minimax_m3::descriptor();
        assert_eq!(semantic_descriptor.id, "minimax_m3");
        assert_eq!(
            semantic_descriptor.capability,
            domain::provider::ProviderCapability::Semantic
        );
        assert_eq!(
            providers::semantic::minimax_m3::DEFAULT_MODEL_NAME,
            DEFAULT_SEMANTIC_MODEL_NAME
        );
        assert_eq!(
            providers::semantic::minimax_m3::MAX_CONTEXT_TOKENS,
            1_000_000
        );
    }

    #[test]
    fn should_expose_v04_provider_catalog() {
        let catalog = providers::provider_catalog();

        for expected_provider in [
            "local_whisperkit",
            "local_speakerkit",
            "minimax_m3",
            "reserved",
            "local_file",
        ] {
            assert!(
                catalog
                    .iter()
                    .any(|provider| provider.id == expected_provider),
                "provider catalog 应包含 {expected_provider}，实际为 {catalog:?}"
            );
        }

        assert!(
            catalog.iter().any(|provider| provider.capability
                == domain::provider::ProviderCapability::Semantic
                && provider.privacy_boundary.contains("云端语义理解")),
            "MiniMax M3 语义 provider 必须声明云端隐私边界"
        );
    }

    fn table_columns(connection: &Connection, table_name: &str) -> Vec<String> {
        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({table_name})"))
            .expect("应能读取表结构");
        let rows = statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("应能查询表字段");

        rows.collect::<Result<Vec<_>, _>>()
            .expect("应能读取字段列表")
    }
}
