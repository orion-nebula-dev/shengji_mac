# shengji_mac Codex Instructions

## 结论

本项目是 macOS 桌面端声记工作台，技术栈为 `Tauri 2 + Rust + React + TypeScript + SQLite`。Codex 开发时优先遵守本文；更细规则见 `.codex/instructions/` 和 `.agents/skills/`。

## 事实来源优先级

1. 用户当前明确指令。
2. `AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md`
3. `AI文档/02-技术方案/声记-版本迭代与项目架构方案.md`
4. `AI文档/05-规范制度/开发规范.md`
5. `AI文档/05-规范制度/Git规范.md`
6. `README.md`
7. `.codex/instructions/` 下相关分片。

若文档冲突，以更接近当前版本目标的 `AI文档` 为准，并在交付说明中指出冲突。

## 当前主线

核心链路：

```text
录音 / 音频导入
-> 语音转写
-> 说话人分离
-> 转写修正
-> MiniMax M3 类型化语义理解
-> semantic_artifacts / model_invocations
-> Todo / 摘要 / 纪要 / 脑图 / 导出
```

关键边界：

1. Todo 语义入口固定为 MiniMax M3，通过 `semantic_artifacts` 承载候选产物。
2. 禁止重新引入旧 Qwen / llama.cpp Todo runtime 作为默认、兜底或 legacy 路径。
3. 无老用户前提下，旧迁移兼容可以从简，不为已删除旧路径保留复杂回退。
4. 用户音频、完整转写文本、API Key、模型缓存路径均按敏感信息处理。

## 按任务读取规则

| 任务范围 | 读取 |
| --- | --- |
| Rust/Tauri 架构、`src-tauri/src/lib.rs` 拆分 | `.codex/instructions/backend-rust/architecture.instructions.md` |
| Domain、DTO、command、状态枚举 | `.codex/instructions/backend-rust/domain-commands.instructions.md` |
| Provider、SQLite、jobs、模型调用 | `.codex/instructions/backend-rust/providers-infra-jobs.instructions.md` |
| React、TypeScript、Tauri invoke 客户端 | `.codex/instructions/frontend/project.instructions.md` |
| UI 视觉、macOS 风格、交互状态 | `.codex/instructions/frontend/visual-style.instructions.md` |
| 测试、构建、完成前验证 | `.codex/instructions/quality/testing-verification.instructions.md` |
| Git、文档、版本归档 | `.codex/instructions/workflow/git-docs-release.instructions.md` |
| Code review、功能 review、subagent 边界 | `.codex/instructions/workflow/review-subagents.instructions.md` |
| Agent team 编排、并行/串行、上下文压缩控制 | `.codex/instructions/workflow/agent-team.instructions.md` |

也可以显式调用 repo skills：

1. `$shengji-architecture`
2. `$shengji-frontend`
3. `$shengji-review`
4. `$shengji-release`
5. `$shengji-agent-team`

## 开发方式

1. 简单任务直接执行；复杂或跨模块任务先给简要计划。
2. 先读现有代码和相关规则，再改文件。
3. 小步实现，避免混入无关重构。
4. 不覆盖、不回滚用户未提交改动。
5. 发生代码或文档改动后执行最小可行验证。
6. 交付时说明变更摘要、影响范围、验证结果和剩余风险。

## Agent Team 默认规则

1. 主 agent 负责范围、计划、最终决策、文件编辑和交付说明。
2. Subagent 优先用于读多写少的探索、审查、日志/测试输出分析和版本验收检查。
3. 默认 subagent 只读、只输出摘要；除非用户明确要求，不让 subagent 直接编辑文件。
4. 多个 subagent 可并行处理互不重叠的只读任务；涉及同一文件编辑、迁移、版本号、发布归档时必须串行。
5. 主会话只保留结论、阻塞项、决策和验证结果；不要把大段日志、源码摘录、探索过程塞回主会话。
6. 具体 agent team 编排见 `.codex/instructions/workflow/agent-team.instructions.md`。

## 常用验证

前端或类型相关：

```bash
npm run build
```

Rust 编译检查：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Rust 行为或解析逻辑：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

工具脚本：

```bash
npm run test:tools
```

提交前空白检查：

```bash
git diff --check
```

## Git 边界

1. 项目推荐分支为 `feature-*` / `hotfix-*`，不在 `main`、`dev`、`test` 直接开发。
2. Commit 使用 `<type>(<scope>): <summary>`。
3. 不提交模型权重、SQLite 数据库、录音、缓存、密钥、生成产物。
4. 不执行 `git reset --hard`、强推、批量删除等高影响操作，除非用户明确确认。
