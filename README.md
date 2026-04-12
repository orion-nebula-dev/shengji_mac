# 智能 Todo

一个运行在 macOS 桌面端的智能 Todo 原型应用，目标是把“环境音录制 -> 语音转写 -> 会话聚合 -> Todo 提取 -> Todo 管理”串成完整业务闭环。

当前实现基于 `Tauri 2 + Rust + React + TypeScript + SQLite`，前端负责配置与结果展示，Rust 负责录音、数据库和模型调用编排。

## 当前能力

1. 支持桌面端启动与本地数据持久化。
2. 支持设置录音开关、切片时长、空闲触发时间。
3. 支持分别配置 ASR 模型和 Todo 提取模型。
4. 支持真实麦克风录音，并将录音文件保存到本地。
5. 支持将本地录音文件送入 ASR，写入转写结果。
6. 支持基于会话文稿调用大模型提取 Todo，并落库展示。
7. 支持 Todo 完成/未完成切换。
8. 支持处理任务记录、失败原因记录与基础稳定性保护。

## 一期处理链路

```text
录音开始
-> 本地生成 wav 文件
-> 写入 audio_segments
-> 创建 transcription 任务
-> 调用 ASR 转中文
-> 写入 transcript_segments
-> 创建 conversation_sessions
-> 创建 todo_extraction 任务
-> 调用 Todo 模型提取结构化任务
-> 写入 todos
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

### Todo 提取模型配置

1. `Todo Base URL`
2. `Todo Model Name`
3. `Todo API Key`

## 稳定性保护

当前实现已包含：

1. 模型请求有限次重试。
2. `429` 与 `5xx` 的基础退避重试。
3. 空会话、占位会话跳过 Todo 提取。
4. 手动刷新当前会话的冷却保护。

## 文档目录

详细文档见 [AI文档](</Users/wwh/Documents/AItools/项目文件夹/录音app_MAC/V3/AI文档>)：

1. `智能Todo_PRD.md`
2. `技术设计文档.md`
3. `OpenAPI接口契约.md`
4. `数据库DDL设计.md`
5. `开发规范.md`
6. `UI规范.md`

## 当前边界

当前仍属于一期原型 / 工程验证阶段，尚未完成：

1. 自动 30 秒滚动切片录音。
2. 更完整的失败任务重试管理界面。
3. 声纹识别与特定用户过滤。
4. 本地模型正式切换。
5. 多设备同步。
