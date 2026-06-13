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

mod domain;
mod providers;

use providers::semantic::legacy_local_llm;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    recordings_dir: PathBuf,
    models_dir: PathBuf,
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
    todo_base_url: String,
    todo_model_name: String,
    todo_api_key_masked: String,
    local_todo_model_version: String,
    allow_cloud_fallback: bool,
    local_todo_runtime_status: String,
    local_todo_last_health_check_at: String,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalTodoRuntimeStatusDto {
    provider_type: String,
    model_version: String,
    runtime_status: String,
    last_health_check_at: String,
    fallback_enabled: bool,
    message: String,
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
const EMBEDDED_TODO_MODEL_VERSION: &str = legacy_local_llm::MODEL_VERSION;
const DEFAULT_ASR_PROVIDER_TYPE: &str = "local_whisperkit";
const DEFAULT_SPEAKER_PROVIDER_TYPE: &str = "local_speakerkit";
const DEFAULT_SEMANTIC_PROVIDER_TYPE: &str = "minimax_m3";
const DEFAULT_EMBEDDING_PROVIDER_TYPE: &str = "reserved";
const DEFAULT_EXPORT_PROVIDER_TYPE: &str = "local_file";
const DEFAULT_TODO_PROVIDER_TYPE: &str = "semantic_m3";
const LEGACY_TODO_PROVIDER_TYPE: &str = "legacy_local_llm";
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
    match provider_type.trim() {
        "" | "semantic_m3" => DEFAULT_TODO_PROVIDER_TYPE.to_string(),
        "embedded_local" | "legacy_local_llm" => LEGACY_TODO_PROVIDER_TYPE.to_string(),
        "cloud" => "cloud".to_string(),
        other => other.to_string(),
    }
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
            "asr_base_url",
            "ALTER TABLE app_settings ADD COLUMN asr_base_url TEXT NOT NULL DEFAULT ''",
        ),
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
        (
            "asr_model_name",
            "ALTER TABLE app_settings ADD COLUMN asr_model_name TEXT NOT NULL DEFAULT ''",
        ),
        (
            "asr_api_key_ref",
            "ALTER TABLE app_settings ADD COLUMN asr_api_key_ref TEXT NOT NULL DEFAULT ''",
        ),
        (
            "asr_provider_type",
            "ALTER TABLE app_settings ADD COLUMN asr_provider_type TEXT NOT NULL DEFAULT 'local_whisperkit'",
        ),
        (
            "speaker_provider_type",
            "ALTER TABLE app_settings ADD COLUMN speaker_provider_type TEXT NOT NULL DEFAULT 'local_speakerkit'",
        ),
        (
            "todo_provider_type",
            "ALTER TABLE app_settings ADD COLUMN todo_provider_type TEXT NOT NULL DEFAULT 'semantic_m3'",
        ),
        (
            "semantic_provider_type",
            "ALTER TABLE app_settings ADD COLUMN semantic_provider_type TEXT NOT NULL DEFAULT 'minimax_m3'",
        ),
        (
            "embedding_provider_type",
            "ALTER TABLE app_settings ADD COLUMN embedding_provider_type TEXT NOT NULL DEFAULT 'reserved'",
        ),
        (
            "export_provider_type",
            "ALTER TABLE app_settings ADD COLUMN export_provider_type TEXT NOT NULL DEFAULT 'local_file'",
        ),
        (
            "local_todo_model_version",
            "ALTER TABLE app_settings ADD COLUMN local_todo_model_version TEXT NOT NULL DEFAULT ''",
        ),
        (
            "todo_base_url",
            "ALTER TABLE app_settings ADD COLUMN todo_base_url TEXT NOT NULL DEFAULT ''",
        ),
        (
            "todo_model_name",
            "ALTER TABLE app_settings ADD COLUMN todo_model_name TEXT NOT NULL DEFAULT ''",
        ),
        (
            "todo_api_key_ref",
            "ALTER TABLE app_settings ADD COLUMN todo_api_key_ref TEXT NOT NULL DEFAULT ''",
        ),
        (
            "semantic_base_url",
            "ALTER TABLE app_settings ADD COLUMN semantic_base_url TEXT NOT NULL DEFAULT 'https://api.minimax.io/v1/responses'",
        ),
        (
            "semantic_model_name",
            "ALTER TABLE app_settings ADD COLUMN semantic_model_name TEXT NOT NULL DEFAULT 'MiniMax-M3'",
        ),
        (
            "semantic_api_key_ref",
            "ALTER TABLE app_settings ADD COLUMN semantic_api_key_ref TEXT NOT NULL DEFAULT ''",
        ),
        (
            "allow_cloud_fallback",
            "ALTER TABLE app_settings ADD COLUMN allow_cloud_fallback INTEGER NOT NULL DEFAULT 1",
        ),
        (
            "local_todo_runtime_status",
            "ALTER TABLE app_settings ADD COLUMN local_todo_runtime_status TEXT NOT NULL DEFAULT 'not_ready'",
        ),
        (
            "local_todo_last_health_check_at",
            "ALTER TABLE app_settings ADD COLUMN local_todo_last_health_check_at TEXT NOT NULL DEFAULT ''",
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
              END,
              asr_provider_type = CASE
                WHEN TRIM(asr_provider_type) = '' OR asr_provider_type = 'local' THEN 'local_whisperkit'
                WHEN asr_provider_type = 'cloud' THEN 'cloud_volc'
                ELSE asr_provider_type
              END,
              speaker_provider_type = CASE
                WHEN TRIM(speaker_provider_type) = '' THEN 'local_speakerkit'
                ELSE speaker_provider_type
              END,
              todo_provider_type = CASE
                WHEN TRIM(todo_provider_type) = '' THEN 'semantic_m3'
                WHEN todo_provider_type = 'embedded_local' THEN 'legacy_local_llm'
                ELSE todo_provider_type
              END,
              semantic_provider_type = CASE
                WHEN TRIM(semantic_provider_type) = '' THEN 'minimax_m3'
                ELSE semantic_provider_type
              END,
              embedding_provider_type = CASE
                WHEN TRIM(embedding_provider_type) = '' THEN 'reserved'
                ELSE embedding_provider_type
              END,
              export_provider_type = CASE
                WHEN TRIM(export_provider_type) = '' THEN 'local_file'
                ELSE export_provider_type
              END,
              semantic_base_url = CASE
                WHEN TRIM(semantic_base_url) = '' THEN 'https://api.minimax.io/v1/responses'
                ELSE semantic_base_url
              END,
              semantic_model_name = CASE
                WHEN TRIM(semantic_model_name) = '' THEN 'MiniMax-M3'
                ELSE semantic_model_name
              END,
              local_todo_model_version = CASE
                WHEN TRIM(local_todo_model_version) = '' OR local_todo_model_version = 'todo-embedded-v1' THEN ?
                ELSE local_todo_model_version
              END,
              allow_cloud_fallback = CASE
                WHEN allow_cloud_fallback IS NULL THEN 1
                ELSE allow_cloud_fallback
              END,
              local_todo_runtime_status = CASE
                WHEN TRIM(local_todo_runtime_status) = '' THEN 'not_ready'
                ELSE local_todo_runtime_status
              END,
              local_todo_last_health_check_at = CASE
                WHEN local_todo_last_health_check_at IS NULL THEN ''
                ELSE local_todo_last_health_check_at
              END
            WHERE id = 'default'
            "#,
            params![EMBEDDED_TODO_MODEL_VERSION],
        )
        .map_err(|error| format!("回填 ASR 设置字段失败: {error}"))?;

    Ok(())
}

fn ensure_conversation_sessions_columns(connection: &Connection) -> Result<(), String> {
    let mut columns = Vec::new();
    let mut statement = connection
        .prepare("PRAGMA table_info(conversation_sessions)")
        .map_err(|error| format!("读取会话表结构失败: {error}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("查询会话表字段失败: {error}"))?;

    for column in rows {
        columns.push(column.map_err(|error| format!("读取会话字段失败: {error}"))?);
    }

    for (name, sql) in [
        (
            "extraction_provider_used",
            "ALTER TABLE conversation_sessions ADD COLUMN extraction_provider_used TEXT NOT NULL DEFAULT ''",
        ),
        (
            "extraction_fallback_used",
            "ALTER TABLE conversation_sessions ADD COLUMN extraction_fallback_used INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "extraction_fallback_reason",
            "ALTER TABLE conversation_sessions ADD COLUMN extraction_fallback_reason TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute(sql, [])
                .map_err(|error| format!("补充会话字段 {name} 失败: {error}"))?;
        }
    }

    connection
        .execute(
            r#"
            UPDATE conversation_sessions
            SET
              extraction_provider_used = CASE
                WHEN TRIM(extraction_provider_used) = '' THEN 'unknown'
                ELSE extraction_provider_used
              END,
              extraction_fallback_used = CASE
                WHEN extraction_fallback_used IS NULL THEN 0
                ELSE extraction_fallback_used
              END,
              extraction_fallback_reason = CASE
                WHEN extraction_fallback_reason IS NULL THEN ''
                ELSE extraction_fallback_reason
              END
            "#,
            [],
        )
        .map_err(|error| format!("回填会话提取标记失败: {error}"))?;

    Ok(())
}

fn ensure_model_invocations_columns(connection: &Connection) -> Result<(), String> {
    let mut columns = Vec::new();
    let mut statement = connection
        .prepare("PRAGMA table_info(model_invocations)")
        .map_err(|error| format!("读取模型调用表结构失败: {error}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("查询模型调用表字段失败: {error}"))?;

    for column in rows {
        columns.push(column.map_err(|error| format!("读取模型调用字段失败: {error}"))?);
    }

    for (name, sql) in [
        (
            "input_tokens",
            "ALTER TABLE model_invocations ADD COLUMN input_tokens INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "output_tokens",
            "ALTER TABLE model_invocations ADD COLUMN output_tokens INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "duration_ms",
            "ALTER TABLE model_invocations ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "estimated_cost_microunits",
            "ALTER TABLE model_invocations ADD COLUMN estimated_cost_microunits INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "currency",
            "ALTER TABLE model_invocations ADD COLUMN currency TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute(sql, [])
                .map_err(|error| format!("补充模型调用字段 {name} 失败: {error}"))?;
        }
    }

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
        provider_mode TEXT NOT NULL DEFAULT 'local' CHECK (provider_mode IN ('cloud', 'local')),
        asr_provider_type TEXT NOT NULL DEFAULT 'local_whisperkit',
        speaker_provider_type TEXT NOT NULL DEFAULT 'local_speakerkit',
        todo_provider_type TEXT NOT NULL DEFAULT 'semantic_m3',
        semantic_provider_type TEXT NOT NULL DEFAULT 'minimax_m3',
        embedding_provider_type TEXT NOT NULL DEFAULT 'reserved',
        export_provider_type TEXT NOT NULL DEFAULT 'local_file',
        asr_base_url TEXT NOT NULL DEFAULT '',
        asr_submit_url TEXT NOT NULL DEFAULT '',
        asr_query_url TEXT NOT NULL DEFAULT '',
        asr_resource_id TEXT NOT NULL DEFAULT '',
        asr_model_name TEXT NOT NULL DEFAULT '',
        asr_api_key_ref TEXT NOT NULL DEFAULT '',
        semantic_base_url TEXT NOT NULL DEFAULT '',
        semantic_model_name TEXT NOT NULL DEFAULT 'MiniMax-M3',
        semantic_api_key_ref TEXT NOT NULL DEFAULT '',
        todo_base_url TEXT NOT NULL DEFAULT '',
        todo_model_name TEXT NOT NULL DEFAULT '',
        todo_api_key_ref TEXT NOT NULL DEFAULT '',
        local_todo_model_version TEXT NOT NULL DEFAULT '',
        allow_cloud_fallback INTEGER NOT NULL DEFAULT 1 CHECK (allow_cloud_fallback IN (0, 1)),
        local_todo_runtime_status TEXT NOT NULL DEFAULT 'not_ready',
        local_todo_last_health_check_at TEXT NOT NULL DEFAULT '',
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
        extraction_provider_used TEXT NOT NULL DEFAULT '',
        extraction_fallback_used INTEGER NOT NULL DEFAULT 0 CHECK (extraction_fallback_used IN (0, 1)),
        extraction_fallback_reason TEXT NOT NULL DEFAULT '',
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

      CREATE TABLE IF NOT EXISTS semantic_artifacts (
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
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (session_id) REFERENCES conversation_sessions(id) ON DELETE CASCADE
      );

      CREATE TABLE IF NOT EXISTS model_invocations (
        id TEXT PRIMARY KEY,
        provider TEXT NOT NULL,
        model_name TEXT NOT NULL,
        capability TEXT NOT NULL,
        status TEXT NOT NULL
          CHECK (status IN ('pending', 'running', 'succeeded', 'failed')),
        request_summary TEXT NOT NULL DEFAULT '',
        response_summary TEXT NOT NULL DEFAULT '',
        input_tokens INTEGER NOT NULL DEFAULT 0,
        output_tokens INTEGER NOT NULL DEFAULT 0,
        duration_ms INTEGER NOT NULL DEFAULT 0,
        estimated_cost_microunits INTEGER NOT NULL DEFAULT 0,
        currency TEXT NOT NULL DEFAULT '',
        error_message TEXT NOT NULL DEFAULT '',
        trace_id TEXT,
        started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        finished_at DATETIME,
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
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
      CREATE INDEX IF NOT EXISTS idx_semantic_artifacts_session_type
        ON semantic_artifacts(session_id, artifact_type);
      CREATE INDEX IF NOT EXISTS idx_semantic_artifacts_status
        ON semantic_artifacts(status);
      CREATE INDEX IF NOT EXISTS idx_model_invocations_provider
        ON model_invocations(provider, capability);
      CREATE INDEX IF NOT EXISTS idx_model_invocations_status
        ON model_invocations(status);
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
    ensure_conversation_sessions_columns(&connection)?;
    ensure_model_invocations_columns(&connection)?;

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
        asr_provider_type,
        speaker_provider_type,
        todo_provider_type,
        semantic_provider_type,
        embedding_provider_type,
        export_provider_type,
        asr_base_url,
        asr_submit_url,
        asr_query_url,
        asr_resource_id,
        asr_model_name,
        asr_api_key_ref,
        semantic_base_url,
        semantic_model_name,
        semantic_api_key_ref,
        todo_base_url,
        todo_model_name,
        todo_api_key_ref,
        local_todo_model_version,
        allow_cloud_fallback,
        local_todo_runtime_status,
        local_todo_last_health_check_at
      ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)
      "#,
            params![
                "default",
                0,
                "zh-CN",
                30,
                20,
                "local",
                DEFAULT_ASR_PROVIDER_TYPE,
                DEFAULT_SPEAKER_PROVIDER_TYPE,
                DEFAULT_TODO_PROVIDER_TYPE,
                DEFAULT_SEMANTIC_PROVIDER_TYPE,
                DEFAULT_EMBEDDING_PROVIDER_TYPE,
                DEFAULT_EXPORT_PROVIDER_TYPE,
                "https://api.example.com/asr/query",
                "https://api.example.com/asr/submit",
                "https://api.example.com/asr/query",
                "volc.seedasr.auc",
                "bigmodel",
                "sk-asr-****",
                DEFAULT_SEMANTIC_BASE_URL,
                DEFAULT_SEMANTIC_MODEL_NAME,
                "sk-m3-****",
                "https://api.example.com/todo",
                "todo-model-v1",
                "sk-todo-****",
                EMBEDDED_TODO_MODEL_VERSION,
                1,
                "not_ready",
                ""
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
        extraction_provider_used,
        extraction_fallback_used,
        extraction_fallback_reason,
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
        'seed',
        0,
        '',
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
        asr_provider_type,
        speaker_provider_type,
        todo_provider_type,
        semantic_provider_type,
        embedding_provider_type,
        export_provider_type,
        asr_submit_url,
        asr_query_url,
        asr_resource_id,
        asr_model_name,
        asr_api_key_ref,
        semantic_base_url,
        semantic_model_name,
        semantic_api_key_ref,
        todo_base_url,
        todo_model_name,
        todo_api_key_ref,
        local_todo_model_version,
        allow_cloud_fallback,
        local_todo_runtime_status,
        local_todo_last_health_check_at
      FROM app_settings
      WHERE id = 'default'
      "#,
            [],
            |row| {
                let local_todo_model_version =
                    legacy_local_llm::normalize_model_version(row.get::<_, String>(22)?.as_str());
                Ok(SettingsDto {
                    record_enabled: row.get::<_, i64>(0)? == 1,
                    language: row.get(1)?,
                    chunk_seconds: row.get(2)?,
                    idle_trigger_seconds: row.get(3)?,
                    provider_mode: row.get(4)?,
                    asr_provider_type: normalize_asr_provider_type(&row.get::<_, String>(5)?),
                    speaker_provider_type: row.get(6)?,
                    todo_provider_type: normalize_todo_provider_type(&row.get::<_, String>(7)?),
                    semantic_provider_type: row.get(8)?,
                    embedding_provider_type: row.get(9)?,
                    export_provider_type: row.get(10)?,
                    asr_submit_url: row.get(11)?,
                    asr_query_url: row.get(12)?,
                    asr_resource_id: row.get(13)?,
                    asr_model_name: row.get(14)?,
                    asr_api_key_masked: row.get(15)?,
                    semantic_base_url: row.get(16)?,
                    semantic_model_name: row.get(17)?,
                    semantic_api_key_masked: row.get(18)?,
                    todo_base_url: row.get(19)?,
                    todo_model_name: row.get(20)?,
                    todo_api_key_masked: row.get(21)?,
                    local_todo_model_version,
                    allow_cloud_fallback: row.get::<_, i64>(23)? == 1,
                    local_todo_runtime_status: row.get(24)?,
                    local_todo_last_health_check_at: row.get(25)?,
                })
            },
        )
        .map_err(|error| format!("读取设置失败: {error}"))
}

fn persist_local_todo_runtime_status(
    connection: &Connection,
    runtime_status: &str,
    message: &str,
) -> Result<(), String> {
    connection
        .execute(
            r#"
            UPDATE app_settings
            SET
              local_todo_runtime_status = ?1,
              local_todo_last_health_check_at = CURRENT_TIMESTAMP,
              updated_at = CURRENT_TIMESTAMP
            WHERE id = 'default'
            "#,
            params![runtime_status],
        )
        .map_err(|error| format!("更新本地模型状态失败: {error}"))?;

    if runtime_status == "failed" {
        log::warn!("本地 Todo 运行时不可用: {message}");
    }
    Ok(())
}

fn ensure_legacy_todo_runtime_files_if_selected(
    connection: &Connection,
    models_dir: &PathBuf,
) -> Result<(), String> {
    let settings = query_settings(connection)?;
    if normalize_todo_provider_type(&settings.todo_provider_type) == LEGACY_TODO_PROVIDER_TYPE {
        legacy_local_llm::ensure_runtime_files(models_dir)?;
    }

    Ok(())
}

pub fn run_embedded_todo_runtime_once() -> Result<(), String> {
    legacy_local_llm::run_once()
}

fn query_local_todo_runtime_status(
    connection: &Connection,
    models_dir: &PathBuf,
) -> Result<LocalTodoRuntimeStatusDto, String> {
    let settings = query_settings(connection)?;
    if normalize_todo_provider_type(&settings.todo_provider_type) != LEGACY_TODO_PROVIDER_TYPE {
        return Ok(LocalTodoRuntimeStatusDto {
            provider_type: settings.todo_provider_type,
            model_version: legacy_local_llm::normalize_model_version(
                &settings.local_todo_model_version,
            ),
            runtime_status: "not_ready".into(),
            last_health_check_at: settings.local_todo_last_health_check_at,
            fallback_enabled: settings.allow_cloud_fallback,
            message: "旧本地 Todo 运行时已降级为 legacy，默认语义链路不启动该运行时".into(),
        });
    }

    let version = legacy_local_llm::normalize_model_version(&settings.local_todo_model_version);
    let manifest_path = legacy_local_llm::manifest_path(models_dir, &version);

    let (runtime_status, message) = if manifest_path.exists() {
        match legacy_local_llm::verify_runtime_files(models_dir, &version) {
            Ok(()) => {
                match legacy_local_llm::spawn_runtime_request(&legacy_local_llm::RuntimeRequest {
                    action: "health_check".into(),
                    model_version: version.clone(),
                    runtime_dir: models_dir.to_string_lossy().to_string(),
                    text: String::new(),
                }) {
                    Ok(response) if response.success => ("ready".to_string(), response.message),
                    Ok(response) => (response.runtime_status, response.message),
                    Err(error) => ("failed".to_string(), error),
                }
            }
            Err(error) => ("failed".to_string(), error),
        }
    } else {
        (
            "not_ready".to_string(),
            "未检测到本地 Todo 运行时资源，请重新初始化应用数据目录".to_string(),
        )
    };

    persist_local_todo_runtime_status(connection, &runtime_status, &message)?;
    let refreshed = query_settings(connection)?;

    Ok(LocalTodoRuntimeStatusDto {
        provider_type: refreshed.todo_provider_type,
        model_version: if refreshed.local_todo_model_version.trim().is_empty() {
            version
        } else {
            refreshed.local_todo_model_version
        },
        runtime_status,
        last_health_check_at: refreshed.local_todo_last_health_check_at,
        fallback_enabled: refreshed.allow_cloud_fallback,
        message,
    })
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

fn test_todo_cloud_provider(settings: &SettingsDto) -> Result<ModelTestResult, String> {
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

fn test_todo_embedded_provider(
    settings: &SettingsDto,
    state: &AppState,
) -> Result<ModelTestResult, String> {
    let connection = open_connection(&state.db_path)?;
    let runtime = query_local_todo_runtime_status(&connection, &state.models_dir)?;
    let success = runtime.runtime_status == "ready";

    Ok(ModelTestResult {
        provider: "todo".into(),
        success,
        status_code: if success { 200 } else { 503 },
        message: if success {
            format!(
                "本地 Todo 运行时测试成功，当前版本 {}",
                legacy_local_llm::normalize_model_version(&settings.local_todo_model_version)
            )
        } else {
            runtime.message
        },
        response_excerpt: "".into(),
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

    let starts = input
        .match_indices('[')
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();
    let ends = input
        .match_indices(']')
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();

    for start in starts.iter().rev() {
        for end in ends.iter().rev() {
            if end < start {
                continue;
            }

            let candidate = &input[*start..=*end];
            if let Ok(value) = serde_json::from_str::<Vec<serde_json::Value>>(candidate) {
                return Ok(value);
            }
        }
    }

    Err("无法从模型响应中解析 Todo JSON 数组".to_string())
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

fn request_cloud_todo_extraction(
    settings: &SettingsDto,
    merged_text: &str,
) -> Result<Vec<serde_json::Value>, String> {
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
    extract_json_array(&output_text)
}

fn request_embedded_todo_extraction(
    settings: &SettingsDto,
    models_dir: &PathBuf,
    merged_text: &str,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(legacy_local_llm::request_todo_extraction(
        models_dir,
        &settings.local_todo_model_version,
        merged_text,
    )?
    .into_iter()
    .map(|item| {
        serde_json::json!({
            "title": item.title,
            "note": item.note
        })
    })
    .collect::<Vec<_>>())
}

fn register_semantic_todo_artifact_boundary(
    connection: &Connection,
    settings: &SettingsDto,
    session_id: &str,
    merged_text: &str,
) -> Result<(), String> {
    let payload = serde_json::json!({
        "boundary": "v0.4_semantic_provider",
        "status": "pending_provider_integration",
        "todo_candidates": [],
        "source_preview": clip_text(merged_text, 240),
    })
    .to_string();
    let artifact_id = format!("semantic_todo_{}", current_timestamp_label());

    connection
        .execute(
            "DELETE FROM semantic_artifacts WHERE session_id = ?1 AND artifact_type = 'todo_extraction'",
            params![session_id],
        )
        .map_err(|error| format!("清理旧 Todo 语义产物失败: {error}"))?;

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
            ) VALUES (?1, ?2, 'todo_extraction', 'pending', ?3, ?4, 'v0.4', '[]', ?5, '', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
            params![
                artifact_id,
                session_id,
                settings.semantic_provider_type.as_str(),
                settings.semantic_model_name.as_str(),
                payload,
            ],
        )
        .map_err(|error| format!("登记 Todo 语义产物边界失败: {error}"))?;

    connection
        .execute(
            r#"
            UPDATE conversation_sessions
            SET
              extraction_status = 'success',
              extraction_provider_used = ?1,
              extraction_fallback_used = 0,
              extraction_fallback_reason = 'v0.4 仅登记 MiniMax M3 语义产物边界，实际 Todo 候选生成在后续版本接入'
            WHERE id = ?2
            "#,
            params![settings.semantic_provider_type.as_str(), session_id],
        )
        .map_err(|error| format!("更新 Todo 语义边界状态失败: {error}"))?;

    Ok(())
}

fn generate_todos_for_session(
    connection: &Connection,
    settings: &SettingsDto,
    models_dir: &PathBuf,
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
                "UPDATE conversation_sessions SET extraction_status = 'success', extraction_provider_used = 'skipped', extraction_fallback_used = 0 WHERE id = ?1",
                params![session_id],
            )
            .map_err(|error| format!("更新空会话状态失败: {error}"))?;
        return Ok(0);
    }

    if normalize_todo_provider_type(&settings.todo_provider_type) == DEFAULT_TODO_PROVIDER_TYPE {
        register_semantic_todo_artifact_boundary(connection, settings, session_id, &merged_text)?;
        return Ok(0);
    }

    let (
        todos,
        extraction_model_name,
        extraction_provider_used,
        extraction_fallback_used,
        extraction_fallback_reason,
    ) = if normalize_todo_provider_type(&settings.todo_provider_type) == LEGACY_TODO_PROVIDER_TYPE {
        match request_embedded_todo_extraction(settings, models_dir, &merged_text) {
            Ok(local_items) if !local_items.is_empty() => (
                local_items,
                legacy_local_llm::normalize_model_version(&settings.local_todo_model_version),
                LEGACY_TODO_PROVIDER_TYPE.to_string(),
                false,
                "".to_string(),
            ),
            Ok(local_items) => {
                if !settings.allow_cloud_fallback
                    || settings.todo_base_url.trim().is_empty()
                    || settings.todo_model_name.trim().is_empty()
                    || settings.todo_api_key_masked.trim().is_empty()
                {
                    (
                        local_items,
                        legacy_local_llm::normalize_model_version(
                            &settings.local_todo_model_version,
                        ),
                        LEGACY_TODO_PROVIDER_TYPE.to_string(),
                        false,
                        "".to_string(),
                    )
                } else {
                    (
                        request_cloud_todo_extraction(settings, &merged_text)?,
                        settings.todo_model_name.clone(),
                        "cloud".to_string(),
                        true,
                        "本地提取结果为空，已使用云端兜底".to_string(),
                    )
                }
            }
            Err(error) => {
                if !settings.allow_cloud_fallback
                    || settings.todo_base_url.trim().is_empty()
                    || settings.todo_model_name.trim().is_empty()
                    || settings.todo_api_key_masked.trim().is_empty()
                {
                    return Err(error);
                }

                log::warn!("本地 Todo 子进程提取失败，使用云端 Provider 兜底: {error}");
                (
                    request_cloud_todo_extraction(settings, &merged_text)?,
                    settings.todo_model_name.clone(),
                    "cloud".to_string(),
                    true,
                    format!("本地提取失败后使用云端兜底：{}", clip_text(&error, 120)),
                )
            }
        }
    } else {
        (
            request_cloud_todo_extraction(settings, &merged_text)?,
            settings.todo_model_name.clone(),
            "cloud".to_string(),
            false,
            "".to_string(),
        )
    };

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
                    extraction_model_name.as_str(),
                    format!("trace_todo_{}_{}", session_id, index)
                ],
            )
            .map_err(|error| format!("写入 Todo 失败: {error}"))?;
        inserted += 1;
    }

    connection
        .execute(
            "UPDATE conversation_sessions SET extraction_status = 'success', extraction_provider_used = ?1, extraction_fallback_used = ?2, extraction_fallback_reason = ?3 WHERE id = ?4",
            params![
                extraction_provider_used,
                if extraction_fallback_used { 1 } else { 0 },
                extraction_fallback_reason,
                session_id
            ],
        )
        .map_err(|error| format!("更新会话提取状态失败: {error}"))?;

    Ok(inserted)
}

fn process_pending_jobs_internal(
    connection: &Connection,
    models_dir: &PathBuf,
) -> Result<String, String> {
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

        match generate_todos_for_session(connection, &settings, models_dir, &session_id) {
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
    let models_dir = db_path
        .parent()
        .map(|parent| parent.join("models"))
        .unwrap_or_else(|| PathBuf::from("models"));
    initialize_database(&db_path)?;
    let connection = open_connection(&db_path)?;
    ensure_legacy_todo_runtime_files_if_selected(&connection, &models_dir)?;
    process_pending_jobs_internal(&connection, &models_dir)
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
fn test_model_connection(
    payload: ModelTestRequest,
    state: tauri::State<'_, AppState>,
) -> Result<ModelTestResult, String> {
    match payload.provider.as_str() {
        "todo" => {
            let todo_provider_type =
                normalize_todo_provider_type(&payload.settings.todo_provider_type);
            if todo_provider_type == DEFAULT_TODO_PROVIDER_TYPE {
                Ok(ModelTestResult {
                    provider: "todo".into(),
                    success: true,
                    status_code: 0,
                    message: "MiniMax M3 语义 Todo 边界已登记；v0.4 不发起实际 Todo 生成调用"
                        .into(),
                    response_excerpt: "semantic_artifacts(type='todo_extraction')".into(),
                })
            } else if todo_provider_type == LEGACY_TODO_PROVIDER_TYPE {
                test_todo_embedded_provider(&payload.settings, &state)
            } else {
                test_todo_cloud_provider(&payload.settings)
            }
        }
        "asr" => test_asr_provider(&payload.settings),
        other => Err(format!("不支持的模型测试类型: {other}")),
    }
}

#[tauri::command]
fn get_local_todo_runtime_status(
    state: tauri::State<'_, AppState>,
) -> Result<LocalTodoRuntimeStatusDto, String> {
    let connection = open_connection(&state.db_path)?;
    query_local_todo_runtime_status(&connection, &state.models_dir)
}

#[tauri::command]
fn get_desktop_context(state: tauri::State<'_, AppState>) -> Result<DesktopContext, String> {
    let recording = is_recording(&state)?;
    let connection = open_connection(&state.db_path)?;
    let runtime = query_local_todo_runtime_status(&connection, &state.models_dir)?;
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
        models_status: runtime.message,
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

fn persist_settings(connection: &Connection, payload: &SettingsDto) -> Result<(), String> {
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
        asr_provider_type = ?6,
        speaker_provider_type = ?7,
        todo_provider_type = ?8,
        semantic_provider_type = ?9,
        embedding_provider_type = ?10,
        export_provider_type = ?11,
        asr_base_url = ?12,
        asr_submit_url = ?13,
        asr_query_url = ?14,
        asr_resource_id = ?15,
        asr_model_name = ?16,
        asr_api_key_ref = ?17,
        semantic_base_url = ?18,
        semantic_model_name = ?19,
        semantic_api_key_ref = ?20,
        todo_base_url = ?21,
        todo_model_name = ?22,
        todo_api_key_ref = ?23,
        local_todo_model_version = ?24,
        allow_cloud_fallback = ?25,
        local_todo_runtime_status = ?26,
        local_todo_last_health_check_at = ?27,
        updated_at = CURRENT_TIMESTAMP
      WHERE id = 'default'
      "#,
            params![
                if payload.record_enabled { 1 } else { 0 },
                payload.language.as_str(),
                payload.chunk_seconds,
                payload.idle_trigger_seconds,
                payload.provider_mode.as_str(),
                normalize_asr_provider_type(&payload.asr_provider_type),
                payload.speaker_provider_type.as_str(),
                normalize_todo_provider_type(&payload.todo_provider_type),
                payload.semantic_provider_type.as_str(),
                payload.embedding_provider_type.as_str(),
                payload.export_provider_type.as_str(),
                payload.asr_submit_url.as_str(),
                payload.asr_submit_url.as_str(),
                payload.asr_query_url.as_str(),
                payload.asr_resource_id.as_str(),
                payload.asr_model_name.as_str(),
                payload.asr_api_key_masked.as_str(),
                payload.semantic_base_url.as_str(),
                payload.semantic_model_name.as_str(),
                payload.semantic_api_key_masked.as_str(),
                payload.todo_base_url.as_str(),
                payload.todo_model_name.as_str(),
                payload.todo_api_key_masked.as_str(),
                payload.local_todo_model_version.as_str(),
                if payload.allow_cloud_fallback { 1 } else { 0 },
                payload.local_todo_runtime_status.as_str(),
                payload.local_todo_last_health_check_at.as_str(),
            ],
        )
        .map_err(|error| format!("保存设置失败: {error}"))?;
    Ok(())
}

#[tauri::command]
fn save_settings(
    payload: SettingsDto,
    state: tauri::State<'_, AppState>,
) -> Result<SettingsDto, String> {
    let connection = open_connection(&state.db_path)?;
    persist_settings(&connection, &payload)?;
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
    let _ = process_pending_jobs_internal(&connection, &state.models_dir)?;

    latest_session(&connection)?.ok_or_else(|| "未找到刚创建的会话".to_string())
}

#[tauri::command]
fn process_pending_jobs(
    state: tauri::State<'_, AppState>,
) -> Result<ProcessingActionResult, String> {
    let connection = open_connection(&state.db_path)?;
    let message = process_pending_jobs_internal(&connection, &state.models_dir)?;
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
    let processing_summary = process_pending_jobs_internal(&connection, &state.models_dir)?;

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
    let processing_summary = process_pending_jobs_internal(&connection, &state.models_dir)?;

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
            let models_dir = app_data_dir.join("models");
            let db_path = app_data_dir.join("smart-todo.sqlite");

            initialize_database(&db_path)?;
            fs::create_dir_all(&recordings_dir)
                .map_err(|error| format!("创建录音目录失败: {error}"))?;
            let connection = open_connection(&db_path)?;
            ensure_legacy_todo_runtime_files_if_selected(&connection, &models_dir)?;
            app.manage(AppState {
                db_path,
                recordings_dir,
                models_dir,
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
            get_local_todo_runtime_status,
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
    fn should_build_embedded_prompt_with_input_text() {
        let template = "前文\n{{input_text}}\n后文";
        let prompt = legacy_local_llm::build_prompt(template, "  今天联系客户并发送报价  ");
        assert!(prompt.contains("今天联系客户并发送报价"));
        assert!(!prompt.contains("{{input_text}}"));
    }

    #[test]
    fn should_parse_runtime_todos_from_wrapped_json_output() {
        let output = "思考略\n[{\"title\":\"联系客户发送报价并确认税率\",\"note\":\"今天下午联系客户并发送报价，确认税率口径。\"}]";
        let todos = legacy_local_llm::parse_runtime_todos(output).expect("应能解析 JSON 数组");
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].title, "联系客户发送报价并确认税率");
        assert!(todos[0].note.contains("确认税率"));
    }

    #[test]
    fn should_extract_last_valid_json_array_from_prompt_echo_output() {
        let output = "示例输出：\n[{\"title\":\"示例任务\",\"note\":\"示例说明\"}]\n待处理文稿：\n今天下班前把合同发给小李。\n\n[{\"title\":\"给小李发送合同\",\"note\":\"今天下班前把合同发给小李。\"}] [end of text]";
        let todos = legacy_local_llm::parse_runtime_todos(output)
            .expect("应能从回显提示词中提取最后一个 JSON 数组");
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].title, "给小李发送合同");
        assert_eq!(todos[0].note, "今天下班前把合同发给小李。");
    }

    #[test]
    fn should_parse_embedded_manifest_text() {
        let manifest = legacy_local_llm::parse_manifest_text(
            legacy_local_llm::embedded_manifest_text().expect("内嵌清单应为 UTF-8"),
        )
        .expect("应能解析内嵌清单");
        assert_eq!(manifest.model_version, EMBEDDED_TODO_MODEL_VERSION);
        assert_eq!(manifest.engine, "llama.cpp");
        assert_eq!(manifest.prompt_template_rel_path, "prompt_template.txt");
    }

    #[test]
    fn should_report_not_ready_when_llama_cli_or_gguf_missing() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-runtime-test-{}",
            current_timestamp_label()
        ));
        legacy_local_llm::ensure_runtime_files(&temp_dir).expect("应能释放内嵌模型清单");

        let error = legacy_local_llm::load_runtime_config(&temp_dir, EMBEDDED_TODO_MODEL_VERSION)
            .expect_err("缺少 llama-cli 和 GGUF 时不应进入 ready");
        assert!(
            error.contains("llama.cpp") || error.contains("GGUF"),
            "错误信息应明确指出缺失的运行时资源，实际为: {error}"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

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
        let settings = query_settings(&connection).expect("应能读取默认设置");

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
        let settings = query_settings(&connection).expect("应能读取迁移后的设置");

        assert_eq!(settings.asr_provider_type, "local_whisperkit");
        assert_eq!(settings.speaker_provider_type, "local_speakerkit");
        assert_eq!(settings.todo_provider_type, LEGACY_TODO_PROVIDER_TYPE);
        assert_eq!(settings.semantic_provider_type, "minimax_m3");
        assert_eq!(settings.semantic_base_url, DEFAULT_SEMANTIC_BASE_URL);
        assert_eq!(settings.semantic_model_name, DEFAULT_SEMANTIC_MODEL_NAME);

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
        let settings = query_settings(&connection).expect("应能读取默认设置");
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

        let inserted = generate_todos_for_session(&connection, &settings, &temp_dir, session_id)
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
    fn should_prepare_legacy_runtime_files_only_when_selected() {
        let temp_dir = std::env::temp_dir().join(format!(
            "smart-todo-v04-legacy-runtime-gate-test-{}",
            current_timestamp_label()
        ));
        fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
        let db_path = temp_dir.join("smart-todo.sqlite");
        let models_dir = temp_dir.join("models");
        let manifest_path =
            legacy_local_llm::manifest_path(&models_dir, legacy_local_llm::MODEL_VERSION);

        initialize_database(&db_path).expect("应能初始化数据库");
        let connection = open_connection(&db_path).expect("应能打开测试数据库");

        ensure_legacy_todo_runtime_files_if_selected(&connection, &models_dir)
            .expect("默认语义路径不应要求 legacy 资源");
        assert!(
            !manifest_path.exists(),
            "默认 semantic_m3 路径不应释放 legacy manifest"
        );

        connection
            .execute(
                "UPDATE app_settings SET todo_provider_type = ?1 WHERE id = 'default'",
                params![LEGACY_TODO_PROVIDER_TYPE],
            )
            .expect("应能切换到 legacy Todo provider");
        ensure_legacy_todo_runtime_files_if_selected(&connection, &models_dir)
            .expect("显式 legacy provider 应释放本地运行时资源");
        assert!(manifest_path.exists(), "legacy provider 应释放 manifest");

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
        let mut settings = query_settings(&connection).expect("应能读取默认设置");

        settings.asr_provider_type = "cloud_volc".into();
        settings.todo_provider_type = "legacy_local_llm".into();
        settings.semantic_base_url = "https://m3.example.test/v1/responses".into();
        settings.semantic_model_name = "MiniMax-M3-Test".into();
        settings.semantic_api_key_masked = "sk-test-****".into();
        settings.export_provider_type = "local_file".into();

        persist_settings(&connection, &settings).expect("应能保存 v0.4 设置");
        let persisted = query_settings(&connection).expect("应能读取保存后的设置");

        assert_eq!(persisted.asr_provider_type, "cloud_volc");
        assert_eq!(persisted.todo_provider_type, LEGACY_TODO_PROVIDER_TYPE);
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
    fn should_expose_legacy_local_llm_provider_boundary() {
        let normalized = legacy_local_llm::normalize_model_version("todo-embedded-v1");

        assert_eq!(normalized, legacy_local_llm::MODEL_VERSION);
        assert!(legacy_local_llm::manifest_path(
            &PathBuf::from("/tmp/models"),
            legacy_local_llm::MODEL_VERSION
        )
        .ends_with("todo/qwen3-4b-instruct-2507-q4_k_m/manifest.json"));
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
