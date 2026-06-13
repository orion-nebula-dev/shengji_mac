# 声记 Mac App / 智能 Todo

## 项目简介

声记是一款运行在 macOS 上的本地优先录音与智能 Todo 工具。当前版本聚焦“一期闭环”：持续录音、音频切片、语音转写、会话聚合、Todo 提取、Todo 管理、运行状态与排障日志展示。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面容器 | Tauri 2 |
| 前端 | React 19 + TypeScript + Vite |
| 样式 | 原生 CSS，集中在 `src/styles.css` |
| 桌面核心 | Rust，位于 `src-tauri/src` |
| 本地存储 | SQLite 与本地应用数据目录 |
| 模型运行时 | 内嵌 Todo 模型，`Qwen3-4B-Instruct-2507 Q4_K_M` + `llama.cpp` 子进程 |
| 文档 | `AI文档/` 下 PRD、设计文档、开发规范、接口契约 |

## 仓库结构

```text
shengji_mac/
├── src/                    # React 前端
│   ├── App.tsx             # 当前主界面与交互编排
│   ├── styles.css          # 全局视觉样式
│   ├── types.ts            # 前端共享类型
│   ├── data/mock.ts        # 浏览器原型兜底数据
│   └── lib/                # Tauri bridge 与本地状态封装
├── src-tauri/              # Tauri / Rust 桌面核心
│   ├── src/                # Tauri commands、录音、存储、模型运行时
│   ├── resources/          # 内嵌模型资源说明与 manifest
│   └── tauri.conf.json
├── AI文档/                 # PRD、设计文档、技术设计、接口契约
└── .github/                # Agent、CI 与项目指令
```

## 当前产品边界

- In Scope：录音开关、切片参数、ASR 配置、Todo 提取配置、会话文稿、Todo 列表、运行状态、失败排障、本地 Todo 模型状态。
- Out of Scope：多用户账号、云同步、飞书/OAuth、Web 后端、PostgreSQL、素材审核、日历正式同步、本地 ASR 正式接入。

## 开发规则

1. 用户界面文案、文档说明、日志说明默认使用中文。
2. 代码与文档统一使用 UTF-8 无 BOM。
3. 前端当前不使用 Tailwind、Zustand、React Router；不要引入这些依赖，除非需求明确且文档同步。
4. UI 层只负责展示和交互，不直接实现模型推理、录音采集或数据库细节。
5. Tauri commands 是前端与桌面核心的边界，返回结构要稳定、可序列化、可被 `src/types.ts` 表达。
6. 日志和排障信息不得输出完整音频内容、完整会话文稿、完整密钥或本地敏感路径。
7. 每次代码改动后至少运行 `npm run build`；涉及 Rust/Tauri 时补充 `cargo build` 或 `cargo test`。
8. 功能或接口变更必须同步 `AI文档/` 下对应 PRD、设计文档、技术设计或接口契约。
9. 不在 `main` 或 `develop` 分支直接开发；版本分支命名使用 `codex/gpt-5.5-vX.Y.Z`。

## Codex 适配

- Codex 进入本目录时先读 `.github/AGENTS.md`。
- 新功能、修复、审查分别使用 `.github/agents/feat.agent.md`、`.github/agents/fix.agent.md`、`.github/agents/review.agent.md` 的流程。
- 具体编码约束按 `.github/instructions/` 中与改动范围匹配的文件执行。
