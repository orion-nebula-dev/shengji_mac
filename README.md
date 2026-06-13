# 智能 Todo

一个运行在 macOS 桌面端的声记工作台，目标是把“录音 / 音频导入 -> 本地转写 -> 说话人分离 -> 转写修正 -> MiniMax M3 类型化语义理解 -> Todo / 摘要 / 脑图 / 导出”串成可追溯的桌面工作流。

当前实现基于 `Tauri 2 + Rust + React + TypeScript + SQLite`，前端负责配置与结果展示，Rust 负责录音、数据库和模型调用编排。

## 当前能力

1. 支持桌面端启动与本地 SQLite 持久化。
2. 支持设置录音开关、切片时长、空闲触发时间。
3. 支持 ASR、Speaker、Semantic、Embedding、Export provider 边界配置。
4. 支持真实麦克风录音，并将录音文件保存到本地。
5. 支持本地音频导入、离线转写评估时间轴、说话人标签修正与失败转写任务重试。
6. Todo 入口固定为 MiniMax M3 语义边界，候选产物先进入 `semantic_artifacts(type='todo_extraction')`。
7. 支持 Todo 完成/未完成切换。
8. 支持本地模型缓存状态展示、处理任务记录、失败原因记录与基础稳定性保护。

## v0.5 处理链路

```text
录音开始 / 本地音频导入
-> 本地生成或读取 wav 文件
-> 写入 audio_segments
-> 创建 transcription 任务
-> 通过 local_whisperkit / Argmax 边界生成离线转写评估时间轴
-> 写入 transcript_segments / speakers / speaker_segments
-> 支持说话人改名、时间轴跳转、错误片段标注与失败任务重试
-> 创建 conversation_sessions
-> 创建 todo_extraction 任务
-> 登记 semantic_artifacts(type='todo_extraction')
-> 后续版本进入 Todo 候选确认与正式 todos
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
npm run tauri:build
```

## 默认本地数据位置

录音文件目录：

```text
~/Library/Application Support/com.smarttodo.desktop/recordings
```

SQLite 数据库：

```text
~/Library/Application Support/com.smarttodo.desktop/smart-todo.sqlite
```

## 配置说明

### ASR 配置

当前项目已适配火山语音识别配置项：

1. `ASR 提交地址`
2. `ASR 查询地址`
3. `ASR 资源 ID`
4. `ASR 模型类型`
5. `ASR API Key`

### MiniMax M3 语义配置

1. `M3 调用地址`
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

详细文档见 [AI文档索引](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/README.md>)：

1. [工作区目录规范](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/00-项目总览/工作区目录规范.md>)
2. [声记-版本迭代与项目架构方案](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/02-技术方案/声记-版本迭代与项目架构方案.md>)
3. [声记-版本迭代目标与代码归档方案](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md>)
4. [MiniMax-能力验证说明](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/06-验证报告/能力验证/MiniMax-能力验证说明.md>)
5. [开发规范](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/05-规范制度/开发规范.md>)
6. [Git规范](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/05-规范制度/Git规范.md>)
7. [发布说明_v0.2.0](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/04-发布记录/发布说明_v0.2.0.md>)
8. [发布说明_v0.3.0](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/04-发布记录/发布说明_v0.3.0.md>)
9. [发布说明_v0.4.0](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/04-发布记录/发布说明_v0.4.0.md>)
10. [发布说明_v0.5.0](</Users/wwh/Documents/AI项目管理/shengji_mac/AI文档/04-发布记录/发布说明_v0.5.0.md>)

过时的一期文档、旧 v2.0 PRD 和旧设计包已归档到：

```text
AI文档/废纸篓/2026-06-12-旧方案归档/
```

## 当前边界

当前为 v0.5 转写评估版本，尚未完成：

1. 自动 30 秒滚动切片录音。
2. 真实 Argmax local server / CLI 推理执行与模型下载器。
3. 声纹识别与特定用户过滤。
4. SpeakerKit 真实说话人分离推理接入。
5. 多设备同步。
