# Codex Instructions

本目录存放面向 Codex 后续开发的项目级 instructions。结构参考 `.github_副本` 的分层规范方式，但内容已适配当前 `shengji_mac` 项目。

## 入口

- `shengji_mac.instructions.md`：总入口、事实来源、按范围读取指引。

## 分片

| 范围 | 文件 |
| --- | --- |
| 项目上下文 | `core/project-context.instructions.md` |
| Rust / Tauri 架构 | `backend-rust/architecture.instructions.md` |
| Rust domain / DTO / command | `backend-rust/domain-commands.instructions.md` |
| Provider / infra / jobs | `backend-rust/providers-infra-jobs.instructions.md` |
| React 前端结构 | `frontend/project.instructions.md` |
| 前端视觉与交互 | `frontend/visual-style.instructions.md` |
| 测试与验证 | `quality/testing-verification.instructions.md` |
| Git、文档、发布归档 | `workflow/git-docs-release.instructions.md` |
| Code review / 功能 review 协作 | `workflow/review-subagents.instructions.md` |
| Agent team 编排与上下文控制 | `workflow/agent-team.instructions.md` |

## 使用原则

1. 只读取与当前改动范围相关的分片，不一次性加载所有 instructions。
2. 本目录只沉淀长期稳定规则，不记录一次性任务计划。
3. 具体版本目标以 `AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md` 为准。
4. 开发、Git、发布、归档规范以 `AI文档/05-规范制度/` 下文档为准。
5. 若本目录内容与 `AI文档` 冲突，优先遵守 `AI文档`，并同步修正本目录。
