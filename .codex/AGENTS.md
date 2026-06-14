# shengji_mac Codex Home Instructions

本文件用于 `CODEX_HOME=$(pwd)/.codex codex` 这种项目专属 Codex home 启动方式。

常规仓库启动时，Codex 应读取仓库根目录的 `AGENTS.md`。若两者冲突，以当前任务所在仓库的 `AGENTS.md` 和用户最新指令为准。

## 项目主线

`shengji_mac` 是 `Tauri 2 + Rust + React + TypeScript + SQLite` 的 macOS 声记工作台。

后续开发主线：

```text
录音 / 音频导入
-> 语音转写
-> 说话人分离
-> 转写修正
-> MiniMax M3 类型化语义理解
-> semantic_artifacts / model_invocations
-> Todo / 摘要 / 纪要 / 脑图 / 导出
```

## 必守边界

1. Todo 语义入口固定 MiniMax M3。
2. 禁止恢复旧 Qwen / llama.cpp Todo runtime。
3. 用户音频、完整转写文本、API Key、模型缓存路径均按敏感信息处理。
4. 详细规范按需读取 `.codex/instructions/`。

## 推荐技能

1. `$shengji-architecture`：Rust/Tauri 架构、domain、provider、jobs。
2. `$shengji-frontend`：React/TypeScript、桌面 UI、视觉风格。
3. `$shengji-review`：code review、功能 review、subagent 边界。
4. `$shengji-release`：Git、文档、版本发布归档。
5. `$shengji-agent-team`：agent team 编排、并行/串行边界、上下文压缩控制。
