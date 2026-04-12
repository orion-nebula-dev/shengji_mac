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
struct SettingsDto {
    record_enabled: bool,
    language: String,
    chunk_seconds: i64,
    idle_trigger_seconds: i64,
    provider_mode: String,
    asr_submit_url: String,
    asr_query_url: String,
    asr_resource_id: String,
    asr_model_name: String,
    asr_api_key_masked: String,
    todo_base_url: String,
    todo_model_name: String,
    todo_api_key_masked: String,
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
struct ModelTestRequest {
    provider: String,
    settings: SettingsDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelTestResult {
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

fn open_connection(db_path: &PathBuf) -> Result<Connection, String> {
    Connection::open(db_path).map_err(|error| format!("打开数据库失败: {error}"))
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

fn ensure_app_settings_columns(connection: &Connection) -> Result<(), String> {
    let mut columns = Vec::new();
    let mut statement = connection
        .prepare("PRAGMA table_info(app_settings)")
        .map_err(|error| format!("读取设置表结构失败: {error}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("查询设置表字段失败: {error}"))?;

    for column in rows {
        columns.push(column.map_err(|error| format!("读取设置字段失败: {error}"))?);
    }

    for (name, sql) in [
        (
            "asr_submit_url",
            "ALTER TABLE app_settings ADD COLUMN asr_submit_url TEXT NOT NULL DEFAULT ''",
        ),
        (
            "asr_query_url",
            "ALTER TABLE app_settings ADD COLUMN asr_query_url TEXT NOT NULL DEFAULT ''",
        ),
        (
            "asr_resource_id",
            "ALTER TABLE app_settings ADD COLUMN asr_resource_id TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute(sql, [])
                .map_err(|error| format!("补充设置字段 {name} 失败: {error}"))?;
        }
    }

    connection
        .execute(
            r#"
            UPDATE app_settings
            SET
              asr_query_url = CASE
                WHEN asr_query_url = '' THEN asr_base_url
                ELSE asr_query_url
              END,
              asr_submit_url = CASE
                WHEN asr_submit_url = '' AND asr_base_url LIKE '%/query' THEN REPLACE(asr_base_url, '/query', '/submit')
                WHEN asr_submit_url = '' THEN asr_base_url
                ELSE asr_submit_url
              END,
              asr_resource_id = CASE
                WHEN asr_resource_id = '' THEN asr_model_name
                ELSE asr_resource_id
              END,
              asr_model_name = CASE
                WHEN asr_model_name LIKE 'volc.%' THEN 'bigmodel'
                ELSE asr_model_name
              END
            WHERE id = 'default'
            "#,
            [],
        )
        .map_err(|error| format!("回填 ASR 设置字段失败: {error}"))?;

    Ok(())
}

fn initialize_database(db_path: &PathBuf) -> Result<(), String> {
    let parent_dir = db_path
        .parent()
        .ok_or_else(|| "数据库目录无效".to_string())?;
    fs::create_dir_all(parent_dir).map_err(|error| format!("创建数据库目录失败: {error}"))?;

    let connection = open_connection(db_path)?;
    connection
        .execute_batch(
            r#"
      PRAGMA foreign_keys = ON;

      CREATE TABLE IF NOT EXISTS app_settings (
        id TEXT PRIMARY KEY,
        record_enabled INTEGER NOT NULL DEFAULT 0 CHECK (record_enabled IN (0, 1)),
        language TEXT NOT NULL DEFAULT 'zh-CN',
        chunk_seconds INTEGER NOT NULL DEFAULT 30 CHECK (chunk_seconds > 0),
        idle_trigger_seconds INTEGER NOT NULL DEFAULT 20 CHECK (idle_trigger_seconds > 0),
        provider_mode TEXT NOT NULL DEFAULT 'cloud' CHECK (provider_mode IN ('cloud', 'local')),
        asr_base_url TEXT NOT NULL DEFAULT '',
        asr_submit_url TEXT NOT NULL DEFAULT '',
        asr_query_url TEXT NOT NULL DEFAULT '',
        asr_resource_id TEXT NOT NULL DEFAULT '',
        asr_model_name TEXT NOT NULL DEFAULT '',
        asr_api_key_ref TEXT NOT NULL DEFAULT '',
        todo_base_url TEXT NOT NULL DEFAULT '',
        todo_model_name TEXT NOT NULL DEFAULT '',
        todo_api_key_ref TEXT NOT NULL DEFAULT '',
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
      );

      CREATE TABLE IF NOT EXISTS audio_segments (
        id TEXT PRIMARY KEY,
        file_path TEXT NOT NULL,
        started_at TEXT NOT NULL,
        ended_at DATETIME NOT NULL,
        duration_ms INTEGER NOT NULL DEFAULT 0,
        sample_rate INTEGER NOT NULL DEFAULT 16000,
        channels INTEGER NOT NULL DEFAULT 1,
        has_effective_voice INTEGER NOT NULL DEFAULT 0 CHECK (has_effective_voice IN (0, 1)),
        voice_energy_score REAL,
        processing_status TEXT NOT NULL DEFAULT 'pending'
          CHECK (processing_status IN ('pending', 'transcribed', 'failed', 'skipped')),
        trace_id TEXT,
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
      );

      CREATE TABLE IF NOT EXISTS conversation_sessions (
        id TEXT PRIMARY KEY,
        merged_text TEXT NOT NULL,
        started_at TEXT NOT NULL,
        ended_at TEXT NOT NULL,
        idle_trigger_seconds INTEGER NOT NULL CHECK (idle_trigger_seconds > 0),
        trigger_reason TEXT NOT NULL
          CHECK (trigger_reason IN ('idle_timeout', 'manual', 'forced_flush')),
        transcript_count INTEGER NOT NULL DEFAULT 0 CHECK (transcript_count >= 0),
        extraction_status TEXT NOT NULL DEFAULT 'pending'
          CHECK (extraction_status IN ('pending', 'success', 'failed')),
        trace_id TEXT,
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
      );

      CREATE TABLE IF NOT EXISTS transcript_segments (
        id TEXT PRIMARY KEY,
        audio_segment_id TEXT NOT NULL,
        conversation_session_id TEXT,
        text TEXT NOT NULL,
        language TEXT NOT NULL DEFAULT 'zh-CN',
        status TEXT NOT NULL DEFAULT 'success'
          CHECK (status IN ('pending', 'success', 'failed')),
        provider_model_name TEXT NOT NULL DEFAULT '',
        trace_id TEXT,
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (audio_segment_id) REFERENCES audio_segments(id) ON DELETE CASCADE,
        FOREIGN KEY (conversation_session_id) REFERENCES conversation_sessions(id) ON DELETE SET NULL
      );

      CREATE TABLE IF NOT EXISTS todos (
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
        updated_at DATETIME NOT NULL,
        FOREIGN KEY (conversation_session_id) REFERENCES conversation_sessions(id) ON DELETE CASCADE
      );

      CREATE TABLE IF NOT EXISTS processing_jobs (
        id TEXT PRIMARY KEY,
        job_type TEXT NOT NULL
          CHECK (job_type IN ('transcription', 'aggregation', 'todo_extraction')),
        target_id TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending'
          CHECK (status IN ('pending', 'running', 'success', 'failed')),
        retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
        max_retry_count INTEGER NOT NULL DEFAULT 3 CHECK (max_retry_count >= 0),
        error_message TEXT,
        trace_id TEXT,
        started_at DATETIME,
        finished_at DATETIME,
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
      );

      CREATE INDEX IF NOT EXISTS idx_audio_segments_created_at
        ON audio_segments(created_at DESC);
      CREATE INDEX IF NOT EXISTS idx_conversation_sessions_created_at
        ON conversation_sessions(created_at DESC);
      CREATE INDEX IF NOT EXISTS idx_conversation_sessions_status
        ON conversation_sessions(extraction_status);
      CREATE INDEX IF NOT EXISTS idx_transcript_segments_audio_segment
        ON transcript_segments(audio_segment_id);
      CREATE INDEX IF NOT EXISTS idx_transcript_segments_session
        ON transcript_segments(conversation_session_id);
      CREATE INDEX IF NOT EXISTS idx_todos_status
        ON todos(status);
      CREATE INDEX IF NOT EXISTS idx_todos_created_at
        ON todos(created_at DESC);
      CREATE INDEX IF NOT EXISTS idx_processing_jobs_status
        ON processing_jobs(status);
      "#,
        )
        .map_err(|error| format!("初始化表结构失败: {error}"))?;

    ensure_app_settings_columns(&connection)?;

    connection
        .execute(
            r#"
      INSERT OR IGNORE INTO app_settings (
        id,
        record_enabled,
        language,
        chunk_seconds,
        idle_trigger_seconds,
        provider_mode,
        asr_base_url,
        asr_submit_url,
        asr_query_url,
        asr_resource_id,
        asr_model_name,
        asr_api_key_ref,
        todo_base_url,
        todo_model_name,
        todo_api_key_ref
      ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
      "#,
            params![
                "default",
                0,
                "zh-CN",
                30,
                20,
                "cloud",
                "https://api.example.com/asr/query",
                "https://api.example.com/asr/submit",
                "https://api.example.com/asr/query",
                "volc.seedasr.auc",
                "bigmodel",
                "sk-asr-****",
                "https://api.example.com/todo",
                "todo-model-v1",
                "sk-todo-****"
            ],
        )
        .map_err(|error| format!("初始化默认设置失败: {error}"))?;

    seed_demo_data(&connection)?;
    Ok(())
}

fn seed_demo_data(connection: &Connection) -> Result<(), String> {
    let todo_count: i64 = connection
        .query_row("SELECT COUNT(1) FROM todos", [], |row| row.get(0))
        .map_err(|error| format!("读取 Todo 数量失败: {error}"))?;

    if todo_count > 0 {
        return Ok(());
    }

    let session_id = "session_seed_001";
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
        trace_id
      ) VALUES (
        ?1,
        '这是初始化示例会话，用于展示 Todo 工作台骨架。',
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP,
        20,
        'manual',
        1,
        'success',
        'trace_seed_001'
      )
      "#,
            params![session_id],
        )
        .map_err(|error| format!("初始化示例会话失败: {error}"))?;

    connection
        .execute(
            r#"
      INSERT OR IGNORE INTO todos (
        id,
        conversation_session_id,
        title,
        note,
        status,
        created_at,
        source_text,
        extraction_model_name,
        trace_id,
        updated_at
      ) VALUES (
        'todo_seed_001',
        ?1,
        '确认双模型配置',
        '补全语音转写模型和 Todo 提取模型的 API 信息',
        'pending',
        CURRENT_TIMESTAMP,
        '请把两个模型的地址、模型名和密钥都配置好。',
        'todo-model-v1',
        'trace_seed_001',
        CURRENT_TIMESTAMP
      )
      "#,
            params![session_id],
        )
        .map_err(|error| format!("初始化示例 Todo 失败: {error}"))?;

    Ok(())
}

fn query_settings(connection: &Connection) -> Result<SettingsDto, String> {
    connection
        .query_row(
            r#"
      SELECT
        record_enabled,
        language,
        chunk_seconds,
        idle_trigger_seconds,
        provider_mode,
        asr_submit_url,
        asr_query_url,
        asr_resource_id,
        asr_model_name,
        asr_api_key_ref,
        todo_base_url,
        todo_model_name,
        todo_api_key_ref
      FROM app_settings
      WHERE id = 'default'
      "#,
            [],
            |row| {
                Ok(SettingsDto {
                    record_enabled: row.get::<_, i64>(0)? == 1,
                    language: row.get(1)?,
                    chunk_seconds: row.get(2)?,
                    idle_trigger_seconds: row.get(3)?,
                    provider_mode: row.get(4)?,
                    asr_submit_url: row.get(5)?,
                    asr_query_url: row.get(6)?,
                    asr_resource_id: row.get(7)?,
                    asr_model_name: row.get(8)?,
                    asr_api_key_masked: row.get(9)?,
                    todo_base_url: row.get(10)?,
                    todo_model_name: row.get(11)?,
                    todo_api_key_masked: row.get(12)?,
                })
            },
        )
        .map_err(|error| format!("读取设置失败: {error}"))
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
                row.get::<_, i64>(6)?,
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
    let settings = query_settings(connection)?;
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

fn set_record_enabled(connection: &Connection, enabled: bool) -> Result<(), String> {
    connection
    .execute(
      "UPDATE app_settings SET record_enabled = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 'default'",
      params![if enabled { 1 } else { 0 }],
    )
    .map_err(|error| format!("更新录音状态失败: {error}"))?;
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
    let settings = query_settings(connection)?;
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
        trace_id,
        created_at
      ) VALUES (?1, ?2, ?3, ?4, ?5, 'idle_timeout', 0, 'pending', ?6, CURRENT_TIMESTAMP)
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

fn test_todo_provider(settings: &SettingsDto) -> Result<ModelTestResult, String> {
    if settings.todo_base_url.trim().is_empty()
        || settings.todo_model_name.trim().is_empty()
        || settings.todo_api_key_masked.trim().is_empty()
    {
        return Ok(ModelTestResult {
            provider: "todo".into(),
            success: false,
            status_code: 0,
            message: "Todo 提取模型配置不完整".into(),
            response_excerpt: "".into(),
        });
    }

    let client = build_http_client()?;
    let response = client
        .post(settings.todo_base_url.trim())
        .bearer_auth(settings.todo_api_key_masked.trim())
        .json(&serde_json::json!({
            "model": settings.todo_model_name.trim(),
            "input": "请只回复ok"
        }))
        .send()
        .map_err(|error| format!("Todo 模型测试请求失败: {error}"))?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .unwrap_or_else(|_| "读取响应正文失败".to_string());

    Ok(ModelTestResult {
        provider: "todo".into(),
        success: status_code / 100 == 2,
        status_code,
        message: if status_code / 100 == 2 {
            "Todo 提取模型测试成功".into()
        } else {
            format!("Todo 提取模型测试失败，HTTP {status_code}")
        },
        response_excerpt: clip_text(&body, 400),
    })
}

fn extract_output_text(value: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    if let Some(output) = value.get("output").and_then(|entry| entry.as_array()) {
        for item in output {
            if item.get("type").and_then(|entry| entry.as_str()) == Some("message") {
                if let Some(content) = item.get("content").and_then(|entry| entry.as_array()) {
                    for part in content {
                        if let Some(text) = part.get("text").and_then(|entry| entry.as_str()) {
                            parts.push(text.to_string());
                        }
                    }
                }
            }
        }
    }
    parts.join("\n")
}

fn extract_json_array(input: &str) -> Result<Vec<serde_json::Value>, String> {
    if let Ok(value) = serde_json::from_str::<Vec<serde_json::Value>>(input) {
        return Ok(value);
    }

    if let (Some(start), Some(end)) = (input.find('['), input.rfind(']')) {
        let candidate = &input[start..=end];
        if let Ok(value) = serde_json::from_str::<Vec<serde_json::Value>>(candidate) {
            return Ok(value);
        }
    }

    Err("无法从模型响应中解析 Todo JSON 数组".to_string())
}

fn transcribe_audio_file(settings: &SettingsDto, file_path: &str) -> Result<String, String> {
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
              trace_id,
              created_at
            ) VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?3, 'forced_flush', 1, 'pending', ?4, CURRENT_TIMESTAMP)
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

fn generate_todos_for_session(
    connection: &Connection,
    settings: &SettingsDto,
    session_id: &str,
) -> Result<usize, String> {
    let (merged_text, trigger_reason, transcript_count): (String, String, i64) = connection
        .query_row(
            "SELECT merged_text, trigger_reason, transcript_count FROM conversation_sessions WHERE id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| format!("读取会话文稿失败: {error}"))?;

    let merged_text = merged_text.trim().to_string();
    if merged_text.is_empty()
        || (transcript_count == 0
            && (trigger_reason == "manual" || is_placeholder_session_text(&merged_text)))
    {
        connection
            .execute(
                "UPDATE conversation_sessions SET extraction_status = 'success' WHERE id = ?1",
                params![session_id],
            )
            .map_err(|error| format!("更新空会话状态失败: {error}"))?;
        return Ok(0);
    }

    let client = build_http_client()?;
    let mut last_error = String::new();
    let body = loop {
        let mut should_break = true;
        let mut response_body = String::new();

        for attempt in 1..=HTTP_MAX_RETRY_ATTEMPTS {
            let response = client
                .post(settings.todo_base_url.trim())
                .bearer_auth(settings.todo_api_key_masked.trim())
                .json(&serde_json::json!({
                    "model": settings.todo_model_name.trim(),
                    "input": [
                        {
                            "role": "system",
                            "content": "你是 Todo 提取助手。请从给定中文文稿中提取 todos，严格只返回 JSON 数组。每个元素包含 title 和 note 两个字符串字段，不要输出任何额外文字。没有待办时返回 []。"
                        },
                        {
                            "role": "user",
                            "content": format!("请从下面文稿中提取 Todo：\n{merged_text}")
                        }
                    ]
                }))
                .send();

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    last_error = format!("调用 Todo 提取模型失败: {error}");
                    if attempt < HTTP_MAX_RETRY_ATTEMPTS {
                        sleep_before_retry(attempt);
                        continue;
                    }
                    break;
                }
            };

            let status = response.status().as_u16();
            response_body = response
                .text()
                .unwrap_or_else(|_| "读取 Todo 模型响应失败".to_string());
            if status / 100 == 2 {
                should_break = false;
                break;
            }

            last_error = format!(
                "Todo 提取模型返回 HTTP {status}: {}",
                clip_text(&response_body, 300)
            );
            if should_retry_http_status(status) && attempt < HTTP_MAX_RETRY_ATTEMPTS {
                sleep_before_retry(attempt);
                continue;
            }
            break;
        }

        if should_break {
            return Err(last_error);
        }
        break response_body;
    };

    let payload: serde_json::Value =
        serde_json::from_str(&body).map_err(|error| format!("解析 Todo 模型响应失败: {error}"))?;
    let output_text = extract_output_text(&payload);
    let todos = extract_json_array(&output_text)?;

    connection
        .execute(
            "DELETE FROM todos WHERE conversation_session_id = ?1",
            params![session_id],
        )
        .map_err(|error| format!("清理旧 Todo 失败: {error}"))?;

    let mut inserted = 0_usize;
    for (index, item) in todos.into_iter().enumerate() {
        let title = item
            .get("title")
            .and_then(|entry| entry.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let note = item
            .get("note")
            .and_then(|entry| entry.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if title.is_empty() {
            continue;
        }

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
                  updated_at
                ) VALUES (?1, ?2, ?3, ?4, 'pending', CURRENT_TIMESTAMP, ?5, ?6, ?7, CURRENT_TIMESTAMP)
                "#,
                params![
                    format!("todo_generated_{}_{}", current_timestamp_label(), index),
                    session_id,
                    title,
                    note,
                    merged_text,
                    settings.todo_model_name.as_str(),
                    format!("trace_todo_{}_{}", session_id, index)
                ],
            )
            .map_err(|error| format!("写入 Todo 失败: {error}"))?;
        inserted += 1;
    }

    connection
        .execute(
            "UPDATE conversation_sessions SET extraction_status = 'success' WHERE id = ?1",
            params![session_id],
        )
        .map_err(|error| format!("更新会话提取状态失败: {error}"))?;

    Ok(inserted)
}

fn process_pending_jobs_internal(connection: &Connection) -> Result<String, String> {
    let settings = query_settings(connection)?;
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

        match generate_todos_for_session(connection, &settings, &session_id) {
            Ok(count) => {
                update_processing_job(connection, &job_id, "success", None)?;
                summary.push(format!("已从会话 {session_id} 生成 {count} 条 Todo"));
            }
            Err(error) => {
                connection
                    .execute(
                        "UPDATE conversation_sessions SET extraction_status = 'failed' WHERE id = ?1",
                        params![session_id.as_str()],
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
            "ASR 提交与查询测试成功".into()
        } else {
            format!("ASR 查询请求失败，HTTP {status_code}")
        },
        response_excerpt: clip_text(&body, 400),
    })
}

#[tauri::command]
fn test_model_connection(payload: ModelTestRequest) -> Result<ModelTestResult, String> {
    match payload.provider.as_str() {
        "todo" => test_todo_provider(&payload.settings),
        "asr" => test_asr_provider(&payload.settings),
        other => Err(format!("不支持的模型测试类型: {other}")),
    }
}

#[tauri::command]
fn get_desktop_context(state: tauri::State<'_, AppState>) -> Result<DesktopContext, String> {
    let recording = is_recording(&state)?;

    Ok(DesktopContext {
        runtime: "tauri".into(),
        platform: std::env::consts::OS.into(),
        recorder_status: if recording {
            "真实麦克风录音中".into()
        } else {
            "录音已停止，可启动真实麦克风录音".into()
        },
        storage_status: "SQLite 已接入 settings / audio_segments / sessions / todos".into(),
        models_status: "双模型链路未接入，当前先打通真实录音与切片落库".into(),
    })
}

#[tauri::command]
fn get_bootstrap_data(state: tauri::State<'_, AppState>) -> Result<BootstrapData, String> {
    let connection = open_connection(&state.db_path)?;
    Ok(BootstrapData {
        settings: query_settings(&connection)?,
        todos: query_todos(&connection)?,
        sessions: query_sessions(&connection)?,
        runtime: query_runtime_status(&connection)?,
    })
}

#[tauri::command]
fn save_settings(
    payload: SettingsDto,
    state: tauri::State<'_, AppState>,
) -> Result<SettingsDto, String> {
    let connection = open_connection(&state.db_path)?;
    connection
        .execute(
            r#"
      UPDATE app_settings
      SET
        record_enabled = ?1,
        language = ?2,
        chunk_seconds = ?3,
        idle_trigger_seconds = ?4,
        provider_mode = ?5,
        asr_base_url = ?6,
        asr_submit_url = ?7,
        asr_query_url = ?8,
        asr_resource_id = ?9,
        asr_model_name = ?10,
        asr_api_key_ref = ?11,
        todo_base_url = ?12,
        todo_model_name = ?13,
        todo_api_key_ref = ?14,
        updated_at = CURRENT_TIMESTAMP
      WHERE id = 'default'
      "#,
            params![
                if payload.record_enabled { 1 } else { 0 },
                payload.language,
                payload.chunk_seconds,
                payload.idle_trigger_seconds,
                payload.provider_mode,
                payload.asr_query_url,
                payload.asr_submit_url,
                payload.asr_query_url,
                payload.asr_resource_id,
                payload.asr_model_name,
                payload.asr_api_key_masked,
                payload.todo_base_url,
                payload.todo_model_name,
                payload.todo_api_key_masked,
            ],
        )
        .map_err(|error| format!("保存设置失败: {error}"))?;

    query_settings(&connection)
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
        trace_id,
        created_at
      ) VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 20, 'manual', 0, 'pending', ?3, CURRENT_TIMESTAMP)
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
    set_record_enabled(&connection, true)?;
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
    set_record_enabled(&connection, false)?;
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
    let settings = query_settings(&connection)?;
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
            save_settings,
            test_model_connection,
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
