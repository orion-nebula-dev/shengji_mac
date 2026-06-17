# 声记

一个运行在 macOS 桌面端的声记工作台，目标是把“录音 -> 本地转写 -> 说话人分离 -> 转写修正 -> MiniMax M3 类型化语义理解 -> Todo / 摘要 / 脑图 / Moment / 深度研究 / 翻译 / 多语言导出”串成可追溯的桌面工作流。

当前实现基于 `Tauri 2 + Rust + React + TypeScript + SQLite`，前端负责配置与结果展示，Rust 负责录音、数据库和模型调用编排。

## 当前能力

1. 支持桌面端启动与本地 SQLite 持久化。
2. 支持设置录音开关、切片时长、空闲触发时间。
3. 支持本地 ASR、Speaker、Semantic、Embedding、Export provider 边界配置。
4. 支持真实麦克风录音，并将录音文件保存到本地。
5. 支持录音片段查看、离线转写时间轴、说话人标签修正与失败转写任务重试。
6. Todo 入口固定为 MiniMax M3 语义边界，候选产物先进入 `semantic_artifacts(type='todo_extraction')`。
7. 支持 Todo 完成/未完成切换。
8. 支持本地 ASR runtime 探测、模型下载状态、缓存目录、处理任务记录、失败原因记录与基础稳定性保护。
9. 支持基于修正文稿和摘要生成思维脑图，节点可编辑、折叠、追溯来源并导出 Markdown / JSON。
10. 支持自动生成 Moment、Deep Research 草稿，并把研究结论转为 Todo 或脑图节点。
11. 支持导出中心：Markdown、SRT、JSON、本地分享快照、会话归档搜索、导出记录和 provider 成本 / 隐私 / 密钥状态展示。
12. 支持翻译与多语言导出：转写翻译、摘要翻译、多语言 Markdown / JSON / 快照模板和来源追溯。
13. 支持 v1.2 体验打磨：错误恢复面板、任务状态时间线、runtime_metrics 性能指标、转写修正 diff、脑图版本对比、Todo 候选编辑和 Design Token 雏形。

## v1.2 处理链路

```text
录音开始
-> 本地生成或读取 wav 文件
-> 写入 audio_segments
-> 创建 transcription 任务
-> 探测 argmax-cli / whisperkit-cli
-> 下载或校验本地 ASR 模型
-> 调用本地 CLI 生成录音片段时间轴
-> 写入 transcript_segments / speakers / speaker_segments
-> 支持说话人改名、时间轴跳转、错误片段标注与失败任务重试
-> 创建 conversation_sessions
-> 生成 transcript_revision / summary / meeting_minutes / todo_extraction
-> 确认 Todo 候选并写入 todos
-> 生成 mind_map / moment / deep_research
-> 在导出中心生成 Markdown / SRT / JSON / 本地分享快照
-> 生成 translation artifact，保留转写片段和摘要来源追溯
-> 在导出中心生成多语言 Markdown / JSON / 快照模板
-> 在运行状态页查看错误恢复、性能指标和任务状态时间线
-> 写入 external_exports 形成本地导出记录
```

## 技术栈

| 层级 | 技术 |
| --- | --- |
| 桌面容器 | Tauri 2 |
| 核心语言 | Rust |
| 前端 | React 19 + TypeScript + Vite |
| 本地存储 | SQLite |
| 音频采集 | cpal + hound |
| 云端调用 | reqwest |

## 本地运行

### 1. 安装依赖

```bash
npm install
```

需要本机已安装：

1. Node.js
2. Rust / Cargo
3. Tauri 2 所需的 macOS 构建环境

### 2. 启动前端

```bash
npm run dev
```

### 3. 启动桌面应用

```bash
npm run tauri:dev
```

### 4. 构建产物

```bash
npm run build
npm run build:archive
```

桌面安装包构建和归档：

```bash
npm run tauri:build
npm run build:archive -- --include-tauri-bundle
```

也可以使用组合命令：

```bash
npm run build:release
npm run tauri:build:archive
```

构建工具会先在根目录生成临时 `dist/` / `release/`，归档命令会移动到本地-only 的 `其他文件/build/vX.Y.Z/YYYY-MM-DD/`。同一版本同一天重复归档时，会自动使用 `YYYY-MM-DD-2`、`YYYY-MM-DD-3` 后缀，避免覆盖旧产物。

## 默认本地数据位置

录音文件目录：

```text
~/Library/Application Support/com.shengji.desktop/recordings
```

SQLite 数据库：

```text
~/Library/Application Support/com.shengji.desktop/shengji.sqlite
```

本地 ASR 模型缓存：

```text
~/Library/Application Support/com.soundworkbench.shengji/models/whisperkit
```

## 配置说明

### 本地 ASR 配置

当前项目使用本地优先 ASR：

1. 探测 `argmax-cli` 与 `whisperkit-cli`。
2. 默认模型为 `large-v3-v20240930_626MB`。
3. 可切换到 `base` 或 `tiny` 小模型。
4. 应用通过 CLI `transcribe --model ... --download-model-path ... --download-tokenizer-path ...` 预热下载模型，并记录缓存目录、下载进度、离线可用状态和错误提示。
5. 未安装 runtime 或模型未就绪时，录音转写会给出明确缺失提示。

### MiniMax M3 语义配置

1. `M3 调用地址`：默认 `https://api.minimaxi.com/v1/chat/completions`，用于 Token Plan 中国区 `sk-cp-...` key。
2. `M3 模型`
3. `M3 API Key`
4. `Todo 语义产物` 固定进入 `semantic_artifacts(type='todo_extraction')`

## 稳定性保护

当前实现已包含：

1. 模型请求有限次重试。
2. `429` 与 `5xx` 的基础退避重试。
3. 空会话、占位会话跳过 Todo 提取。
4. 手动刷新当前会话的冷却保护。
5. Todo 语义入口固定为 MiniMax M3，旧 Qwen / llama.cpp Todo runtime 不再作为默认、兜底或 legacy 路径。

## 文档目录

本地详细文档见 [AI文档索引](AI文档/README.md)。`AI文档/` 是本地-only 文档区，受根 `.gitignore` 保护，不上传远端；远端仓库只保留代码、构建配置和必要版本号变更。

当前常用入口：

1. [总览骨架](AI文档/00-总览/README.md)
2. [产品规划与任务索引](AI文档/01-产品方案/plan&task.md)
3. [v1.2 体验打磨规划](AI文档/01-产品方案/v12-experience-polish.md)
4. [Design Token](AI文档/02-设计/design-tokens.md)
5. [技术文档入口](AI文档/03-技术/README.md)
6. [Git 规范](AI文档/04-规范/Git规范.md)
7. [v1.2.1 发布说明](AI文档/05-发布/v1.2.1.md)
8. [v1.2.2 发布说明](AI文档/05-发布/v1.2.2.md)
9. [验证报告入口](AI文档/06-验证/README.md)
10. [v1.2.1 验收记录](AI文档/07-归档/v1.2.1/验收记录.md)
11. [待找回历史文档](AI文档/99-废纸篓/待找回历史文档.md)
12. [Understand Anything 快照说明](AI文档/100-UnderstandAnything/README.md)
13. [Build 产物归档规范](其他文件/build/README.md)

## 当前边界

当前为 v1.2.2 体验打磨与运行时可观测性版本，尚未完成：

1. 自动 30 秒滚动切片录音。
2. 声纹识别与特定用户过滤。
3. SpeakerKit 真实说话人分离推理接入。
4. 多设备同步。
5. 云端分享和外部同步。
6. 真实联网深度研究检索与外部资料引用。
7. 播客脚本、TTS provider 和音频生成入口。
