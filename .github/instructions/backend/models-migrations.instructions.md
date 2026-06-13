---
applyTo: "src-tauri/src/**/*.rs, AI文档/数据库DDL设计.md"
---

# 本地数据与迁移规范

当前项目使用本地 SQLite 思路承载设置、音频片段、会话、Todo 和处理任务。

## 数据对象

- `app_settings`
- `audio_segments`
- `transcript_segments`
- `conversation_sessions`
- `todos`
- `processing_jobs`

## 规则

1. 数据结构变化必须同步 `AI文档/数据库DDL设计.md`。
2. 敏感字段如 API Key 不明文回显。
3. 会话、Todo、处理任务之间必须保留可追溯 ID。
4. 失败原因、provider、fallback 状态要持久化或可查询，便于排障。
5. 不修改已发布用户数据时需说明兼容策略；不兼容变更需写入发布说明。
