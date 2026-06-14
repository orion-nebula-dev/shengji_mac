---
applyTo: "src-tauri/src/providers/**/*.rs, src-tauri/src/infra/**/*.rs, src-tauri/src/jobs/**/*.rs, src-tauri/src/app/**/*.rs"
---

# Providers / Infra / Jobs Instructions

## Provider 抽象

Provider 层承载可替换 AI 能力。当前路线：

1. `AsrProvider`：语音转写，后续本地 WhisperKit / Argmax，当前可保留云端兜底。
2. `SpeakerProvider`：说话人分离，后续 LocalSpeakerKit。
3. `SemanticProvider`：MiniMax M3 类型化语义理解。
4. `EmbeddingProvider`：预留向量能力接口。
5. `ExportProvider`：预留导出能力接口。

规则：

1. Provider trait 放在稳定边界，具体实现放到对应 provider 子目录。
2. Provider 输入输出使用 domain 类型，不直接使用前端设置对象。
3. ASR、Speaker、Semantic 必须可独立配置。
4. MiniMax M3 只作为语义 provider，不作为 ASR 主线。
5. 禁止重新添加旧 Qwen / llama.cpp Todo runtime。

## MiniMax M3 语义链路

Todo、摘要、纪要等语义结果应先进入统一 AI 产物链路：

```text
model_invocations
semantic_artifacts(type='todo_extraction' | ...)
```

要求：

1. 保存模型调用记录时记录 provider、model、阶段、状态、耗时、错误摘要。
2. `semantic_artifacts` 存候选产物和来源追溯，不直接绕过进入最终用户确认态。
3. 失败时保留可重试状态和错误摘要。
4. 不记录完整 API Key、完整用户文稿或完整请求响应。

## Infra 层

`infra/` 负责外部副作用：

1. SQLite 连接、查询、迁移初始化。
2. HTTP client、超时、重试、错误映射。
3. 音频文件读写、录音文件目录。
4. Keychain 或系统安全存储。
5. 本地 sidecar 进程、端口检查、模型缓存目录。

规则：

1. SQLite schema 变化必须同步文档和验证。
2. 网络请求必须设置超时；对 `429` 和 `5xx` 可做有限重试。
3. 日志只记录摘要、长度、hash、状态码、耗时，不记录敏感正文。
4. 本地路径返回 UI 前需判断是否有隐私暴露风险。

## Jobs 层

`jobs/` 承载后台任务：

1. transcription job
2. session aggregation job
3. semantic extraction job
4. retry / failure handling

要求：

1. 每个任务有明确状态、阶段、失败原因和重试次数。
2. 任务处理应幂等，重复触发不能生成重复 Todo 或重复 artifact。
3. 长流程按阶段落库，避免进程退出后丢失上下文。
4. 任务处理不直接更新 React 本地状态，由 command/bootstrap 重新读取事实状态。

## App 服务层

`app/` 负责编排业务流程，例如：

```text
录音切片 -> 创建转写任务 -> 写 transcript -> 聚合 session -> 创建 semantic job -> 写 semantic artifact
```

约束：

1. app 层可以组合 domain、infra、providers、jobs。
2. app 层不 import React/TypeScript 概念。
3. app 层不直接生成 UI 文案，除非是 command 返回的用户可见摘要。
4. 复杂流程优先拆 helper 并补测试。
