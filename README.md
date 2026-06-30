# 声记

一个运行在 macOS 桌面端的声记工作台,目标是把"录音 -> 本地转写 -> 说话人分离 -> 转写修正 -> MiniMax M3 类型化语义理解 -> Todo / 摘要 / 脑图 / Moment / 深度研究 / 翻译 / 多语言导出"串成可追溯的桌面工作流。

当前实现基于 **Swift / SwiftUI / AppKit / WhisperKit / Core ML / SQLite** 的原生 macOS 应用,代码主目录 `macos/RecordingAgent/`,使用 Swift Package Manager + Xcode 工程双构建路径。Swift 原生 App 已于 2026-06-18 通过 v1.0 MVP 真实麦克风 smoke 验收(详见 `AI文档/06-验证/recording-agent-v1.0-mvp-validation-2026-06-18.md`)。

v1.2.x 之前的 Tauri 2 + Rust + React + Vite 实现已废弃,不再维护也不再作为参考占位。

## 当前能力(基于 v1.0 MVP 验收)

1. 桌面端启动 + 本地 SQLite 持久化(`macos/RecordingAgent/Sources/RecordingAgentCore/SQLiteRecordingStore.swift`)。
2. 主动麦克风录音 + 16kHz mono WAV 保存到本地。
3. 固定 15 秒切片(`Audio.swift` 中的 `SlicePlanner`)。
4. 本地 WhisperKit / Core ML ASR 实时转写,默认模型 `small`,可选 `tiny` / `base`。
5. 首页确认片段 + 历史列表 + 详情回看(含音频播放、转写、时间戳、失败片段、Agent 事实)。
6. Agent 事件 / 步骤 / 产物 SQLite 持久化。
7. XCTest 单元测试 + XCUITest UI 测试 + `swift test` + `xcodebuild test`。
8. 本地 Release 打包脚本(`Scripts/package-local-release.sh`),自动归档到 `其他文件/build/v1.0.0/YYYY-MM-DD/`。

## v1.0 MVP 处理链路

```text
用户主动开始麦克风记录
-> NativeAudioCaptureService 采集 16kHz mono PCM
-> SlicePlanner 按 15 秒切分
-> 写入 SQLiteRecordingStore 的 recording / segment 表
-> WhisperKitASREngine 对每个 segment 跑本地 ASR
-> 写入 ASR transcript + Agent 事件 / 步骤 / 产物
-> 首页确认片段(RecordingAgentViewModel + Views)
-> 历史列表(HistoryView)
-> 详情回看(DetailView + MarkdownTranscriptExporter)
```

## 技术栈

| 层级 | 技术 |
| --- | --- |
| 桌面容器 | 原生 macOS App(Xcode 15+ / SwiftPM) |
| 核心语言 | Swift 5.10+ |
| UI | SwiftUI 为主,AppKit 按需辅助 |
| 音频采集 | AVFoundation / AVAudioEngine(`NativeAudioCaptureService.swift`) |
| 本地 ASR | WhisperKit / Core ML(`argmaxinc/argmax-oss-swift` 1.0.0) |
| 本地存储 | SQLite C API(`SQLiteRecordingStore.swift`) |
| 测试 | XCTest / XCUITest / xcodebuild / swift test |
| 打包 | `macos/RecordingAgent/Scripts/package-local-release.sh`(codesign + ditto + shasum) |

## 本地运行

需要本机已安装:

1. macOS 13+ (Apple Silicon 推荐)
2. Xcode 15+ / Swift 5.10+
3. 命令行工具 `xcode-select --install`
4. 可选: `brew install xcodegen`(用于 `Scripts/generate-xcodeproj.sh`)

### 1. 生成 / 刷新 Xcode 工程(可选)

`RecordingAgent.xcodeproj/` 已存在,直接用即可。仅在 `project.yml` / `Package.swift` 改了之后才需要重生成:

```bash
bash macos/RecordingAgent/Scripts/generate-xcodeproj.sh
```

### 2. Swift Package Manager 测试

```bash
swift test --package-path macos/RecordingAgent
```

### 3. Xcode 工程测试

```bash
xcodebuild test \
  -project macos/RecordingAgent/RecordingAgent.xcodeproj \
  -scheme RecordingAgent \
  -destination 'platform=macOS'
```

### 4. 编译 Debug / Release

```bash
xcodebuild build \
  -project macos/RecordingAgent/RecordingAgent.xcodeproj \
  -scheme RecordingAgent \
  -configuration Debug

xcodebuild build \
  -project macos/RecordingAgent/RecordingAgent.xcodeproj \
  -scheme RecordingAgent \
  -configuration Release
```

### 5. 核心自检 / Smoke

```bash
swift run --package-path macos/RecordingAgent RecordingAgentCoreSelfTests
swift run --package-path macos/RecordingAgent RecordingAgentSmoke
```

### 6. 本地 Release 打包

```bash
bash macos/RecordingAgent/Scripts/package-local-release.sh
```

产物自动归档到本地-only `其他文件/build/v1.0.0/YYYY-MM-DD/`(同日重复归档自动追加 `-2`、`-3` 后缀)。

## 默认本地数据位置

实际数据目录由 `macos/RecordingAgent/Sources/RecordingAgentApp.swift` 与 `SQLiteRecordingStore.swift` 中的常量决定。常见位置(参考 Swift Application Support 默认行为):

```text
~/Library/Application Support/com.shengji.recording-agent/
  ├── shengji.sqlite         # SQLite 数据库
  ├── recordings/            # 录音 WAV
  └── models/whisperkit/     # WhisperKit 模型缓存
```

不要在此 README 硬编码路径,改 `RecordingAgentApp.swift` 常量后可能漂移。

## 配置说明

### WhisperKit 模型

1. 默认模型:`small`(精度 / 体积平衡)
2. 可选:`tiny`、`base`、`small`
3. 模型下载由 `WhisperKitModelManager` + `WhisperKitModelFileDownloader.swift` 处理(`/usr/bin/curl` 下载 Core ML + tokenizer)
4. 模型缓存目录见上一节;未就绪时给出明确缺失提示

### 应用配置

Swift MVP 当前只支持 v1.0 PRD 范围(主动录音 + 切片 + 实时转写 + 历史回看),**不含**:

- MiniMax M3 语义理解(由 v1.0 PRD 明确排除)
- 说话人分离 / SpeakerKit(由 v1.0 PRD 明确排除)
- 翻译 / RAG / 声纹 / 团队能力(由 v1.0 PRD 明确排除)
- 菜单栏入口 / 暂停 / 系统音频(由 v1.0 PRD 明确排除)
- 公证 / App Store 上架 / Developer ID 签名(由 v1.0 PRD 明确排除)

## 稳定性保护

1. SPM 与 Xcode 工程测试全过(`AI文档/06-验证/recording-agent-v1.0-mvp-validation-2026-06-18.md` 记录 21 tests, 0 failures)。
2. `RecordingAgentCoreSelfTests` 退出码 0,核心状态机 / 切片计划 / 持久化通过。
3. `RecordingAgentSmoke` 退出码 0,真实麦克风 35.8s 录音 + 3 段成功转写(2026-06-18 验收)。
4. Release build `** BUILD SUCCEEDED **`,本地 `.app` 与 zip 已归档到 `其他文件/build/v1.0.0/2026-06-18-xcode-release/`。

## 文档目录

本地详细文档见 [AI文档索引](AI文档/README.md)。`AI文档/` 是本地-only 文档区,受根 `.gitignore` 保护,不上传远端;远端仓库只保留代码、构建配置和必要版本号变更。

当前常用入口:

1. [总览骨架](AI文档/00-总览/README.md)
2. [产品方案目录](AI文档/01-产品方案/README.md)
3. [Recording Agent 产品线 PRD](AI文档/01-产品方案/recording-agent-product-line-prd.md)
4. [Recording Agent 版本需求拆解](AI文档/01-产品方案/recording-agent-requirements-breakdown.md)
5. [v1.0 MVP 产品需求](AI文档/01-产品方案/v10-mvp-local-recording-realtime-transcription-prd.md)
6. [v1.0 MVP 技术架构](AI文档/03-技术/MVP/v10-technical-architecture.md)
7. [v1.0 MVP 实施计划](AI文档/03-技术/MVP/v10-implementation-plan.md)
8. [技术拆解与实现状态检查](AI文档/03-技术/plan&task.md)
9. [Design Token](AI文档/02-设计/design-tokens.md)
10. [Git 规范](AI文档/04-规范/Git规范.md)
11. [v1.0.0 MVP 发布说明](AI文档/05-发布/v1.0.0.md)(待写)
12. [v1.0 MVP 验收记录](AI文档/06-验证/recording-agent-v1.0-mvp-validation-2026-06-18.md)
13. [v1.0 MVP 验收截图](AI文档/06-验证/recording-agent-home-success-2026-06-18.png)
14. [历史文档归档(99-废纸篓)](AI文档/99-废纸篓/待找回历史文档.md)
15. [v1.0 ~ v1.2 历史快照(本地-only)](AI文档/100-UnderstandAnything/README.md)
16. [Build 产物归档规范](其他文件/build/README.md)

## 当前边界

当前为 v1.0.0 原生 macOS MVP 验收通过版本,尚未完成(全部由 v1.0 PRD 明确排除,**v1.1+ 路线需另开 PRD**):

1. 菜单栏入口 / 暂停 / 系统音频。
2. 说话人分离(SpeakerKit)与声纹识别。
3. MiniMax M3 语义理解 / Todo / 摘要 / 脑图 / Moment / 深度研究。
4. 翻译 / 多语言导出。
5. 自动 30 秒滚动切片(v1.0 固定 15 秒)。
6. 真实联网深度研究检索与外部资料引用。
7. 多设备同步 / 云端分享 / 外部同步。
8. 播客脚本 / TTS provider / 音频生成入口。
9. Developer ID 签名 / 公证 / App Store 上架 / 外部分发。

旧 Tauri v1.2.x 栈(2026-06-30 之前的主路径)的 v1.1+ 能力(语义 / Todo / 翻译 / 脑图 / Moment / 深度研究 / 说话人分离 / 导出中心)在 Swift MVP 接管后**全部下线**,需在 v1.1+ 路线中按 PRD 流程重新评估是否在原生栈复现。
