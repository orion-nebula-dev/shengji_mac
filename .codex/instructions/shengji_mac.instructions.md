---
description: "shengji_mac Codex project entrypoint. Read this first, then load only the relevant split instructions for the current change."
---

# shengji_mac Codex Instructions

## 1. 入口结论

`shengji_mac` 是 macOS 桌面端智能 Todo / 声记工作台项目，技术栈为 `Tauri 2 + Rust + React + TypeScript + SQLite`。后续开发目标是从旧“录音转 Todo 原型”演进为“本地转写 + 说话人分离 + MiniMax M3 类型化语义理解 + 统一 AI 产物”的桌面工作台。

## 2. 事实来源优先级

1. 用户当前明确指令。
2. `AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md`
3. `AI文档/02-技术方案/声记-版本迭代与项目架构方案.md`
4. `AI文档/05-规范制度/开发规范.md`
5. `AI文档/05-规范制度/Git规范.md`
6. `README.md`
7. `.codex/instructions/` 下相关分片。

若冲突，以更接近当前版本目标的 `AI文档` 为准，并在交付说明中指出冲突。

## 3. 按改动范围读取分片

| 改动范围 | 必读 instructions |
| --- | --- |
| 项目定位、文档优先级、全局边界 | `core/project-context.instructions.md` |
| `src-tauri/src/lib.rs`、Tauri 初始化、Rust 模块拆分 | `backend-rust/architecture.instructions.md` |
| `src-tauri/src/domain/`、DTO、状态枚举、command 参数 | `backend-rust/domain-commands.instructions.md` |
| `src-tauri/src/providers/`、`infra/`、`jobs/`、AI 调用、SQLite | `backend-rust/providers-infra-jobs.instructions.md` |
| `src/**/*.tsx`、`src/lib/`、`src/types.ts` | `frontend/project.instructions.md` |
| 视觉优化、桌面交互、样式、布局 | `frontend/visual-style.instructions.md` |
| 测试、构建、验证、完成前检查 | `quality/testing-verification.instructions.md` |
| 分支、commit、文档同步、版本归档 | `workflow/git-docs-release.instructions.md` |
| code review、功能 review、subagent 协作边界 | `workflow/review-subagents.instructions.md` |
| agent team 编排、并行/串行、上下文压缩控制 | `workflow/agent-team.instructions.md` |

## 4. 默认工作方式

1. 简单任务直接执行；复杂或跨模块任务先给简要计划。
2. 先读现有代码和相关分片，再改文件。
3. 小步实现，避免混入无关重构。
4. 发生代码或文档改动后执行最小可行验证。
5. 不覆盖、不回滚用户未提交改动。
6. 交付时说明变更摘要、影响范围、验证结果、剩余风险。

## 5. 禁止事项

1. 禁止重新引入旧 Qwen / llama.cpp Todo runtime 作为默认或回滚路径。
2. 禁止提交 API Key、完整用户音频、完整转写文本、SQLite 数据库、模型权重、运行缓存和生成产物。
3. 禁止在未确认时执行破坏性 Git 操作。
4. 禁止把一次性聊天过程写成长期规范。
