# MiniMax 能力验证说明

文档状态：v1.0
生成日期：2026-06-12
验证工具：`tools/minimax-capability-probe.mjs`

## 1. 当前结论

截至 2026-06-12，MiniMax 官方文档和 OpenAPI 索引可以确认这些能力：

| 能力 | 官方路径 / 模型 | 结论 |
| --- | --- | --- |
| M3 语义理解 | `/v1/chat/completions`，`MiniMax-M3` | 可作为摘要、Todo、思维脑图、深度研究主模型 |
| TTS 文本转语音 | `/v1/t2a_v2`，`speech-2.8-hd`、`speech-2.8-turbo` 等 | 官方确认 |
| 长文本异步 TTS | `/v1/t2a_async_v2` | 官方确认 |
| 语音克隆 | `/v1/voice_clone` | 官方确认 |
| 语音管理 | `/v1/get_voice`、`/v1/delete_voice` | 官方确认 |
| 文件管理 | `/v1/files/*` | 官方确认 |
| ASR / STT 语音转写 | 未在官方文档索引和 OpenAPI paths 中出现 | 不应作为主线承诺 |

工程结论：

1. MiniMax M3 应作为语义理解 provider。
2. ASR 仍要保留独立 `AsrProvider`。
3. `/v1/audio/transcriptions` 可以作为实验探测路径，但不是官方文档确认路径。
4. 如果探测失败，应继续使用火山 ASR 或其他明确支持 ASR 的服务。

## 2. 你需要提供什么

不要把 API Key 发到聊天里。你只需要在本机终端设置环境变量：

```bash
export MINIMAX_API_KEY="你的 MiniMax API Key"
```

如果要验证 ASR，还需要准备一段短音频：

```text
建议格式：wav 或 mp3
建议时长：5-20 秒
建议内容：普通中文口语，背景噪音不要太大
```

示例路径：

```text
/Users/wwh/Desktop/minimax-asr-sample.wav
```

## 3. 已实现的验证工具

工具位置：

```text
tools/minimax-capability-probe.mjs
```

npm 脚本：

```bash
npm run probe:minimax
npm run test:tools
```

## 4. 不需要 Key 的验证

检查 MiniMax 官方文档索引和 OpenAPI 是否包含 ASR/STT：

```bash
npm run probe:minimax -- docs
```

当前已跑过一次，结果是：

```text
No official ASR/STT documentation page or OpenAPI path was found.
Treat ASR as unconfirmed.
```

发现的音频相关官方路径只有：

```text
/v1/t2a_v2
/v1/t2a_async_v2
/v1/query/t2a_async_query_v2
/v1/files/upload
/v1/voice_clone
/v1/voice_design
/v1/get_voice
/v1/delete_voice
/v1/files/retrieve
/v1/files/list
/v1/files/retrieve_content
/v1/files/delete
```

## 5. 需要 Key 的验证

### 5.1 验证模型列表

```bash
MINIMAX_API_KEY="你的 key" npm run probe:minimax -- models
```

用途：

1. 确认 key 是否有效。
2. 确认当前账号能看到哪些 OpenAI-compatible 模型。
3. 判断是否能直接调用 `MiniMax-M3`。

### 5.2 验证 MiniMax M3 语义能力

```bash
MINIMAX_API_KEY="你的 key" npm run probe:minimax -- chat --model MiniMax-M3
```

成功时说明：

1. API Key 可用。
2. `MiniMax-M3` 可作为语义模型。
3. 项目里的摘要、Todo、脑图、深度研究可以走 M3。

### 5.3 验证 TTS

TTS 会产生极少量调用费用，只在需要验证播客化或语音输出能力时运行：

```bash
MINIMAX_API_KEY="你的 key" npm run probe:minimax -- tts \
  --tts-model speech-2.8-turbo \
  --voice-id English_expressive_narrator
```

成功时说明：

1. Speech 2.8 TTS 可用。
2. 后续 v1.1 的播客化 / 语音输出可以考虑 MiniMax Speech。

### 5.4 探测 ASR

这个命令用于探测疑似 OpenAI-compatible ASR 路径：

```bash
MINIMAX_API_KEY="你的 key" npm run probe:minimax -- asr \
  --audio /Users/wwh/Desktop/minimax-asr-sample.wav
```

默认会依次尝试：

```text
speech-2.8-turbo
speech-2.8-hd
MiniMax-M3
```

也可以手动指定：

```bash
MINIMAX_API_KEY="你的 key" npm run probe:minimax -- asr \
  --audio /Users/wwh/Desktop/minimax-asr-sample.wav \
  --asr-models speech-2.8-turbo,speech-2.8-hd,MiniMax-M3
```

判断方式：

| 返回 | 结论 |
| --- | --- |
| `200` 且有 `text` / `segments` | 当前 key/区域可能支持该 ASR 路径，需要继续做准确率评估 |
| `404` | 当前路径不存在，MiniMax ASR 不通过这个 OpenAI-compatible 路径暴露 |
| `401` / `403` | Key、套餐或权限问题 |
| `400` | 请求格式或模型名不接受 |
| `5xx` | 服务端异常，不能证明能力不可用 |

## 6. 推荐验证顺序

```bash
npm run probe:minimax -- docs
MINIMAX_API_KEY="你的 key" npm run probe:minimax -- models
MINIMAX_API_KEY="你的 key" npm run probe:minimax -- chat --model MiniMax-M3
MINIMAX_API_KEY="你的 key" npm run probe:minimax -- asr --audio /path/to/sample.wav
```

如果 `chat` 成功、`asr` 失败，项目架构结论不变：

```text
MiniMax M3 = 摘要 / Todo / 思维脑图 / 深度研究
ASR = 独立 provider，继续使用火山或其他正式 ASR 服务
```

## 7. 验证结果如何落回项目方案

| 验证结果 | 架构决策 |
| --- | --- |
| M3 chat 成功 | 实现 `MinimaxSemanticProvider` |
| TTS 成功 | v1.1 可接 `MinimaxTtsProvider` |
| ASR 成功 | 增加 `MinimaxAsrProvider`，进入 v0.5 评测 |
| ASR 失败 | 保持 `AsrProvider` 独立，不把 M3 当转写模型 |
