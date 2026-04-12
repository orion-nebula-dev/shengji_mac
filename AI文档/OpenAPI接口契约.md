# OpenAPI 接口契约

## 1. 文档目标

本文档定义智能 Todo 一期对内接口契约，供前端、桌面核心服务、本地持久化模块联调使用。当前以本地应用内部 API 为主，协议风格参考 OpenAPI 3.1，便于后续扩展为本地 HTTP 服务或 RPC 网关。

## 2. 设计原则

1. 所有接口返回统一响应结构。
2. 设置、Todo、会话、处理任务分模块定义。
3. 敏感字段如 `api_key` 在读取接口中默认脱敏或不返回。
4. 所有时间字段统一使用 ISO 8601 字符串。

## 3. 通用约定

### 3.1 基础信息

| 项目 | 内容 |
| --- | --- |
| 协议版本 | OpenAPI 3.1 风格 |
| 基础路径 | `/api/v1` |
| 数据格式 | `application/json` |
| 字符编码 | `UTF-8` |

### 3.2 通用响应结构

```json
{
  "code": 0,
  "message": "ok",
  "data": {},
  "request_id": "req_123456"
}
```

字段说明：

1. `code`：业务状态码，`0` 表示成功。
2. `message`：业务说明。
3. `data`：实际返回数据。
4. `request_id`：请求链路 ID，便于排障。

### 3.3 业务状态码

| code | 含义 |
| --- | --- |
| 0 | 成功 |
| 4001 | 参数校验失败 |
| 4002 | 配置缺失 |
| 4003 | 资源不存在 |
| 4004 | 操作过于频繁 |
| 5001 | 转写任务失败 |
| 5002 | Todo 提取任务失败 |
| 5003 | 持久化失败 |

## 4. Schema 定义

### 4.1 AppSettings

```json
{
  "id": "default",
  "record_enabled": true,
  "language": "zh-CN",
  "chunk_seconds": 30,
  "idle_trigger_seconds": 20,
  "provider_mode": "cloud",
  "asr_submit_url": "https://api.example.com/asr/submit",
  "asr_query_url": "https://api.example.com/asr/query",
  "asr_resource_id": "volc.seedasr.auc",
  "asr_model_name": "bigmodel",
  "asr_api_key_masked": "sk-****",
  "todo_base_url": "https://api.example.com/todo",
  "todo_model_name": "todo-model-v1",
  "todo_api_key_masked": "sk-****",
  "created_at": "2026-04-12T10:00:00+08:00",
  "updated_at": "2026-04-12T10:00:00+08:00"
}
```

### 4.2 UpdateSettingsRequest

```json
{
  "record_enabled": true,
  "chunk_seconds": 30,
  "idle_trigger_seconds": 20,
  "language": "zh-CN",
  "provider_mode": "cloud",
  "asr_submit_url": "https://api.example.com/asr/submit",
  "asr_query_url": "https://api.example.com/asr/query",
  "asr_resource_id": "volc.seedasr.auc",
  "asr_model_name": "bigmodel",
  "asr_api_key": "sk-xxx",
  "todo_base_url": "https://api.example.com/todo",
  "todo_model_name": "todo-model-v1",
  "todo_api_key": "sk-yyy"
}
```

### 4.3 Todo

```json
{
  "id": "todo_001",
  "conversation_session_id": "session_001",
  "title": "给客户发送报价",
  "note": "今天下午发送最新报价版本，并确认税率口径",
  "status": "pending",
  "created_at": "2026-04-12T10:20:00+08:00",
  "completed_at": null,
  "source_text": "下午给客户发送报价，然后确认税率口径",
  "source_audio_id": "audio_001",
  "speaker_id": null,
  "updated_at": "2026-04-12T10:20:00+08:00"
}
```

### 4.4 TranscriptSegment

```json
{
  "id": "transcript_001",
  "audio_segment_id": "audio_001",
  "conversation_session_id": "session_001",
  "content": "下午给客户发送报价，然后确认税率口径",
  "started_at": "2026-04-12T10:19:00+08:00",
  "ended_at": "2026-04-12T10:19:30+08:00",
  "status": "success",
  "trace_id": "trace_001"
}
```

### 4.5 ConversationSession

```json
{
  "id": "session_001",
  "merged_text": "下午给客户发送报价，然后确认税率口径。明天上午和小王确认排期。",
  "started_at": "2026-04-12T10:19:00+08:00",
  "ended_at": "2026-04-12T10:20:20+08:00",
  "idle_trigger_seconds": 20,
  "trigger_reason": "idle_timeout",
  "transcript_count": 2,
  "extraction_status": "success",
  "trace_id": "trace_001",
  "created_at": "2026-04-12T10:20:20+08:00"
}
```

### 4.6 ProcessingJob

```json
{
  "id": "job_001",
  "job_type": "todo_extraction",
  "target_id": "session_001",
  "status": "success",
  "retry_count": 0,
  "max_retry_count": 3,
  "error_message": null,
  "trace_id": "trace_001",
  "started_at": "2026-04-12T10:20:21+08:00",
  "finished_at": "2026-04-12T10:20:23+08:00",
  "created_at": "2026-04-12T10:20:21+08:00"
}
```

## 5. 接口清单

### 5.1 设置模块

#### 5.1.1 获取设置

`GET /api/v1/settings`

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "settings": {
      "id": "default",
      "record_enabled": true,
      "language": "zh-CN",
      "chunk_seconds": 30,
      "idle_trigger_seconds": 20,
      "provider_mode": "cloud",
      "asr_submit_url": "https://api.example.com/asr/submit",
      "asr_query_url": "https://api.example.com/asr/query",
      "asr_resource_id": "volc.seedasr.auc",
      "asr_model_name": "bigmodel",
      "asr_api_key_masked": "sk-****",
      "todo_base_url": "https://api.example.com/todo",
      "todo_model_name": "todo-model-v1",
      "todo_api_key_masked": "sk-****"
    }
  },
  "request_id": "req_001"
}
```

#### 5.1.2 更新设置

`PUT /api/v1/settings`

请求体：`UpdateSettingsRequest`

校验规则：

1. `chunk_seconds` 必须大于 `0`。
2. `idle_trigger_seconds` 必须大于 `0`。
3. `asr_submit_url`、`asr_query_url`、`todo_base_url` 必须为合法 URL。
4. 当 `provider_mode=cloud` 时，双模型配置均必填。

响应：

```json
{
  "code": 0,
  "message": "settings updated",
  "data": {
    "updated": true
  },
  "request_id": "req_002"
}
```

失败响应：

```json
{
  "code": 4004,
  "message": "手动刷新过于频繁，请稍后再试",
  "data": {},
  "request_id": "req_005"
}
```

#### 5.1.3 校验设置

`POST /api/v1/settings/validate`

请求：

```json
{
  "chunk_seconds": 30,
  "idle_trigger_seconds": 20,
  "asr_base_url": "https://api.example.com/asr",
  "asr_model_name": "asr-model-v1",
  "todo_base_url": "https://api.example.com/todo",
  "todo_model_name": "todo-model-v1"
}
```

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "valid": true,
    "errors": []
  },
  "request_id": "req_003"
}
```

### 5.2 录音状态模块

#### 5.2.1 获取录音运行状态

`GET /api/v1/recording/status`

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "record_enabled": true,
    "runtime_status": "recording",
    "current_session_status": "idle_waiting",
    "last_audio_segment_id": "audio_010",
    "last_effective_voice_at": "2026-04-12T10:20:00+08:00"
  },
  "request_id": "req_004"
}
```

#### 5.2.2 手动刷新当前会话

`POST /api/v1/recording/flush-session`

用途：手动将当前缓冲区文稿强制触发 Todo 提取。

响应：

```json
{
  "code": 0,
  "message": "session flushed",
  "data": {
    "conversation_session_id": "session_002"
  },
  "request_id": "req_005"
}
```

### 5.3 Todo 模块

#### 5.3.1 查询 Todo 列表

`GET /api/v1/todos?status=pending&page=1&page_size=20`

查询参数：

| 参数 | 必填 | 说明 |
| --- | --- | --- |
| status | 否 | `pending` / `completed` |
| keyword | 否 | 标题或备注关键词 |
| page | 否 | 页码，默认 `1` |
| page_size | 否 | 每页数量，默认 `20` |

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "items": [],
    "pagination": {
      "page": 1,
      "page_size": 20,
      "total": 0
    }
  },
  "request_id": "req_006"
}
```

#### 5.3.2 获取 Todo 详情

`GET /api/v1/todos/{todo_id}`

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "todo": {}
  },
  "request_id": "req_007"
}
```

#### 5.3.3 更新 Todo

`PATCH /api/v1/todos/{todo_id}`

请求：

```json
{
  "title": "给客户发送报价",
  "note": "补充邮件附件",
  "status": "completed"
}
```

响应：

```json
{
  "code": 0,
  "message": "todo updated",
  "data": {
    "updated": true
  },
  "request_id": "req_008"
}
```

### 5.4 会话与文稿模块

#### 5.4.1 查询会话列表

`GET /api/v1/conversation-sessions?page=1&page_size=20`

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "items": []
  },
  "request_id": "req_009"
}
```

#### 5.4.2 获取会话详情

`GET /api/v1/conversation-sessions/{session_id}`

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "session": {},
    "transcript_segments": [],
    "todos": []
  },
  "request_id": "req_010"
}
```

### 5.5 处理任务模块

#### 5.5.1 查询处理任务列表

`GET /api/v1/processing-jobs?job_type=todo_extraction&status=failed`

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "items": []
  },
  "request_id": "req_011"
}
```

#### 5.5.2 重试处理任务

`POST /api/v1/processing-jobs/{job_id}/retry`

响应：

```json
{
  "code": 0,
  "message": "job retry scheduled",
  "data": {
    "job_id": "job_001"
  },
  "request_id": "req_012"
}
```

## 6. 错误响应示例

### 6.1 配置缺失

```json
{
  "code": 4002,
  "message": "todo model config missing",
  "data": null,
  "request_id": "req_err_001"
}
```

### 6.2 参数非法

```json
{
  "code": 4001,
  "message": "chunk_seconds must be greater than 0",
  "data": {
    "field": "chunk_seconds"
  },
  "request_id": "req_err_002"
}
```

## 7. 后续扩展预留

1. 后续若采用 Tauri command 而非本地 HTTP，可保持同样的请求响应结构。
2. 后续可新增 `/api/v1/audio-segments` 与 `/api/v1/transcript-segments` 调试接口。
3. 后续支持本地模型后，可在 `provider_mode` 和 provider schema 中扩展 `local` 类型。
