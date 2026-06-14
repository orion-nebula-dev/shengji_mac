---
applyTo: "**/*"
---

# Git / Docs / Release Instructions

## 分支规范

以 `AI文档/05-规范制度/Git规范.md` 为准。

关键点：

1. 功能开发使用 `feature-*`。
2. 紧急修复使用 `hotfix-*`。
3. 不在 `main`、`dev`、`test` 上直接开发。
4. 晋升链路为 `feature-* / hotfix-* -> dev -> test -> main`。
5. `codex/*` 仅作为历史兼容命名，不是本项目推荐命名。

## 日常开发流

默认开发流：

```text
dev -> feature-* / hotfix-* -> push origin -> GitHub PR -> dev -> test -> main
```

要求：

1. 日常开发前先切到最新 `dev`，不要以 `main` 作为日常开发入口。
2. 新任务必须从 `dev` 拉出 `feature-*` 或 `hotfix-*`。
3. 本地 commit 后必须尽快 `git push -u origin <branch>`，不要长期只保留本地分支。
4. 功能 PR 默认目标分支为 `dev`，不得直接把工作分支 PR 到 `main`。
5. `main` 只用于发布合并、tag、发布后核对，不用于日常功能开发。

建议命令：

```bash
git fetch origin
git switch dev
git pull --ff-only origin dev
git switch -c feature-vX.Y.Z-topic
git push -u origin feature-vX.Y.Z-topic
```

如果当前人在 `main`：

```bash
git switch dev
git pull --ff-only origin dev
git switch -c feature-vX.Y.Z-topic
```

只有在准备发布、核对发布结果、或需要查看 `main` 正式状态时，才切回 `main`。

## Commit 规范

格式：

```text
<type>(<scope>): <summary>
```

常用 type：

1. `feat`
2. `fix`
3. `docs`
4. `refactor`
5. `test`
6. `build`
7. `chore`
8. `perf`

要求：

1. 每个 commit 只做一件事。
2. 不混入无关格式化或顺手重构。
3. 不提交未验证代码。
4. 不提交模型权重、数据库、缓存、密钥、录音、生成产物。

## 文档同步

以下变更必须同步 `AI文档`：

1. 产品需求、版本目标、验收标准变化。
2. 架构边界、模块职责、provider 接口变化。
3. SQLite schema、DTO、配置字段变化。
4. AI provider、隐私边界、密钥处理方式变化。
5. 版本完成、发布说明、归档清单、验收记录变化。

文档写法：

1. 记录事实、决策、影响和验证结果。
2. 不记录一次性聊天过程。
3. 不复制源码快照到文档目录。
4. 旧方案归档到 `AI文档/废纸篓/`，不要混在当前主线说明中。

## 版本完成

每个版本完成前至少确认：

1. `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 版本一致。
2. 发布说明存在：`AI文档/04-发布记录/发布说明_vX.Y.Z.md`。
3. 归档记录存在：`AI文档/版本归档/vX.Y.Z/`。
4. 最小验证通过：`npm run build` 与 `cargo check --manifest-path src-tauri/Cargo.toml`。
5. tag 打在 `main` 发布合并提交上，而不是临时开发分支上。

## 工作区保护

1. 开始前查看 `git status --short --branch`。
2. 不覆盖用户未提交改动。
3. 不执行 `git reset --hard`、强推、批量删除等高影响操作，除非用户明确确认。
4. 若遇到非本次任务变更，交付时说明已避开。
