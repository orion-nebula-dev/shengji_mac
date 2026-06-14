use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
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
use domain::model_test::ModelTestResult;
use domain::session::SessionDto;
use domain::settings::SettingsDto;
use domain::transcript::TranscriptRecord;
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

#[derive(Debug)]
struct AudioSegmentRecord {
    id: String,
    file_path: String,
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
    app::query_service::latest_session(connection)
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
                            DEFAULT_TODO_PROVIDER_TYPE,
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
            commands::desktop_context::get_desktop_context,
            commands::bootstrap::get_bootstrap_data,
            commands::settings::save_settings,
            commands::model_test::test_model_connection,
            commands::todo::toggle_todo_status,
            commands::todo::update_todo_status,
            commands::todo::sync_todo_candidates,
            commands::todo::list_todo_candidates,
            commands::todo::accept_todo_candidate,
            commands::todo::dismiss_todo_candidate,
            commands::session::flush_current_session,
            commands::jobs::process_pending_jobs,
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::simulate_audio_slice,
            commands::transcript::import_local_audio,
            commands::transcript::get_transcript_review,
            commands::transcript::rename_speaker,
            commands::transcript::mark_transcript_segment,
            commands::transcript::retry_transcript_job,
            commands::semantic::generate_semantic_workbench,
            commands::semantic::get_semantic_workbench,
            commands::semantic::set_correction_pattern_enabled,
            commands::semantic::delete_correction_pattern,
            commands::semantic::retry_semantic_artifact,
            commands::semantic::reject_transcript_revision,
            commands::semantic::generate_mind_map,
            commands::semantic::update_mind_map_node,
            commands::semantic::toggle_mind_map_node,
            commands::semantic::export_mind_map,
            commands::semantic::generate_value_discovery,
            commands::semantic::generate_translation,
            commands::semantic::start_research_from_segment,
            commands::semantic::convert_research_to_todo,
            commands::semantic::add_research_to_mind_map,
            commands::export::generate_export_bundle
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
    fn should_initialize_missing_settings_columns_with_v04_provider_defaults() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-settings-columns-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        {
            let connection = open_connection(&db_path).expect("应能打开缺列测试数据库");
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
                        asr_provider_type TEXT NOT NULL DEFAULT 'local'
                    );

                    INSERT INTO app_settings (
                        id,
                        record_enabled,
                        language,
                        chunk_seconds,
                        idle_trigger_seconds,
                        provider_mode,
                        asr_provider_type
                    ) VALUES (
                        'default',
                        0,
                        'zh-CN',
                        30,
                        20,
                        'local',
                        'local'
                    );
                    "#,
                )
                .expect("应能准备缺失 v0.4 列的设置表");
        }

        initialize_database(&db_path).expect("应能补齐 v0.4 设置列");
        let connection = open_connection(&db_path).expect("应能打开补齐后的数据库");
        let settings = settings_service::load_settings(&connection).expect("应能读取补齐后的设置");

        assert_eq!(settings.asr_provider_type, "local_whisperkit");
        assert_eq!(settings.speaker_provider_type, "local_speakerkit");
        assert_eq!(settings.todo_provider_type, DEFAULT_TODO_PROVIDER_TYPE);
        assert_eq!(settings.semantic_provider_type, "minimax_m3");
        assert_eq!(settings.semantic_base_url, DEFAULT_SEMANTIC_BASE_URL);
        assert_eq!(settings.semantic_model_name, DEFAULT_SEMANTIC_MODEL_NAME);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_clamp_existing_unsupported_semantic_provider_during_database_init() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-semantic-provider-migration-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        {
            let connection = open_connection(&db_path).expect("应能打开测试数据库");
            connection
                .execute(
                    "UPDATE app_settings SET semantic_provider_type = 'unsupported_semantic_provider' WHERE id = 'default'",
                    [],
                )
                .expect("应能模拟旧语义 provider 持久化值");
        }

        initialize_database(&db_path).expect("再次初始化应收敛旧语义 provider");
        let connection = open_connection(&db_path).expect("应能打开迁移后的数据库");
        let persisted: String = connection
            .query_row(
                "SELECT semantic_provider_type FROM app_settings WHERE id = 'default'",
                [],
                |row| row.get(0),
            )
            .expect("应能读取迁移后的语义 provider");

        assert_eq!(persisted, DEFAULT_SEMANTIC_PROVIDER_TYPE);

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
    fn should_expose_query_service_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-query-service-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");

        let todos =
            app::query_service::query_todos(&connection).expect("query service 应能读取 Todo 列表");
        let sessions = app::query_service::query_sessions(&connection)
            .expect("query service 应能读取会话列表");
        let runtime = app::query_service::query_runtime_status(&connection)
            .expect("query service 应能读取运行状态");

        assert!(!todos.is_empty(), "query service 应返回 demo Todo");
        assert!(!sessions.is_empty(), "query service 应返回 demo session");
        assert!(!runtime.runtime_label.trim().is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_import_local_audio_and_generate_v05_timeline() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v05-transcript-evaluation-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("sample-meeting.wav");
        fs::write(&audio_path, b"fake wav bytes for local evaluation")
            .expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        let result = commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("v0.5 应能导入本地音频并生成离线评估时间轴");

        assert_eq!(result.audio.status, "succeeded");
        assert!(!result.segments.is_empty(), "应生成时间轴转写片段");
        assert!(
            result
                .segments
                .iter()
                .all(|segment| segment.end_ms > segment.start_ms),
            "每个转写片段都应有可跳转时间范围"
        );
        assert!(
            result
                .speakers
                .iter()
                .any(|speaker| speaker.label == "Speaker 1"),
            "应生成默认 speaker label"
        );
        assert!(result.model_status.offline_available);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_rename_speaker_and_mark_transcript_segment() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v05-speaker-rename-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("sample-standup.wav");
        fs::write(&audio_path, b"fake wav bytes for speaker evaluation")
            .expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        let result = commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("应能准备 speaker 测试数据");
        let speaker_id = result.speakers[0].id.clone();
        let segment_id = result.segments[0].id.clone();

        let renamed = commands::transcript::rename_speaker_payload(
            &db_path,
            speaker_id.as_str(),
            "产品负责人",
        )
        .expect("speaker label 应可重命名并持久化");
        assert_eq!(renamed.label, "产品负责人");

        let marked = commands::transcript::mark_transcript_segment_payload(
            &db_path,
            segment_id.as_str(),
            "speaker",
            "说话人需要人工复核",
        )
        .expect("转写片段应支持错误标注");
        assert_eq!(marked.review_status, "flagged");
        assert_eq!(marked.review_reason, "说话人需要人工复核");

        let refreshed = commands::transcript::get_transcript_review_payload(&db_path)
            .expect("应能读取更新后的转写评估状态");
        assert!(refreshed
            .speakers
            .iter()
            .any(|speaker| speaker.label == "产品负责人"));
        assert!(refreshed
            .segments
            .iter()
            .any(|segment| segment.id == segment_id && segment.review_status == "flagged"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_retry_failed_transcript_job() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v05-transcript-retry-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
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
                  created_at
                ) VALUES ('transcript_job_failed_retry', 'missing_audio_for_retry', 'failed', 1, 3, '本地模型未就绪', 'local_whisperkit', 'large-v3-turbo', CURRENT_TIMESTAMP)
                "#,
                [],
            )
            .expect("应能准备失败转写任务");

        let retried = commands::transcript::retry_transcript_job_payload(
            &db_path,
            "transcript_job_failed_retry",
        )
        .expect("失败转写任务应可重试");
        assert_eq!(retried.status, "queued");
        assert_eq!(retried.retry_count, 2);
        assert!(retried.error_message.is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_reject_transcript_job_retry_after_max_attempts() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v05-transcript-retry-limit-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
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
                  created_at
                ) VALUES ('transcript_job_retry_exhausted', 'missing_audio_for_retry', 'failed', 3, 3, '本地模型未就绪', 'local_whisperkit', 'large-v3-turbo', CURRENT_TIMESTAMP)
                "#,
                [],
            )
            .expect("应能准备达到重试上限的失败任务");

        let error = commands::transcript::retry_transcript_job_payload(
            &db_path,
            "transcript_job_retry_exhausted",
        )
        .expect_err("达到最大重试次数后不应继续重试");
        assert!(error.contains("最大重试次数"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_generate_v06_revision_and_semantic_artifacts_from_corrected_transcript() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v06-semantic-workbench-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("sample-review.wav");
        fs::write(&audio_path, b"fake wav bytes for semantic review")
            .expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("应能准备 v0.6 转写输入");

        let workbench = commands::semantic::generate_semantic_workbench_payload(&db_path)
            .expect("v0.6 应能生成修正文稿和结构化语义产物");

        assert_eq!(workbench.recording_type.template_id, "meeting_minutes_v1");
        assert!(!workbench.revisions.is_empty(), "应生成转写修正对照");
        assert!(workbench
            .revisions
            .iter()
            .any(|revision| revision.change_level == "meaning_affecting"
                && !revision.reason_summary.trim().is_empty()
                && !revision.source_segment_id.trim().is_empty()));
        assert!(workbench.summary.basis.contains("修正文稿"));
        assert!(workbench
            .meeting_minutes
            .decisions
            .iter()
            .any(|item| item.contains("复核")));
        assert!(workbench.todo_candidates.iter().any(|todo| {
            todo.title.contains("复核")
                && todo
                    .source_segment_ids
                    .iter()
                    .any(|segment_id| segment_id.starts_with("transcript_"))
        }));

        let artifact_types: Vec<String> = workbench
            .artifacts
            .iter()
            .map(|artifact| artifact.artifact_type.clone())
            .collect();
        for artifact_type in [
            "transcript_revision",
            "recording_type",
            "summary",
            "meeting_minutes",
            "todo_extraction",
        ] {
            assert!(
                artifact_types.iter().any(|value| value == artifact_type),
                "应写入 semantic_artifacts({artifact_type})"
            );
        }
        assert!(workbench
            .model_invocations
            .iter()
            .any(|invocation| invocation.capability == "semantic"
                && invocation.status == "succeeded"));

        let revision_to_reject = workbench
            .revisions
            .iter()
            .find(|revision| revision.change_level != "none")
            .expect("应至少有一条可拒绝的修正");
        assert_eq!(revision_to_reject.status, "proposed");
        let rejected = commands::semantic::reject_transcript_revision_payload(
            &db_path,
            revision_to_reject.id.as_str(),
        )
        .expect("用户应能拒绝某条修正");
        assert_eq!(rejected.status, "rejected");
        let refreshed = commands::semantic::get_semantic_workbench_payload(&db_path)
            .expect("拒绝后应能刷新工作台");
        assert!(refreshed
            .revisions
            .iter()
            .any(|revision| revision.id == rejected.id && revision.status == "rejected"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_keep_v06_correction_memory_private_and_mutable() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v06-correction-memory-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("private-customer-call.wav");
        fs::write(&audio_path, b"fake wav bytes for correction memory")
            .expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("应能准备 v0.6 修正记忆输入");

        let workbench = commands::semantic::generate_semantic_workbench_payload(&db_path)
            .expect("应能生成修正记忆");
        let pattern = workbench
            .correction_patterns
            .first()
            .expect("应生成至少一条短语级修正记忆");

        assert!(!pattern.phrase.trim().is_empty());
        assert!(!pattern.replacement.trim().is_empty());
        assert!(
            !pattern.phrase.contains("已导入")
                && !pattern.phrase.contains("private-customer-call.wav"),
            "修正记忆不应保存完整转写稿或完整音频路径"
        );

        let disabled = commands::semantic::set_correction_pattern_enabled_payload(
            &db_path,
            pattern.id.as_str(),
            false,
        )
        .expect("修正记忆应可禁用");
        assert!(!disabled.enabled);

        let deleted =
            commands::semantic::delete_correction_pattern_payload(&db_path, pattern.id.as_str())
                .expect("修正记忆应可删除");
        assert_eq!(deleted.deleted_id, pattern.id);

        let refreshed = commands::semantic::get_semantic_workbench_payload(&db_path)
            .expect("应能读取删除后的工作台状态");
        assert!(!refreshed
            .correction_patterns
            .iter()
            .any(|candidate| candidate.id == pattern.id));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_store_v06_semantic_parse_failure_and_retry() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v06-semantic-retry-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let failed = commands::semantic::record_semantic_parse_failure_payload(
            &db_path,
            "session_semantic_retry",
            "{not-json",
        )
        .expect("M3 JSON 解析失败应写入 failed artifact");
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.artifact_type, "summary");
        assert!(failed.error_message.contains("JSON"));

        let retried =
            commands::semantic::retry_semantic_artifact_payload(&db_path, failed.id.as_str())
                .expect("失败语义产物应提供重试入口");
        assert_eq!(retried.status, "pending");
        assert!(retried.error_message.is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_generate_edit_and_export_v08_mind_map_without_overwriting_edits() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v08-mind-map-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("sample-mind-map.wav");
        fs::write(&audio_path, b"fake wav bytes for mind map").expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("应能准备 v0.8 脑图输入");
        commands::semantic::generate_semantic_workbench_payload(&db_path)
            .expect("应能先生成修正文稿和摘要");

        let generated = commands::semantic::generate_mind_map_payload(&db_path)
            .expect("v0.8 应能生成 mind_map artifact");
        assert_eq!(generated.artifact_type, "mind_map");
        assert_eq!(generated.schema_version, "v0.8");
        assert_eq!(generated.status, "succeeded");

        let parsed =
            serde_json::from_str::<domain::artifact::MindMapDto>(generated.payload_json.as_str())
                .expect("mind_map payload 应符合契约");
        assert_eq!(parsed.root, "root");
        assert!(!parsed.nodes.is_empty(), "脑图应至少包含根节点和主题节点");
        assert!(parsed
            .nodes
            .iter()
            .any(|node| !node.source_span_refs.is_empty()));
        assert!(parsed.summary.contains("修正文稿") || parsed.summary.contains("摘要"));
        assert!(!parsed.edited);

        let editable_node = parsed
            .nodes
            .iter()
            .find(|node| node.id != parsed.root)
            .expect("应存在可编辑节点");
        let edited = commands::semantic::update_mind_map_node_payload(
            &db_path,
            domain::artifact::UpdateMindMapNodeCommand {
                artifact_id: generated.id.clone(),
                node_id: editable_node.id.clone(),
                label: "复核说话人标签和时间跳转".into(),
                note: "用户编辑后的节点说明必须保留为新版本。".into(),
            },
        )
        .expect("用户编辑节点应生成新版本 artifact");
        assert_ne!(edited.id, generated.id, "编辑不得覆盖原始生成 artifact");

        let edited_payload =
            serde_json::from_str::<domain::artifact::MindMapDto>(edited.payload_json.as_str())
                .expect("编辑后的 mind_map payload 应符合契约");
        assert!(edited_payload.edited);
        assert_eq!(edited_payload.parent_artifact_id, generated.id);
        assert!(edited_payload
            .nodes
            .iter()
            .any(|node| node.label == "复核说话人标签和时间跳转"));

        let regenerated = commands::semantic::generate_mind_map_payload(&db_path)
            .expect("重新生成脑图应生成新 artifact");
        assert_ne!(regenerated.id, edited.id);
        let original_after_regen = {
            let connection = open_connection(&db_path).expect("应能打开测试数据库");
            connection
                .query_row(
                    "SELECT payload_json FROM semantic_artifacts WHERE id = ?1",
                    params![edited.id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .expect("编辑版本不应被重新生成覆盖")
        };
        assert!(original_after_regen.contains("复核说话人标签和时间跳转"));

        let markdown =
            commands::semantic::export_mind_map_payload(&db_path, edited.id.as_str(), "markdown")
                .expect("应能导出 Markdown 脑图");
        assert_eq!(markdown.format, "markdown");
        assert!(markdown.content.contains("# 语义脑图"));
        assert!(markdown.content.contains("来源"));

        let json =
            commands::semantic::export_mind_map_payload(&db_path, edited.id.as_str(), "json")
                .expect("应能导出 JSON 脑图");
        assert_eq!(json.format, "json");
        assert!(json.content.contains("\"nodes\""));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_generate_v09_moments_and_research_with_action_conversion() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v09-value-discovery-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("sample-value-discovery.wav");
        fs::write(&audio_path, b"fake wav bytes for value discovery")
            .expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("应能准备 v0.9 价值发现输入");
        commands::semantic::generate_semantic_workbench_payload(&db_path)
            .expect("应能先生成修正文稿和摘要");
        commands::semantic::generate_mind_map_payload(&db_path)
            .expect("应能先生成可追加节点的脑图");

        let moment_artifact = commands::semantic::generate_value_discovery_payload(&db_path)
            .expect("v0.9 应能生成 Moment artifact");
        assert_eq!(moment_artifact.artifact_type, "moment");
        assert_eq!(moment_artifact.schema_version, "v0.9");
        let moments = serde_json::from_str::<Vec<domain::artifact::MomentDto>>(
            moment_artifact.payload_json.as_str(),
        )
        .expect("moment payload 应符合契约");
        assert!(
            (3..=10).contains(&moments.len()),
            "Moment 数量应在 3-10 个之间，实际为 {}",
            moments.len()
        );
        assert!(moments
            .iter()
            .all(|moment| moment.start_ms <= moment.end_ms));
        assert!(moments
            .iter()
            .all(|moment| !moment.source_span_refs.is_empty()));
        assert!(moments
            .iter()
            .any(|moment| moment.moment_type == "decision"));
        assert!(moments.iter().any(|moment| moment.moment_type == "risk"));

        let workbench = commands::semantic::get_semantic_workbench_payload(&db_path)
            .expect("v0.9 工作台应能读取 Moment 和研究草稿");
        assert_eq!(workbench.moments.len(), moments.len());
        assert!(!workbench.deep_research.is_empty(), "应生成研究草稿");
        let auto_research = workbench.deep_research.first().expect("应存在自动研究草稿");
        assert!(!auto_research.question.trim().is_empty());
        assert!(!auto_research.background.trim().is_empty());
        assert!(!auto_research.hypotheses.is_empty());
        assert!(!auto_research.search_directions.is_empty());
        assert!(!auto_research.next_steps.is_empty());

        let source_revision = workbench
            .revisions
            .first()
            .expect("应存在可发起研究的来源片段");
        let research_artifact = commands::semantic::start_research_from_segment_payload(
            &db_path,
            domain::artifact::StartResearchFromSegmentCommand {
                segment_id: source_revision.source_segment_id.clone(),
                question: "这个风险是否会影响离线转写验收？".into(),
            },
        )
        .expect("应能从转写片段发起研究");
        assert_eq!(research_artifact.artifact_type, "deep_research");
        let research = serde_json::from_str::<domain::artifact::DeepResearchDraftDto>(
            research_artifact.payload_json.as_str(),
        )
        .expect("deep_research payload 应符合契约");
        assert!(research
            .source_span_refs
            .contains(&source_revision.source_segment_id));
        assert!(research.question.contains("离线转写验收"));

        let converted = commands::semantic::convert_research_to_todo_payload(
            &db_path,
            domain::artifact::ConvertResearchToTodoCommand {
                artifact_id: research_artifact.id.clone(),
                research_id: research.id.clone(),
            },
        )
        .expect("研究草稿应能转为正式 Todo");
        assert_eq!(converted.status, "open");
        assert!(converted.title.contains("研究"));
        assert!(converted
            .source_span_refs
            .contains(&source_revision.source_segment_id));

        let mind_map_artifact = commands::semantic::add_research_to_mind_map_payload(
            &db_path,
            domain::artifact::AddResearchToMindMapCommand {
                artifact_id: research_artifact.id.clone(),
                research_id: research.id.clone(),
            },
        )
        .expect("研究草稿应能转成脑图节点");
        assert_eq!(mind_map_artifact.artifact_type, "mind_map");
        let mind_map = serde_json::from_str::<domain::artifact::MindMapDto>(
            mind_map_artifact.payload_json.as_str(),
        )
        .expect("追加研究节点后的 mind_map payload 应符合契约");
        assert!(mind_map.edited, "追加研究节点应生成编辑版脑图");
        assert!(mind_map
            .nodes
            .iter()
            .any(|node| node.kind == "research" && node.label.contains("研究")));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_generate_v10_export_bundle_and_local_share_snapshot() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v10-export-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("sample-export.wav");
        fs::write(&audio_path, b"fake wav bytes for export").expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("应能准备 v1.0 导出输入");
        commands::semantic::generate_semantic_workbench_payload(&db_path)
            .expect("应能生成语义工作台");
        commands::semantic::generate_mind_map_payload(&db_path).expect("应能生成脑图");
        commands::semantic::generate_value_discovery_payload(&db_path).expect("应能生成价值发现");

        let bundle = commands::export::generate_export_bundle_payload(
            &db_path,
            domain::export::GenerateExportBundleCommand {
                formats: vec![
                    "markdown".into(),
                    "srt".into(),
                    "json".into(),
                    "snapshot".into(),
                ],
                target_languages: Vec::new(),
            },
        )
        .expect("v1.0 应能生成完整导出包");

        assert_eq!(bundle.provider, "local_file");
        assert_eq!(bundle.status, "succeeded");
        assert_eq!(bundle.items.len(), 4);
        assert!(bundle.privacy_summary.contains("本地"));
        assert!(bundle
            .items
            .iter()
            .any(|item| item.format == "markdown" && item.content.contains("# 声记会话导出")));
        assert!(bundle
            .items
            .iter()
            .any(|item| item.format == "srt" && item.content.contains("00:00:00,000 -->")));
        let json_item = bundle
            .items
            .iter()
            .find(|item| item.format == "json")
            .expect("应包含 JSON 导出");
        let json_value: serde_json::Value =
            serde_json::from_str(json_item.content.as_str()).expect("JSON 导出应可解析");
        assert_eq!(json_value["sessionId"], bundle.session_id);
        assert!(json_value["transcriptSegments"].is_array());
        assert!(json_value["semanticArtifacts"].is_array());

        let snapshot = bundle.snapshot.as_ref().expect("应生成本地分享快照");
        assert!(snapshot.html.contains("<!doctype html>"));
        assert!(snapshot.html.contains("声记分享快照"));
        assert!(
            !snapshot.html.contains(temp_dir.to_string_lossy().as_ref()),
            "分享快照不应暴露完整本地路径"
        );

        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let export_count: i64 = connection
            .query_row("SELECT COUNT(1) FROM external_exports", [], |row| {
                row.get(0)
            })
            .expect("应能读取导出记录");
        assert_eq!(export_count, 4);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_generate_v11_translation_and_multilingual_export_without_overwriting_summary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v11-translation-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("sample-translation.wav");
        fs::write(&audio_path, b"fake wav bytes for translation").expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("应能准备 v1.1 翻译输入");
        commands::semantic::generate_semantic_workbench_payload(&db_path)
            .expect("应能生成基础语义工作台");

        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let original_summary_payload: String = connection
            .query_row(
                "SELECT payload_json FROM semantic_artifacts WHERE artifact_type = 'summary' ORDER BY datetime(created_at) DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("应能读取原始摘要产物");
        drop(connection);

        let translation = commands::semantic::generate_translation_payload(
            &db_path,
            domain::artifact::GenerateTranslationCommand {
                target_language: "en-US".into(),
            },
        )
        .expect("v1.1 应能生成翻译产物");

        assert_eq!(translation.artifact_type, "translation");
        assert_eq!(translation.status, "succeeded");
        assert_eq!(translation.schema_version, "v1.1");

        let payload: serde_json::Value = serde_json::from_str(translation.payload_json.as_str())
            .expect("translation payload 应可解析");
        assert_eq!(payload["targetLanguage"], "en-US");
        assert!(payload["transcriptTranslations"].is_array());
        assert!(payload["transcriptTranslations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["sourceSegmentId"]
                .as_str()
                .unwrap_or("")
                .starts_with("transcript_")
                && item["translatedText"]
                    .as_str()
                    .unwrap_or("")
                    .contains("[en-US]")));
        assert_eq!(
            payload["summaryTranslation"]["sourceArtifactType"],
            "summary"
        );
        assert!(payload["summaryTranslation"]["translatedTitle"]
            .as_str()
            .unwrap_or("")
            .contains("[en-US]"));

        let connection = open_connection(&db_path).expect("应能重新打开测试数据库");
        let refreshed_summary_payload: String = connection
            .query_row(
                "SELECT payload_json FROM semantic_artifacts WHERE artifact_type = 'summary' ORDER BY datetime(created_at) DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("应能读取翻译后的摘要产物");
        assert_eq!(
            refreshed_summary_payload, original_summary_payload,
            "摘要翻译不应覆盖原始摘要"
        );
        drop(connection);

        let bundle = commands::export::generate_export_bundle_payload(
            &db_path,
            domain::export::GenerateExportBundleCommand {
                formats: vec!["markdown".into(), "json".into(), "snapshot".into()],
                target_languages: vec!["en-US".into()],
            },
        )
        .expect("v1.1 应能生成多语言导出包");

        assert!(bundle.items.iter().any(|item| {
            item.format == "markdown_en-US"
                && item.file_name.contains("en-US")
                && item.content.contains("# ShengJi Multilingual Export")
                && item.content.contains("Transcript Translation")
        }));
        let multilingual_json = bundle
            .items
            .iter()
            .find(|item| item.format == "json_en-US")
            .expect("应包含多语言 JSON 导出");
        let multilingual_value: serde_json::Value =
            serde_json::from_str(multilingual_json.content.as_str()).expect("多语言 JSON 应可解析");
        assert_eq!(multilingual_value["targetLanguage"], "en-US");
        assert!(multilingual_value["translations"]["transcriptTranslations"].is_array());
        assert!(bundle
            .snapshot
            .as_ref()
            .unwrap()
            .html
            .contains("Multilingual"));
        assert!(
            !bundle
                .snapshot
                .as_ref()
                .unwrap()
                .html
                .contains(temp_dir.to_string_lossy().as_ref()),
            "多语言快照不应暴露完整本地路径"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_accept_v07_todo_candidate_with_traceable_source() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v07-todo-candidate-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let audio_path = temp_dir.join("candidate-source.wav");
        fs::write(&audio_path, b"fake wav bytes for v07 todo candidate")
            .expect("应能准备本地音频文件");

        initialize_database(&db_path).expect("应能初始化数据库");
        commands::transcript::import_local_audio_payload(
            &db_path,
            audio_path.to_string_lossy().as_ref(),
        )
        .expect("应能准备 v0.7 来源转写");
        commands::semantic::generate_semantic_workbench_payload(&db_path)
            .expect("应能生成 v0.6 待办候选语义产物");

        let candidates = commands::todo::sync_todo_candidates_payload(&db_path)
            .expect("v0.7 应能从 todo_extraction artifact 同步候选");
        let candidate = candidates
            .iter()
            .find(|candidate| candidate.title.contains("复核"))
            .expect("应生成可确认的待办候选");
        assert_eq!(candidate.status, "proposed");
        assert!(candidate
            .source_span_refs
            .iter()
            .any(|source| source.starts_with("transcript_")));
        assert!(candidate.source_text.contains("speaker label"));

        let accepted = commands::todo::accept_todo_candidate_payload(
            &db_path,
            commands::todo::AcceptTodoCandidateCommand {
                candidate_id: candidate.id.clone(),
                title: "复核说话人标签与转写片段".into(),
                detail: "确认说话人切换点和修正文稿是否准确。".into(),
                owner: "我".into(),
                due_at: "2026-06-15 18:00".into(),
                priority: "high".into(),
            },
        )
        .expect("用户确认后候选应进入正式 Todo");
        assert_eq!(accepted.status, "open");
        assert_eq!(accepted.owner, "我");
        assert_eq!(accepted.priority, "high");
        assert!(accepted
            .source_span_refs
            .iter()
            .any(|source| source.starts_with("transcript_")));

        let refreshed =
            commands::todo::list_todo_candidates_payload(&db_path).expect("应能读取候选状态");
        assert!(refreshed
            .iter()
            .any(|candidate| candidate.todo_id == accepted.id && candidate.status == "accepted"));

        let duplicate = commands::todo::accept_todo_candidate_payload(
            &db_path,
            commands::todo::AcceptTodoCandidateCommand {
                candidate_id: candidate.id.clone(),
                title: accepted.title.clone(),
                detail: accepted.note.clone(),
                owner: accepted.owner.clone(),
                due_at: accepted.due_at.clone(),
                priority: accepted.priority.clone(),
            },
        )
        .expect("重复确认不应生成第二条正式 Todo");
        assert_eq!(duplicate.id, accepted.id);

        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let todo_count: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM todos WHERE title = ?1",
                params![accepted.title.as_str()],
                |row| row.get(0),
            )
            .expect("应能查询正式 Todo 数量");
        assert_eq!(todo_count, 1);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_migrate_v07_todo_statuses_and_persist_status_flow() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shengji-v07-todo-status-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let connection = open_connection(&db_path).expect("应能创建旧测试数据库");
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                CREATE TABLE conversation_sessions (
                  id TEXT PRIMARY KEY,
                  merged_text TEXT NOT NULL,
                  started_at DATETIME NOT NULL,
                  ended_at DATETIME NOT NULL,
                  idle_trigger_seconds INTEGER NOT NULL,
                  trigger_reason TEXT NOT NULL,
                  transcript_count INTEGER NOT NULL,
                  extraction_status TEXT NOT NULL,
                  extraction_provider_used TEXT NOT NULL,
                  extraction_fallback_used INTEGER NOT NULL,
                  extraction_fallback_reason TEXT NOT NULL,
                  trace_id TEXT,
                  created_at DATETIME NOT NULL
                );
                CREATE TABLE todos (
                  id TEXT PRIMARY KEY,
                  conversation_session_id TEXT NOT NULL,
                  title TEXT NOT NULL,
                  note TEXT NOT NULL DEFAULT '',
                  status TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'completed')),
                  created_at TEXT NOT NULL,
                  completed_at DATETIME,
                  source_text TEXT,
                  source_audio_id TEXT,
                  speaker_id TEXT,
                  extraction_model_name TEXT NOT NULL DEFAULT '',
                  trace_id TEXT,
                  updated_at DATETIME NOT NULL
                );
                INSERT INTO conversation_sessions (
                  id, merged_text, started_at, ended_at, idle_trigger_seconds, trigger_reason,
                  transcript_count, extraction_status, extraction_provider_used,
                  extraction_fallback_used, extraction_fallback_reason, trace_id, created_at
                ) VALUES (
                  'session_v07_migration', '旧 Todo 状态迁移测试', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP,
                  20, 'manual', 1, 'success', 'minimax_m3', 0, '', 'trace_v07_migration', CURRENT_TIMESTAMP
                );
                INSERT INTO todos (
                  id, conversation_session_id, title, note, status, created_at, source_text, updated_at
                ) VALUES
                  ('todo_v07_pending', 'session_v07_migration', '旧未完成 Todo', '', 'pending', CURRENT_TIMESTAMP, '旧来源', CURRENT_TIMESTAMP),
                  ('todo_v07_completed', 'session_v07_migration', '旧已完成 Todo', '', 'completed', CURRENT_TIMESTAMP, '旧来源', CURRENT_TIMESTAMP);
                "#,
            )
            .expect("应能准备旧 v0.6 Todo 表");
        drop(connection);

        initialize_database(&db_path).expect("应能迁移旧 Todo 状态约束");
        let todos = commands::bootstrap::get_bootstrap_data_payload(&db_path)
            .expect("应能读取迁移后的启动数据")
            .todos;
        assert!(todos
            .iter()
            .any(|todo| todo.id == "todo_v07_pending" && todo.status == "open"));
        assert!(todos
            .iter()
            .any(|todo| todo.id == "todo_v07_completed" && todo.status == "done"));

        let in_progress =
            commands::todo::update_todo_status_payload(&db_path, "todo_v07_pending", "in_progress")
                .expect("Todo 应可进入进行中");
        assert_eq!(in_progress.status, "in_progress");

        let done = commands::todo::update_todo_status_payload(&db_path, "todo_v07_pending", "done")
            .expect("Todo 应可标记完成");
        assert_eq!(done.status, "done");

        let dismissed =
            commands::todo::update_todo_status_payload(&db_path, "todo_v07_pending", "dismissed")
                .expect("Todo 应可忽略");
        assert_eq!(dismissed.status, "dismissed");

        let _ = fs::remove_dir_all(temp_dir);
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
            .expect("再次登记应替换已有 Todo 语义产物");

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
        settings.todo_provider_type = DEFAULT_TODO_PROVIDER_TYPE.into();
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

        let result = commands::model_test::test_model_connection_payload(
            domain::model_test::ModelTestRequest {
                provider: "todo".into(),
                settings,
            },
        )
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

        let result = commands::model_test::test_model_connection_payload(
            domain::model_test::ModelTestRequest {
                provider: "asr".into(),
                settings,
            },
        )
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

        let error = commands::model_test::test_model_connection_payload(
            domain::model_test::ModelTestRequest {
                provider: "embedding".into(),
                settings,
            },
        )
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
        settings.todo_provider_type = DEFAULT_TODO_PROVIDER_TYPE.into();
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
    fn should_clamp_unsupported_provider_inputs_at_settings_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-settings-provider-clamp-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let mut settings = settings_service::load_settings(&connection).expect("应能读取默认设置");
        settings.todo_provider_type = "unsupported_todo_provider".into();
        settings.semantic_provider_type = "unsupported_semantic_provider".into();

        let saved = commands::settings::save_settings_payload(&db_path, settings)
            .expect("settings command boundary 应收敛不支持的 provider 输入");

        assert_eq!(saved.todo_provider_type, DEFAULT_TODO_PROVIDER_TYPE);
        assert_eq!(saved.semantic_provider_type, DEFAULT_SEMANTIC_PROVIDER_TYPE);

        let persisted: (String, String) = connection
            .query_row(
                "SELECT todo_provider_type, semantic_provider_type FROM app_settings WHERE id = 'default'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("应能读取持久化 provider 设置");
        assert_eq!(persisted.0, DEFAULT_TODO_PROVIDER_TYPE);
        assert_eq!(persisted.1, DEFAULT_SEMANTIC_PROVIDER_TYPE);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_desktop_context_command_boundary() {
        let provider_count = providers::provider_catalog().len();
        let context = commands::desktop_context::build_desktop_context(false, provider_count);

        assert_eq!(context.runtime, "tauri");
        assert_eq!(context.platform, std::env::consts::OS);
        assert!(context.recorder_status.contains("录音已停止"));
        assert!(context
            .storage_status
            .contains(&format!("{provider_count} 个 provider")));
        assert!(context.models_status.contains("MiniMax M3"));
        assert!(context.models_status.contains("semantic_m3"));

        let recording_context =
            commands::desktop_context::build_desktop_context(true, provider_count);
        assert!(recording_context.recorder_status.contains("录音中"));
    }

    #[test]
    fn should_expose_bootstrap_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-bootstrap-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");

        let bootstrap = commands::bootstrap::get_bootstrap_data_payload(&db_path)
            .expect("bootstrap command boundary 应能读取启动数据");

        assert_eq!(
            bootstrap.settings.todo_provider_type,
            DEFAULT_TODO_PROVIDER_TYPE
        );
        assert_eq!(bootstrap.settings.semantic_provider_type, "minimax_m3");
        assert!(
            !bootstrap.todos.is_empty(),
            "bootstrap 应返回 demo Todo 以支撑前端首屏"
        );
        assert!(
            !bootstrap.sessions.is_empty(),
            "bootstrap 应返回 demo session 以支撑前端首屏"
        );
        assert!(
            bootstrap.runtime.last_extraction_summary.contains("会话")
                || bootstrap.runtime.last_extraction_summary.contains("暂无")
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_todo_status_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-todo-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");

        let completed = commands::todo::toggle_todo_status_payload(&db_path, "todo_seed_001")
            .expect("todo command boundary 应能完成 Todo");
        assert_eq!(completed.id, "todo_seed_001");
        assert_eq!(completed.status, "done");

        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let completed_at: Option<String> = connection
            .query_row(
                "SELECT completed_at FROM todos WHERE id = 'todo_seed_001'",
                [],
                |row| row.get(0),
            )
            .expect("应能读取完成时间");
        assert!(completed_at.is_some(), "完成 Todo 时应写入 completed_at");

        let reopened = commands::todo::toggle_todo_status_payload(&db_path, "todo_seed_001")
            .expect("todo command boundary 应能重新打开 Todo");
        assert_eq!(reopened.id, "todo_seed_001");
        assert_eq!(reopened.status, "open");

        let reopened_completed_at: Option<String> = connection
            .query_row(
                "SELECT completed_at FROM todos WHERE id = 'todo_seed_001'",
                [],
                |row| row.get(0),
            )
            .expect("应能读取重新打开后的完成时间");
        assert!(
            reopened_completed_at.is_none(),
            "重新打开 Todo 时应清空 completed_at"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_pending_jobs_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-jobs-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");

        let result = commands::jobs::process_pending_jobs_payload(&db_path)
            .expect("jobs command boundary 应能返回处理结果");

        assert_eq!(result.message, "暂无待处理任务");
        assert!(result.latest_session.is_some());
        assert!(
            !result.todos.is_empty(),
            "jobs command 应返回 Todo 列表以刷新前端状态"
        );
        assert!(
            !result.sessions.is_empty(),
            "jobs command 应返回会话列表以刷新前端状态"
        );
        assert!(!result.runtime.runtime_label.trim().is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_flush_session_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-session-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        connection
            .execute(
                "UPDATE conversation_sessions SET created_at = datetime('now', '-20 seconds') WHERE trigger_reason = 'manual'",
                [],
            )
            .expect("应能让示例手动会话离开冷却窗口");

        let session = commands::session::flush_current_session_payload(&db_path)
            .expect("session command boundary 应能手动刷新当前会话");

        assert_eq!(session.trigger_reason, "manual");
        assert_eq!(session.extraction_status, "success");
        assert_eq!(session.extraction_provider_used, "skipped");
        assert!(
            session.merged_text.contains("手动刷新会话"),
            "手动刷新应创建可见的占位会话"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_simulate_audio_slice_command_boundary() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-recording-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");

        initialize_database(&db_path).expect("应能初始化数据库");

        let result = commands::recording::simulate_audio_slice_payload(&db_path, true)
            .expect("recording command boundary 应能写入模拟有效切片");

        assert!(result.message.contains("有效录音切片"));
        assert!(result.latest_session.is_none());
        assert!(!result.runtime.runtime_label.trim().is_empty());

        let connection = open_connection(&db_path).expect("应能打开测试数据库");
        let pending_transcription_jobs: i64 = connection
            .query_row(
                "SELECT COUNT(1) FROM processing_jobs WHERE job_type = 'transcription'",
                [],
                |row| row.get(0),
            )
            .expect("应能读取转写任务数量");
        assert_eq!(pending_transcription_jobs, 1);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_stop_recording_command_boundary_when_idle() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-stop-recording-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let recordings_dir = temp_dir.join("recordings");

        initialize_database(&db_path).expect("应能初始化数据库");
        fs::create_dir_all(&recordings_dir).expect("应能创建测试录音目录");
        let state = AppState {
            db_path,
            recordings_dir,
            recorder: Arc::new(Mutex::new(None)),
        };

        let result = commands::recording::stop_recording_payload(&state)
            .expect("recording command boundary 应能处理空闲停止录音");

        assert_eq!(result.message, "当前没有进行中的录音");
        assert!(result.latest_session.is_some());
        assert!(!result.runtime.runtime_label.trim().is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn should_expose_start_recording_command_boundary_when_already_recording() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-start-recording-command-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let recordings_dir = temp_dir.join("recordings");
        let (stop_tx, _stop_rx) = mpsc::channel::<RecorderControl>();

        initialize_database(&db_path).expect("应能初始化数据库");
        fs::create_dir_all(&recordings_dir).expect("应能创建测试录音目录");
        let state = AppState {
            db_path,
            recordings_dir,
            recorder: Arc::new(Mutex::new(Some(RecordingController {
                stop_tx,
                join_handle: thread::spawn(|| Err("测试占位录音线程不应被 join".into())),
            }))),
        };

        let result = commands::recording::start_recording_payload(&state)
            .expect("recording command boundary 应能处理重复开始录音");

        assert_eq!(result.message, "录音已在进行中");
        assert!(result.latest_session.is_some());
        assert!(!result.runtime.runtime_label.trim().is_empty());
        assert!(
            state.recorder.lock().expect("应能读取录音状态").is_some(),
            "重复开始录音不应清空已有 recorder"
        );

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

    #[test]
    fn should_expose_todo_domain_dto_contract() {
        let todo = domain::todo::TodoDto {
            id: "todo_domain_contract".into(),
            title: "同步 v0.4 domain 边界".into(),
            note: "Todo DTO 应从 lib.rs 迁入 domain::todo".into(),
            status: "open".into(),
            created_at: "2026-06-13 12:00:00".into(),
            conversation_session_id: "session_domain_contract".into(),
            source_text: "请同步 v0.4 domain 边界".into(),
            owner: "我".into(),
            due_at: "2026-06-15 18:00".into(),
            priority: "medium".into(),
            source_span_refs: vec!["transcript_domain_contract".into()],
            candidate_id: "candidate_domain_contract".into(),
        };

        let payload = serde_json::to_value(&todo).expect("Todo domain DTO 应可序列化");

        assert_eq!(payload["id"], "todo_domain_contract");
        assert_eq!(payload["conversationSessionId"], "session_domain_contract");
        assert_eq!(payload["sourceSpanRefs"][0], "transcript_domain_contract");
        assert!(payload.get("conversation_session_id").is_none());
    }

    #[test]
    fn should_expose_session_domain_dto_contract() {
        let session = domain::session::SessionDto {
            id: "session_domain_contract".into(),
            merged_text: "同步 v0.4 session domain 边界".into(),
            started_at: "2026-06-14 09:00:00".into(),
            ended_at: "2026-06-14 09:05:00".into(),
            trigger_reason: "manual".into(),
            extraction_status: "success".into(),
            extraction_provider_used: "minimax_m3".into(),
            extraction_fallback_used: false,
            extraction_fallback_reason: "".into(),
            transcript_count: 1,
            related_todo_ids: vec!["todo_domain_contract".into()],
        };

        let payload = serde_json::to_value(&session).expect("Session domain DTO 应可序列化");

        assert_eq!(payload["id"], "session_domain_contract");
        assert_eq!(payload["relatedTodoIds"][0], "todo_domain_contract");
        assert!(payload.get("related_todo_ids").is_none());
    }

    #[test]
    fn should_expose_runtime_domain_dto_contract() {
        let runtime = domain::runtime::RuntimeStatusDto {
            runtime_label: "Tauri + SQLite".into(),
            current_session_status: "collecting".into(),
            last_slice_at: "2026-06-14 10:00:00".into(),
            last_extraction_at: "2026-06-14 10:01:00".into(),
            last_extraction_summary: "最近一次提取成功".into(),
        };

        let payload = serde_json::to_value(&runtime).expect("Runtime domain DTO 应可序列化");

        assert_eq!(payload["runtimeLabel"], "Tauri + SQLite");
        assert_eq!(payload["currentSessionStatus"], "collecting");
        assert!(payload.get("runtime_label").is_none());
    }

    #[test]
    fn should_expose_settings_domain_dto_contract() {
        let settings = domain::settings::SettingsDto {
            record_enabled: true,
            language: "zh-CN".into(),
            chunk_seconds: 30,
            idle_trigger_seconds: 20,
            provider_mode: "local".into(),
            asr_provider_type: "local_whisperkit".into(),
            speaker_provider_type: "local_speakerkit".into(),
            todo_provider_type: "semantic_m3".into(),
            semantic_provider_type: "minimax_m3".into(),
            embedding_provider_type: "reserved".into(),
            export_provider_type: "local_file".into(),
            asr_submit_url: "https://asr.example.test/submit".into(),
            asr_query_url: "https://asr.example.test/query".into(),
            asr_resource_id: "resource-test".into(),
            asr_model_name: "asr-test".into(),
            asr_api_key_masked: "asr-key-****".into(),
            semantic_base_url: "https://api.minimax.io/v1/responses".into(),
            semantic_model_name: "MiniMax-M3".into(),
            semantic_api_key_masked: "semantic-key-****".into(),
            allow_cloud_fallback: false,
        };

        let payload = serde_json::to_value(&settings).expect("Settings domain DTO 应可序列化");

        assert_eq!(payload["recordEnabled"], true);
        assert_eq!(payload["semanticProviderType"], "minimax_m3");
        assert_eq!(payload["semanticModelName"], "MiniMax-M3");
        assert!(payload.get("record_enabled").is_none());
    }

    #[test]
    fn should_expose_model_test_domain_dto_contract() {
        let request = domain::model_test::ModelTestRequest {
            provider: "todo".into(),
            settings: domain::settings::SettingsDto {
                record_enabled: false,
                language: "zh-CN".into(),
                chunk_seconds: 30,
                idle_trigger_seconds: 20,
                provider_mode: "local".into(),
                asr_provider_type: "local_whisperkit".into(),
                speaker_provider_type: "local_speakerkit".into(),
                todo_provider_type: "semantic_m3".into(),
                semantic_provider_type: "minimax_m3".into(),
                embedding_provider_type: "reserved".into(),
                export_provider_type: "local_file".into(),
                asr_submit_url: "".into(),
                asr_query_url: "".into(),
                asr_resource_id: "".into(),
                asr_model_name: "".into(),
                asr_api_key_masked: "".into(),
                semantic_base_url: "https://api.minimax.io/v1/responses".into(),
                semantic_model_name: "MiniMax-M3".into(),
                semantic_api_key_masked: "".into(),
                allow_cloud_fallback: false,
            },
        };
        let result = domain::model_test::ModelTestResult {
            provider: "todo".into(),
            success: true,
            status_code: 0,
            message: "MiniMax M3 语义 Todo 边界已登记".into(),
            response_excerpt: "semantic_artifacts(type='todo_extraction')".into(),
        };

        let request_payload =
            serde_json::to_value(&request).expect("ModelTest request DTO 应可序列化");
        let result_payload =
            serde_json::to_value(&result).expect("ModelTest result DTO 应可序列化");

        assert_eq!(request_payload["provider"], "todo");
        assert_eq!(
            request_payload["settings"]["semanticProviderType"],
            "minimax_m3"
        );
        assert_eq!(result_payload["statusCode"], 0);
        assert!(result_payload.get("status_code").is_none());
    }

    #[test]
    fn should_expose_desktop_context_domain_dto_contract() {
        let context = domain::desktop::DesktopContext {
            runtime: "tauri".into(),
            platform: "macos".into(),
            recorder_status: "录音已停止，可启动真实麦克风录音".into(),
            storage_status: "SQLite 已接入".into(),
            models_status: "Todo 语义入口已固定为 MiniMax M3".into(),
        };

        let payload = serde_json::to_value(&context).expect("DesktopContext domain DTO 应可序列化");

        assert_eq!(payload["runtime"], "tauri");
        assert_eq!(
            payload["recorderStatus"],
            "录音已停止，可启动真实麦克风录音"
        );
        assert!(payload.get("recorder_status").is_none());
    }

    #[test]
    fn should_expose_bootstrap_domain_dto_contract() {
        let bootstrap = domain::bootstrap::BootstrapData {
            settings: domain::settings::SettingsDto {
                record_enabled: false,
                language: "zh-CN".into(),
                chunk_seconds: 30,
                idle_trigger_seconds: 20,
                provider_mode: "local".into(),
                asr_provider_type: "local_whisperkit".into(),
                speaker_provider_type: "local_speakerkit".into(),
                todo_provider_type: "semantic_m3".into(),
                semantic_provider_type: "minimax_m3".into(),
                embedding_provider_type: "reserved".into(),
                export_provider_type: "local_file".into(),
                asr_submit_url: "".into(),
                asr_query_url: "".into(),
                asr_resource_id: "".into(),
                asr_model_name: "".into(),
                asr_api_key_masked: "".into(),
                semantic_base_url: "https://api.minimax.io/v1/responses".into(),
                semantic_model_name: "MiniMax-M3".into(),
                semantic_api_key_masked: "".into(),
                allow_cloud_fallback: false,
            },
            todos: Vec::new(),
            sessions: Vec::new(),
            runtime: domain::runtime::RuntimeStatusDto {
                runtime_label: "已暂停".into(),
                current_session_status: "idle_waiting".into(),
                last_slice_at: "暂无切片".into(),
                last_extraction_at: "暂无".into(),
                last_extraction_summary: "暂无会话提取记录".into(),
            },
        };

        let payload = serde_json::to_value(&bootstrap).expect("Bootstrap domain DTO 应可序列化");

        assert_eq!(payload["settings"]["semanticProviderType"], "minimax_m3");
        assert!(payload["todos"]
            .as_array()
            .expect("todos 应为数组")
            .is_empty());
        assert_eq!(payload["runtime"]["runtimeLabel"], "已暂停");
        assert!(payload.get("runtime_label").is_none());
    }

    #[test]
    fn should_expose_recording_action_domain_dto_contract() {
        let result = domain::recording::RecordingActionResult {
            message: "录音已在进行中".into(),
            runtime: domain::runtime::RuntimeStatusDto {
                runtime_label: "录音中".into(),
                current_session_status: "collecting".into(),
                last_slice_at: "暂无切片".into(),
                last_extraction_at: "暂无".into(),
                last_extraction_summary: "暂无会话提取记录".into(),
            },
            latest_session: None,
        };

        let payload =
            serde_json::to_value(&result).expect("RecordingActionResult domain DTO 应可序列化");

        assert_eq!(payload["message"], "录音已在进行中");
        assert_eq!(payload["runtime"]["runtimeLabel"], "录音中");
        assert!(payload["latestSession"].is_null());
        assert!(payload.get("latest_session").is_none());
    }

    #[test]
    fn should_expose_processing_action_domain_dto_contract() {
        let result = domain::processing::ProcessingActionResult {
            message: "暂无待处理任务".into(),
            runtime: domain::runtime::RuntimeStatusDto {
                runtime_label: "已暂停".into(),
                current_session_status: "idle_waiting".into(),
                last_slice_at: "暂无切片".into(),
                last_extraction_at: "暂无".into(),
                last_extraction_summary: "暂无会话提取记录".into(),
            },
            latest_session: None,
            todos: Vec::new(),
            sessions: Vec::new(),
        };

        let payload =
            serde_json::to_value(&result).expect("ProcessingActionResult domain DTO 应可序列化");

        assert_eq!(payload["message"], "暂无待处理任务");
        assert_eq!(payload["runtime"]["runtimeLabel"], "已暂停");
        assert!(payload["latestSession"].is_null());
        assert!(payload["todos"]
            .as_array()
            .expect("todos 应为数组")
            .is_empty());
        assert!(payload.get("latest_session").is_none());
    }

    #[test]
    fn should_expose_transcript_domain_record_contract() {
        let transcript = domain::transcript::TranscriptRecord {
            id: "transcript_domain_contract".into(),
            text: "请把转写记录移动到 domain::transcript".into(),
            trace_id: "trace_transcript_domain_contract".into(),
        };

        assert_eq!(transcript.id, "transcript_domain_contract");
        assert_eq!(transcript.trace_id, "trace_transcript_domain_contract");
        assert!(
            format!("{transcript:?}").contains("transcript_domain_contract"),
            "TranscriptRecord 应保留 Debug 能力，便于测试和诊断"
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
