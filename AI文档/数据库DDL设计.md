# 数据库DDL设计

## 1. 文档目标

本文档定义智能 Todo 一期本地 SQLite 数据库的建表语句、索引策略、字段约束和迁移建议，作为实际数据库初始化与版本演进的依据。

## 2. 设计原则

1. 一期优先保证数据结构稳定和可追溯。
2. 密钥不明文入库，数据库仅保存安全存储引用标识。
3. 所有核心链路对象均保留时间字段和状态字段。
4. 通过索引优化 Todo 查询、会话回溯和任务排障。
5. 本地嵌入模型优先采用“子进程推理 + 主进程编排”的隔离方案。

## 3. 数据库基础约定

### 3.1 数据库信息

| 项目 | 内容 |
| --- | --- |
| 数据库类型 | SQLite |
| 字符编码 | UTF-8 |
| 主键类型 | TEXT |
| 时间字段格式 | ISO 8601 字符串或 SQLite DATETIME |

### 3.2 命名规则

1. 表名使用复数下划线风格，如 `audio_segments`。
2. 主键统一使用 `id`。
3. 外键字段使用 `{table_singular}_id` 风格。
4. 状态字段统一使用 `TEXT` + 枚举约束思路。

## 4. 初始化 SQL

### 4.1 启用外键

```sql
PRAGMA foreign_keys = ON;
```

### 4.2 `app_settings`

```sql
CREATE TABLE IF NOT EXISTS app_settings (
  id TEXT PRIMARY KEY,
  record_enabled INTEGER NOT NULL DEFAULT 0 CHECK (record_enabled IN (0, 1)),
  language TEXT NOT NULL DEFAULT 'zh-CN',
  chunk_seconds INTEGER NOT NULL DEFAULT 30 CHECK (chunk_seconds > 0),
  idle_trigger_seconds INTEGER NOT NULL DEFAULT 20 CHECK (idle_trigger_seconds > 0),
  provider_mode TEXT NOT NULL DEFAULT 'cloud' CHECK (provider_mode IN ('cloud', 'local')),
  asr_provider_type TEXT NOT NULL DEFAULT 'cloud'
    CHECK (asr_provider_type IN ('cloud')),
  todo_provider_type TEXT NOT NULL DEFAULT 'cloud'
    CHECK (todo_provider_type IN ('cloud', 'embedded_local')),
  asr_base_url TEXT NOT NULL DEFAULT '',
  asr_model_name TEXT NOT NULL DEFAULT '',
  asr_api_key_ref TEXT NOT NULL DEFAULT '',
  todo_base_url TEXT NOT NULL DEFAULT '',
  todo_model_name TEXT NOT NULL DEFAULT '',
  todo_api_key_ref TEXT NOT NULL DEFAULT '',
  local_todo_model_version TEXT NOT NULL DEFAULT '',
  allow_cloud_fallback INTEGER NOT NULL DEFAULT 1 CHECK (allow_cloud_fallback IN (0, 1)),
  local_todo_runtime_status TEXT NOT NULL DEFAULT 'not_ready'
    CHECK (local_todo_runtime_status IN ('not_ready', 'starting', 'ready', 'failed')),
  local_todo_last_health_check_at DATETIME,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL
);
```

初始化默认数据：

```sql
INSERT OR IGNORE INTO app_settings (
  id,
  record_enabled,
  language,
  chunk_seconds,
  idle_trigger_seconds,
  provider_mode,
  asr_provider_type,
  todo_provider_type,
  asr_base_url,
  asr_model_name,
  asr_api_key_ref,
  todo_base_url,
  todo_model_name,
  todo_api_key_ref,
  local_todo_model_version,
  local_todo_runtime_status,
  local_todo_last_health_check_at,
  created_at,
  updated_at
) VALUES (
  'default',
  0,
  'zh-CN',
  30,
  20,
  'cloud',
  'cloud',
  'cloud',
  '',
  '',
  '',
  '',
  '',
  '',
  '',
  'not_ready',
  NULL,
  CURRENT_TIMESTAMP,
  CURRENT_TIMESTAMP
);
```

设计说明补充：

1. `provider_mode` 作为兼容字段保留，长期建议逐步由 `asr_provider_type`、`todo_provider_type` 取代。
2. 第一阶段仅允许 `todo_provider_type` 切换到 `embedded_local`。
3. 本地模型运行状态与健康检查时间需持久化，便于 UI 展示与排障。

### 4.3 `audio_segments`

```sql
CREATE TABLE IF NOT EXISTS audio_segments (
  id TEXT PRIMARY KEY,
  file_path TEXT NOT NULL,
  started_at DATETIME NOT NULL,
  ended_at DATETIME NOT NULL,
  duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
  sample_rate INTEGER NOT NULL DEFAULT 16000 CHECK (sample_rate > 0),
  channels INTEGER NOT NULL DEFAULT 1 CHECK (channels > 0),
  has_effective_voice INTEGER NOT NULL DEFAULT 0 CHECK (has_effective_voice IN (0, 1)),
  voice_energy_score REAL,
  processing_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (processing_status IN ('pending', 'transcribed', 'failed', 'skipped')),
  trace_id TEXT,
  created_at DATETIME NOT NULL
);
```

### 4.4 `conversation_sessions`

```sql
CREATE TABLE IF NOT EXISTS conversation_sessions (
  id TEXT PRIMARY KEY,
  merged_text TEXT NOT NULL,
  started_at DATETIME NOT NULL,
  ended_at DATETIME NOT NULL,
  idle_trigger_seconds INTEGER NOT NULL CHECK (idle_trigger_seconds > 0),
  trigger_reason TEXT NOT NULL
    CHECK (trigger_reason IN ('idle_timeout', 'manual', 'forced_flush')),
  transcript_count INTEGER NOT NULL DEFAULT 0 CHECK (transcript_count >= 0),
  extraction_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (extraction_status IN ('pending', 'success', 'failed')),
  extraction_provider_used TEXT NOT NULL DEFAULT '',
  extraction_fallback_used INTEGER NOT NULL DEFAULT 0
    CHECK (extraction_fallback_used IN (0, 1)),
  extraction_fallback_reason TEXT NOT NULL DEFAULT '',
  trace_id TEXT,
  created_at DATETIME NOT NULL
);
```

### 4.5 `transcript_segments`

```sql
CREATE TABLE IF NOT EXISTS transcript_segments (
  id TEXT PRIMARY KEY,
  audio_segment_id TEXT NOT NULL,
  conversation_session_id TEXT,
  content TEXT NOT NULL,
  language TEXT NOT NULL DEFAULT 'zh-CN',
  asr_provider TEXT NOT NULL DEFAULT 'cloud',
  asr_model_name TEXT NOT NULL DEFAULT '',
  started_at DATETIME NOT NULL,
  ended_at DATETIME NOT NULL,
  confidence_score REAL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'success', 'failed')),
  error_message TEXT,
  trace_id TEXT,
  created_at DATETIME NOT NULL,
  FOREIGN KEY (audio_segment_id) REFERENCES audio_segments(id) ON DELETE CASCADE,
  FOREIGN KEY (conversation_session_id) REFERENCES conversation_sessions(id) ON DELETE SET NULL
);
```

### 4.6 `todos`

```sql
CREATE TABLE IF NOT EXISTS todos (
  id TEXT PRIMARY KEY,
  conversation_session_id TEXT NOT NULL,
  title TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'completed')),
  created_at DATETIME NOT NULL,
  completed_at DATETIME,
  source_text TEXT,
  source_audio_id TEXT,
  speaker_id TEXT,
  extraction_model_name TEXT NOT NULL DEFAULT '',
  trace_id TEXT,
  updated_at DATETIME NOT NULL,
  FOREIGN KEY (conversation_session_id) REFERENCES conversation_sessions(id) ON DELETE CASCADE
);
```

### 4.7 `processing_jobs`

```sql
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
  created_at DATETIME NOT NULL
);
```

## 5. 索引设计

### 5.1 `audio_segments` 索引

```sql
CREATE INDEX IF NOT EXISTS idx_audio_segments_started_at
  ON audio_segments(started_at);

CREATE INDEX IF NOT EXISTS idx_audio_segments_ended_at
  ON audio_segments(ended_at);

CREATE INDEX IF NOT EXISTS idx_audio_segments_effective_voice
  ON audio_segments(has_effective_voice);

CREATE INDEX IF NOT EXISTS idx_audio_segments_processing_status
  ON audio_segments(processing_status);

CREATE INDEX IF NOT EXISTS idx_audio_segments_trace_id
  ON audio_segments(trace_id);
```

### 5.2 `conversation_sessions` 索引

```sql
CREATE INDEX IF NOT EXISTS idx_conversation_sessions_started_at
  ON conversation_sessions(started_at);

CREATE INDEX IF NOT EXISTS idx_conversation_sessions_ended_at
  ON conversation_sessions(ended_at);

CREATE INDEX IF NOT EXISTS idx_conversation_sessions_trigger_reason
  ON conversation_sessions(trigger_reason);

CREATE INDEX IF NOT EXISTS idx_conversation_sessions_extraction_status
  ON conversation_sessions(extraction_status);

CREATE INDEX IF NOT EXISTS idx_conversation_sessions_trace_id
  ON conversation_sessions(trace_id);
```

### 5.3 `transcript_segments` 索引

```sql
CREATE INDEX IF NOT EXISTS idx_transcript_segments_audio_segment_id
  ON transcript_segments(audio_segment_id);

CREATE INDEX IF NOT EXISTS idx_transcript_segments_conversation_session_id
  ON transcript_segments(conversation_session_id);

CREATE INDEX IF NOT EXISTS idx_transcript_segments_started_at
  ON transcript_segments(started_at);

CREATE INDEX IF NOT EXISTS idx_transcript_segments_status
  ON transcript_segments(status);

CREATE INDEX IF NOT EXISTS idx_transcript_segments_trace_id
  ON transcript_segments(trace_id);
```

### 5.4 `todos` 索引

```sql
CREATE INDEX IF NOT EXISTS idx_todos_conversation_session_id
  ON todos(conversation_session_id);

CREATE INDEX IF NOT EXISTS idx_todos_status
  ON todos(status);

CREATE INDEX IF NOT EXISTS idx_todos_created_at
  ON todos(created_at);

CREATE INDEX IF NOT EXISTS idx_todos_title
  ON todos(title);

CREATE INDEX IF NOT EXISTS idx_todos_trace_id
  ON todos(trace_id);
```

### 5.5 `processing_jobs` 索引

```sql
CREATE INDEX IF NOT EXISTS idx_processing_jobs_job_type
  ON processing_jobs(job_type);

CREATE INDEX IF NOT EXISTS idx_processing_jobs_target_id
  ON processing_jobs(target_id);

CREATE INDEX IF NOT EXISTS idx_processing_jobs_status
  ON processing_jobs(status);

CREATE INDEX IF NOT EXISTS idx_processing_jobs_trace_id
  ON processing_jobs(trace_id);
```

## 6. 推荐查询场景

### 6.1 查询未完成 Todo

```sql
SELECT *
FROM todos
WHERE status = 'pending'
ORDER BY created_at DESC
LIMIT 50;
```

### 6.2 查看某个会话及其 Todo

```sql
SELECT *
FROM conversation_sessions
WHERE id = ?;

SELECT *
FROM transcript_segments
WHERE conversation_session_id = ?
ORDER BY started_at ASC;

SELECT *
FROM todos
WHERE conversation_session_id = ?
ORDER BY created_at ASC;
```

### 6.3 查看失败任务

```sql
SELECT *
FROM processing_jobs
WHERE status = 'failed'
ORDER BY created_at DESC
LIMIT 100;
```

## 7. 迁移建议

### 7.0 本地嵌入模型迁移建议

1. 为 `app_settings` 增加 `asr_provider_type`、`todo_provider_type` 字段。
2. 为 `app_settings` 增加本地模型版本、运行状态、健康检查时间字段。
3. 历史数据默认迁移为 `asr_provider_type='cloud'`、`todo_provider_type='cloud'`。
4. 首次启用本地模型时，再将 `todo_provider_type` 更新为 `embedded_local`。

### 7.1 一期版本号建议

建议增加数据库版本管理表：

```sql
CREATE TABLE IF NOT EXISTS schema_migrations (
  version TEXT PRIMARY KEY,
  applied_at DATETIME NOT NULL
);
```

首个版本建议命名：

1. `v1_0_0_initial_schema`

### 7.2 迁移原则

1. 禁止直接修改线上已有表结构后不保留迁移记录。
2. 新增字段优先使用可空字段或带默认值字段。
3. 枚举值扩展时需同步更新应用层校验。
4. 高风险结构调整采用“新表 + 数据迁移 + 切换”方式。

## 8. 风险与注意事项

1. SQLite 不适合存大量音频二进制，音频文件建议落本地文件系统。
2. `file_path` 若变更目录策略，需要同步迁移旧数据。
3. `processing_jobs.target_id` 当前为通用字段，不做外键约束，避免跨对象约束复杂化。
4. 如果后续增加“手动编辑 Todo 版本记录”，建议新增 `todo_histories` 表，不建议直接复用 `processing_jobs`。
5. 本地嵌入模型建议由独立运行时托管，不建议将大模型直接加载进桌面主进程。

## 9. 结论

这套 DDL 已覆盖一期核心链路，并为“内嵌 Todo 本地模型”预留了 Provider 类型、模型版本和运行状态字段。按这份文档可以进入 migration 编写和数据库初始化开发。
