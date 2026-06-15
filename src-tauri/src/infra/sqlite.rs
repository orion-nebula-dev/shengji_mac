use rusqlite::{params, Connection};
use std::{fs, path::PathBuf};

use crate::{
    DEFAULT_ASR_PROVIDER_TYPE, DEFAULT_EMBEDDING_PROVIDER_TYPE, DEFAULT_EXPORT_PROVIDER_TYPE,
    DEFAULT_SEMANTIC_BASE_URL, DEFAULT_SEMANTIC_MODEL_NAME, DEFAULT_SEMANTIC_PROVIDER_TYPE,
    DEFAULT_SPEAKER_PROVIDER_TYPE, DEFAULT_TODO_PROVIDER_TYPE,
};

pub(crate) fn open_connection(db_path: &PathBuf) -> Result<Connection, String> {
    Connection::open(db_path).map_err(|error| format!("打开数据库失败: {error}"))
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

    let had_asr_submit_url = columns.iter().any(|column| column == "asr_submit_url");
    let had_asr_query_url = columns.iter().any(|column| column == "asr_query_url");

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
            "semantic_base_url",
            "ALTER TABLE app_settings ADD COLUMN semantic_base_url TEXT NOT NULL DEFAULT 'https://api.minimaxi.com/v1/chat/completions'",
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
    ] {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute(sql, [])
                .map_err(|error| format!("补充设置字段 {name} 失败: {error}"))?;
        }
    }

    if !had_asr_query_url || !had_asr_submit_url {
        connection
            .execute(
                r#"
                UPDATE app_settings
                SET
                  asr_query_url = CASE
                    WHEN ?1 = 1 AND asr_query_url = '' THEN asr_base_url
                    ELSE asr_query_url
                  END,
                  asr_submit_url = CASE
                    WHEN ?2 = 1 AND asr_submit_url = '' AND asr_base_url LIKE '%/query' THEN REPLACE(asr_base_url, '/query', '/submit')
                    WHEN ?2 = 1 AND asr_submit_url = '' THEN asr_base_url
                    ELSE asr_submit_url
                  END
                WHERE id = 'default'
                "#,
                params![
                    if had_asr_query_url { 0 } else { 1 },
                    if had_asr_submit_url { 0 } else { 1 },
                ],
            )
            .map_err(|error| format!("迁移旧版 ASR URL 设置失败: {error}"))?;
    }

    connection
        .execute(
            r#"
            UPDATE app_settings
            SET
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
                ELSE 'semantic_m3'
              END,
              semantic_provider_type = CASE
                WHEN TRIM(semantic_provider_type) = '' THEN 'minimax_m3'
                ELSE 'minimax_m3'
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
                WHEN TRIM(semantic_base_url) = ''
                  OR semantic_base_url = 'https://api.minimax.io/v1/responses'
                  OR semantic_base_url = 'https://api.minimax.io/v1/text/chatcompletion_v2'
                THEN 'https://api.minimaxi.com/v1/chat/completions'
                ELSE semantic_base_url
              END,
              semantic_model_name = CASE
                WHEN TRIM(semantic_model_name) = '' THEN 'MiniMax-M3'
                ELSE semantic_model_name
              END,
              allow_cloud_fallback = CASE
                WHEN allow_cloud_fallback IS NULL THEN 1
                ELSE allow_cloud_fallback
              END
            WHERE id = 'default'
            "#,
            [],
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

fn ensure_transcript_segments_v05_columns(connection: &Connection) -> Result<(), String> {
    let mut columns = Vec::new();
    let mut statement = connection
        .prepare("PRAGMA table_info(transcript_segments)")
        .map_err(|error| format!("读取转写片段表结构失败: {error}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("查询转写片段表字段失败: {error}"))?;

    for column in rows {
        columns.push(column.map_err(|error| format!("读取转写片段字段失败: {error}"))?);
    }

    for (name, sql) in [
        (
            "speaker_id",
            "ALTER TABLE transcript_segments ADD COLUMN speaker_id TEXT NOT NULL DEFAULT ''",
        ),
        (
            "start_ms",
            "ALTER TABLE transcript_segments ADD COLUMN start_ms INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "end_ms",
            "ALTER TABLE transcript_segments ADD COLUMN end_ms INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "confidence",
            "ALTER TABLE transcript_segments ADD COLUMN confidence REAL NOT NULL DEFAULT 0",
        ),
        (
            "provider",
            "ALTER TABLE transcript_segments ADD COLUMN provider TEXT NOT NULL DEFAULT 'local_whisperkit'",
        ),
        (
            "review_status",
            "ALTER TABLE transcript_segments ADD COLUMN review_status TEXT NOT NULL DEFAULT 'normal'",
        ),
        (
            "review_reason",
            "ALTER TABLE transcript_segments ADD COLUMN review_reason TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute(sql, [])
                .map_err(|error| format!("补充转写片段字段 {name} 失败: {error}"))?;
        }
    }

    Ok(())
}

fn ensure_semantic_artifact_type_constraint(connection: &Connection) -> Result<(), String> {
    let table_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'semantic_artifacts'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| format!("读取 semantic_artifacts 表结构失败: {error}"))?;

    if table_sql.contains("'transcript_revision'")
        && table_sql.contains("'recording_type'")
        && table_sql.contains("'meeting_minutes'")
    {
        return Ok(());
    }

    connection
        .execute_batch(
            r#"
      PRAGMA foreign_keys = OFF;
      DROP INDEX IF EXISTS idx_semantic_artifacts_session_type;
      DROP INDEX IF EXISTS idx_semantic_artifacts_status;
      DROP TABLE IF EXISTS semantic_artifacts_legacy_type_constraint;
      ALTER TABLE semantic_artifacts RENAME TO semantic_artifacts_legacy_type_constraint;

      CREATE TABLE semantic_artifacts (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        artifact_type TEXT NOT NULL
          CHECK (artifact_type IN ('transcript_revision', 'recording_type', 'summary', 'meeting_minutes', 'todo_extraction', 'mind_map', 'moment', 'deep_research', 'translation')),
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
      )
      SELECT
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
      FROM semantic_artifacts_legacy_type_constraint
      WHERE artifact_type IN ('summary', 'todo_extraction', 'mind_map', 'moment', 'deep_research', 'translation');

      DROP TABLE semantic_artifacts_legacy_type_constraint;
      CREATE INDEX IF NOT EXISTS idx_semantic_artifacts_session_type
        ON semantic_artifacts(session_id, artifact_type);
      CREATE INDEX IF NOT EXISTS idx_semantic_artifacts_status
        ON semantic_artifacts(status);
      PRAGMA foreign_keys = ON;
      "#,
        )
        .map_err(|error| format!("迁移 semantic_artifacts 类型约束失败: {error}"))?;

    Ok(())
}

fn ensure_todos_v07_schema(connection: &Connection) -> Result<(), String> {
    let table_sql: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'todos'",
            [],
            |row| row.get(0),
        )
        .ok();
    let Some(sql) = table_sql else {
        return Ok(());
    };

    if sql.contains("'in_progress'") && sql.contains("candidate_id") {
        connection
            .execute_batch(
                r#"
                CREATE INDEX IF NOT EXISTS idx_todos_candidate
                  ON todos(candidate_id);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_todos_dedup_key
                  ON todos(dedup_key)
                  WHERE dedup_key <> '';
                "#,
            )
            .map_err(|error| format!("创建 Todo v0.7 索引失败: {error}"))?;
        return Ok(());
    }

    connection
        .execute_batch(
            r#"
            DROP INDEX IF EXISTS idx_todos_status;
            DROP INDEX IF EXISTS idx_todos_created_at;
            DROP INDEX IF EXISTS idx_todos_candidate;
            DROP INDEX IF EXISTS idx_todos_dedup_key;
            DROP TABLE IF EXISTS todos_v07_migration_source;
            ALTER TABLE todos RENAME TO todos_v07_migration_source;

            CREATE TABLE todos (
              id TEXT PRIMARY KEY,
              conversation_session_id TEXT NOT NULL,
              title TEXT NOT NULL,
              note TEXT NOT NULL DEFAULT '',
              status TEXT NOT NULL DEFAULT 'open'
                CHECK (status IN ('open', 'in_progress', 'done', 'dismissed')),
              created_at TEXT NOT NULL,
              completed_at DATETIME,
              source_text TEXT,
              source_audio_id TEXT,
              speaker_id TEXT,
              extraction_model_name TEXT NOT NULL DEFAULT '',
              trace_id TEXT,
              updated_at DATETIME NOT NULL,
              owner TEXT NOT NULL DEFAULT '',
              due_at TEXT NOT NULL DEFAULT '',
              priority TEXT NOT NULL DEFAULT 'medium'
                CHECK (priority IN ('low', 'medium', 'high')),
              source_span_refs TEXT NOT NULL DEFAULT '[]',
              candidate_id TEXT NOT NULL DEFAULT '',
              dedup_key TEXT NOT NULL DEFAULT '',
              FOREIGN KEY (conversation_session_id) REFERENCES conversation_sessions(id) ON DELETE CASCADE
            );

            INSERT INTO todos (
              id,
              conversation_session_id,
              title,
              note,
              status,
              created_at,
              completed_at,
              source_text,
              source_audio_id,
              speaker_id,
              extraction_model_name,
              trace_id,
              updated_at,
              owner,
              due_at,
              priority,
              source_span_refs,
              candidate_id,
              dedup_key
            )
            SELECT
              id,
              conversation_session_id,
              title,
              note,
              CASE status
                WHEN 'completed' THEN 'done'
                WHEN 'done' THEN 'done'
                WHEN 'in_progress' THEN 'in_progress'
                WHEN 'dismissed' THEN 'dismissed'
                ELSE 'open'
              END,
              created_at,
              completed_at,
              source_text,
              source_audio_id,
              speaker_id,
              extraction_model_name,
              trace_id,
              updated_at,
              '',
              '',
              'medium',
              '[]',
              '',
              conversation_session_id || '::' || lower(trim(title))
            FROM todos_v07_migration_source;

            DROP TABLE todos_v07_migration_source;
            CREATE INDEX IF NOT EXISTS idx_todos_status
              ON todos(status);
            CREATE INDEX IF NOT EXISTS idx_todos_created_at
              ON todos(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_todos_candidate
              ON todos(candidate_id);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_todos_dedup_key
              ON todos(dedup_key)
              WHERE dedup_key <> '';
            "#,
        )
        .map_err(|error| format!("迁移 Todo v0.7 表结构失败: {error}"))?;

    Ok(())
}

pub(crate) fn initialize_database(db_path: &PathBuf) -> Result<(), String> {
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
        allow_cloud_fallback INTEGER NOT NULL DEFAULT 1 CHECK (allow_cloud_fallback IN (0, 1)),
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
        speaker_id TEXT NOT NULL DEFAULT '',
        start_ms INTEGER NOT NULL DEFAULT 0 CHECK (start_ms >= 0),
        end_ms INTEGER NOT NULL DEFAULT 0 CHECK (end_ms >= 0),
        text TEXT NOT NULL,
        confidence REAL NOT NULL DEFAULT 0,
        language TEXT NOT NULL DEFAULT 'zh-CN',
        status TEXT NOT NULL DEFAULT 'success'
          CHECK (status IN ('pending', 'success', 'failed')),
        provider TEXT NOT NULL DEFAULT 'local_whisperkit',
        provider_model_name TEXT NOT NULL DEFAULT '',
        review_status TEXT NOT NULL DEFAULT 'normal'
          CHECK (review_status IN ('normal', 'flagged', 'corrected')),
        review_reason TEXT NOT NULL DEFAULT '',
        trace_id TEXT,
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (audio_segment_id) REFERENCES audio_segments(id) ON DELETE CASCADE,
        FOREIGN KEY (conversation_session_id) REFERENCES conversation_sessions(id) ON DELETE SET NULL
      );

      CREATE TABLE IF NOT EXISTS speakers (
        id TEXT PRIMARY KEY,
        label TEXT NOT NULL,
        display_name TEXT NOT NULL,
        color TEXT NOT NULL DEFAULT '#2f7df6',
        corrected INTEGER NOT NULL DEFAULT 0 CHECK (corrected IN (0, 1)),
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
      );

      CREATE TABLE IF NOT EXISTS speaker_segments (
        id TEXT PRIMARY KEY,
        speaker_id TEXT NOT NULL,
        audio_segment_id TEXT NOT NULL,
        start_ms INTEGER NOT NULL DEFAULT 0 CHECK (start_ms >= 0),
        end_ms INTEGER NOT NULL DEFAULT 0 CHECK (end_ms >= 0),
        confidence REAL NOT NULL DEFAULT 0,
        corrected INTEGER NOT NULL DEFAULT 0 CHECK (corrected IN (0, 1)),
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (speaker_id) REFERENCES speakers(id) ON DELETE CASCADE,
        FOREIGN KEY (audio_segment_id) REFERENCES audio_segments(id) ON DELETE CASCADE
      );

      CREATE TABLE IF NOT EXISTS transcript_jobs (
        id TEXT PRIMARY KEY,
        audio_segment_id TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'queued'
          CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'retrying')),
        retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
        max_retry_count INTEGER NOT NULL DEFAULT 3 CHECK (max_retry_count >= 0),
        error_message TEXT NOT NULL DEFAULT '',
        provider TEXT NOT NULL DEFAULT 'local_whisperkit',
        model_name TEXT NOT NULL DEFAULT 'large-v3-turbo',
        started_at DATETIME,
        finished_at DATETIME,
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
      );

      CREATE TABLE IF NOT EXISTS local_model_status (
        provider TEXT PRIMARY KEY,
        model_name TEXT NOT NULL,
        cache_dir TEXT NOT NULL,
        download_status TEXT NOT NULL DEFAULT 'available'
          CHECK (download_status IN ('not_started', 'downloading', 'available', 'failed')),
        download_progress INTEGER NOT NULL DEFAULT 100 CHECK (download_progress >= 0 AND download_progress <= 100),
        offline_available INTEGER NOT NULL DEFAULT 1 CHECK (offline_available IN (0, 1)),
        device_recommendation TEXT NOT NULL DEFAULT '',
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
      );

      CREATE TABLE IF NOT EXISTS transcript_correction_patterns (
        id TEXT PRIMARY KEY,
        phrase TEXT NOT NULL,
        replacement TEXT NOT NULL,
        pattern_type TEXT NOT NULL,
        scope TEXT NOT NULL DEFAULT 'local',
        confidence REAL NOT NULL DEFAULT 0,
        enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
      );

      CREATE TABLE IF NOT EXISTS semantic_artifacts (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        artifact_type TEXT NOT NULL
          CHECK (artifact_type IN ('transcript_revision', 'recording_type', 'summary', 'meeting_minutes', 'todo_extraction', 'mind_map', 'moment', 'deep_research', 'translation')),
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
        status TEXT NOT NULL DEFAULT 'open'
          CHECK (status IN ('open', 'in_progress', 'done', 'dismissed')),
        created_at TEXT NOT NULL,
        completed_at DATETIME,
        source_text TEXT,
        source_audio_id TEXT,
        speaker_id TEXT,
        extraction_model_name TEXT NOT NULL DEFAULT '',
        trace_id TEXT,
        updated_at DATETIME NOT NULL,
        owner TEXT NOT NULL DEFAULT '',
        due_at TEXT NOT NULL DEFAULT '',
        priority TEXT NOT NULL DEFAULT 'medium'
          CHECK (priority IN ('low', 'medium', 'high')),
        source_span_refs TEXT NOT NULL DEFAULT '[]',
        candidate_id TEXT NOT NULL DEFAULT '',
        dedup_key TEXT NOT NULL DEFAULT '',
        FOREIGN KEY (conversation_session_id) REFERENCES conversation_sessions(id) ON DELETE CASCADE
      );

      CREATE TABLE IF NOT EXISTS todo_candidates (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        artifact_id TEXT NOT NULL,
        title TEXT NOT NULL,
        detail TEXT NOT NULL DEFAULT '',
        owner TEXT NOT NULL DEFAULT '',
        due_at TEXT NOT NULL DEFAULT '',
        priority TEXT NOT NULL DEFAULT 'medium'
          CHECK (priority IN ('low', 'medium', 'high')),
        confidence REAL NOT NULL DEFAULT 0,
        source_span_refs TEXT NOT NULL DEFAULT '[]',
        source_text TEXT NOT NULL DEFAULT '',
        status TEXT NOT NULL DEFAULT 'proposed'
          CHECK (status IN ('proposed', 'accepted', 'dismissed', 'merged')),
        todo_id TEXT NOT NULL DEFAULT '',
        dedup_key TEXT NOT NULL DEFAULT '',
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (session_id) REFERENCES conversation_sessions(id) ON DELETE CASCADE,
        FOREIGN KEY (artifact_id) REFERENCES semantic_artifacts(id) ON DELETE CASCADE
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

      CREATE TABLE IF NOT EXISTS external_exports (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        export_type TEXT NOT NULL,
        format TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'succeeded'
          CHECK (status IN ('pending', 'running', 'succeeded', 'failed')),
        provider TEXT NOT NULL DEFAULT 'local_file',
        file_name TEXT NOT NULL DEFAULT '',
        mime_type TEXT NOT NULL DEFAULT '',
        content_preview TEXT NOT NULL DEFAULT '',
        source_span_refs TEXT NOT NULL DEFAULT '[]',
        error_message TEXT NOT NULL DEFAULT '',
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (session_id) REFERENCES conversation_sessions(id) ON DELETE CASCADE
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
      CREATE INDEX IF NOT EXISTS idx_transcript_segments_timeline
        ON transcript_segments(audio_segment_id, start_ms);
      CREATE INDEX IF NOT EXISTS idx_speaker_segments_audio
        ON speaker_segments(audio_segment_id, start_ms);
      CREATE INDEX IF NOT EXISTS idx_transcript_jobs_status
        ON transcript_jobs(status);
      CREATE INDEX IF NOT EXISTS idx_transcript_correction_patterns_enabled
        ON transcript_correction_patterns(enabled);
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
      CREATE INDEX IF NOT EXISTS idx_todo_candidates_status
        ON todo_candidates(status);
      CREATE UNIQUE INDEX IF NOT EXISTS idx_todo_candidates_dedup_key
        ON todo_candidates(dedup_key)
        WHERE dedup_key <> '';
      CREATE INDEX IF NOT EXISTS idx_processing_jobs_status
        ON processing_jobs(status);
      CREATE INDEX IF NOT EXISTS idx_external_exports_session
        ON external_exports(session_id);
      CREATE INDEX IF NOT EXISTS idx_external_exports_created_at
        ON external_exports(created_at DESC);
      "#,
        )
        .map_err(|error| format!("初始化表结构失败: {error}"))?;

    ensure_app_settings_columns(&connection)?;
    ensure_conversation_sessions_columns(&connection)?;
    ensure_semantic_artifact_type_constraint(&connection)?;
    ensure_model_invocations_columns(&connection)?;
    ensure_transcript_segments_v05_columns(&connection)?;
    ensure_todos_v07_schema(&connection)?;

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
        allow_cloud_fallback
      ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
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
                1
            ],
        )
        .map_err(|error| format!("初始化默认设置失败: {error}"))?;

    connection
        .execute(
            r#"
      INSERT OR IGNORE INTO local_model_status (
        provider,
        model_name,
        cache_dir,
        download_status,
        download_progress,
        offline_available,
        device_recommendation
      ) VALUES (
        'local_whisperkit',
        'large-v3-turbo',
        '~/Library/Application Support/com.smarttodo.desktop/models/argmax',
        'available',
        100,
        1,
        'Apple Silicon 推荐 large-v3-turbo；Intel 机型建议 small/base'
      )
      "#,
            [],
        )
        .map_err(|error| format!("初始化本地模型状态失败: {error}"))?;

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
        updated_at,
        owner,
        due_at,
        priority,
        source_span_refs,
        candidate_id,
        dedup_key
      ) VALUES (
        'todo_seed_001',
        ?1,
        '确认 MiniMax M3 语义配置',
        '补全语音转写配置，并确认 Todo 只进入 MiniMax M3 语义产物边界',
        'open',
        CURRENT_TIMESTAMP,
        '请确认 ASR 配置和 MiniMax M3 语义入口已就绪。',
        'minimax-m3',
        'trace_seed_001',
        CURRENT_TIMESTAMP,
        '',
        '',
        'medium',
        '[]',
        '',
        'seed::确认 minimax m3 语义配置'
      )
      "#,
            params![session_id],
        )
        .map_err(|error| format!("初始化示例 Todo 失败: {error}"))?;

    Ok(())
}
