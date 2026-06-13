# Codex 协作入口

本目录下的 agent 与 instruction 是 Codex 在本仓库工作时需要遵循的项目级规则。执行任务时应优先读取本文件，再按任务范围读取对应文件。

## 使用顺序

1. 先读取 `.github/copilot-instructions.md`，确认项目真实技术栈、目录结构和开发边界。
2. 按任务类型读取 agent：
   - 新功能或 UI 优化：`.github/agents/feat.agent.md`
   - Bug、构建失败、运行异常：`.github/agents/fix.agent.md`
   - 代码审查：`.github/agents/review.agent.md`
3. 按改动范围读取 instruction：
   - 前端：`.github/instructions/frontend/*.instructions.md`
   - Tauri / Rust / 本地服务：`.github/instructions/backend/*.instructions.md`
4. 再读取 `AI文档/` 中对应 PRD、设计文档、技术设计、接口契约。

## Codex 执行要求

1. 默认使用中文回复用户。
2. 不在 `main` 或 `develop` 分支直接开发。
3. 改文件前说明改动点和风险；用户确认后再执行。
4. 优先使用现有实现与依赖，不引入未确认的新框架。
5. 前端改动必须运行 `npm run build`。
6. Rust/Tauri 改动必须运行 `cargo check --manifest-path src-tauri/Cargo.toml` 或更合适的最小验证。
7. 代码或功能变更后，同步检查 `AI文档/` 是否需要更新。
8. 保留日志/排障能力：会话文稿、失败原因、提取路径、回退状态、运行时状态。
9. 日志和 UI 不展示完整密钥、完整音频内容、完整会话文稿或本地敏感路径。

## 当前任务适配

当前仓库不是 Web 全栈项目，也没有 FastAPI、PostgreSQL、Tailwind、Zustand、React Router。Codex 不应按这些不存在的栈生成代码。
