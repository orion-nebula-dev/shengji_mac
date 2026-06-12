# Git 规范 v1.0

## 1. 目标

本规范用于约束本项目的 Git 使用方式，确保：

- 开发过程可追踪
- 功能变更可审计
- 版本发布可回滚
- 分支管理清晰
- 文档、代码、版本号保持一致

## 2. 分支模型

### 2.1 长期分支

#### `main`

用于正式可发布代码。

规则：

- `main` 必须始终保持可构建、可发布
- 禁止直接在 `main` 上做日常开发
- 所有功能、修复、文档更新都应先在特性分支完成，再合并到 `main`
- 每次正式发布后，应在 `main` 上打版本 tag

### 2.2 临时分支

#### `codex/*`

用于 AI 或开发者执行单个需求、单个特性、单个修复任务。

命名建议：

- `codex/qwen3-4b-v0.2.0`
- `codex/fix-fallback-copy`
- `codex/release-notes-v0.2.0`

规则：

- 一个分支只处理一类目标
- 合并到 `main` 后应删除
- 不作为长期保留分支

#### `feature/*`

可作为人工开发特性分支命名方案。

示例：

- `feature/local-qwen-runtime`
- `feature/todo-fallback-ui`

#### `fix/*`

用于缺陷修复。

示例：

- `fix/session-fallback-copy`
- `fix/runtime-timeout`

#### `docs/*`

用于纯文档任务。

示例：

- `docs/git-guideline`
- `docs/release-v0.2.0`

## 3. 禁止事项

以下操作默认禁止：

- 直接在 `main` 上开发功能
- 未确认就执行破坏性 Git 操作
- 使用 `git reset --hard` 清理用户未确认的内容
- 使用 `git push --force` 改写公共历史
- 在未合并前随意删除分支
- 提交未验证代码
- 提交本地模型权重、数据库、运行缓存等大文件或临时文件

## 4. 标准开发流程

### 4.1 开发前

1. 确认当前不在 `main`
2. 从最新 `main` 拉新分支
3. 明确本次目标范围
4. 明确版本影响和文档影响

示例：

```bash
git checkout main
git pull origin main
git checkout -b codex/feature-name
```

### 4.2 开发中

规则：

- 小步提交
- 每次提交只做一件事
- 改代码后必须做最小验证
- 功能改动必须同步更新相关文档

### 4.3 开发完成后

1. 自测通过
2. 确认工作区干净
3. 发起合版本流程
4. 合并到 `main`
5. 发布版本时打 tag
6. 删除已合并临时分支

### 4.4 合版本流程

每个版本都应在独立分支开发，完成后必须执行合版本操作。合版本不是简单 push 分支，而是把版本分支验收后合并回 `main`，并在 `main` 上形成可追踪的发布点。

推荐流程：

```bash
git checkout main
git pull origin main
git merge --no-ff codex/shengji-vX.Y.Z-主题
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin main
git push origin vX.Y.Z
```

合版本前必须确认：

1. 版本分支已完成目标功能和文档更新。
2. 版本分支已通过最小验证。
3. `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 版本号一致。
4. `AI文档/04-发布记录/发布说明_vX.Y.Z.md` 已补齐。
5. `AI文档/版本归档/vX.Y.Z/验收记录.md` 已写入验证结果。

合版本后必须确认：

1. `main` 构建仍通过。
2. tag 打在 `main` 的发布合并提交上。
3. GitHub Release 与 tag 版本一致。
4. 已合并的临时分支可以删除。

## 5. 提交规范

### 5.1 提交原则

每个 commit 应满足：

- 单一目的
- 可读
- 可回滚
- 不混入无关改动

不要这样做：

- 一个 commit 同时改功能、文档、格式化、重构
- 提交信息写成“update”“修改一下”“fix bug”

### 5.2 提交信息格式

推荐格式：

```text
<type>(<scope>): <summary>
```

示例：

- `feat(todo): integrate embedded qwen runtime`
- `fix(runtime): increase embedded subprocess timeout`
- `docs(release): add v0.2.0 release notes`
- `refactor(settings): normalize local model version handling`

### 5.3 type 建议

- `feat`：新功能
- `fix`：缺陷修复
- `docs`：文档修改
- `refactor`：重构
- `test`：测试相关
- `build`：构建或打包配置
- `chore`：杂项维护
- `perf`：性能优化

## 6. 合并规范

### 6.1 合并到 `main`

推荐使用：

```bash
git checkout main
git pull origin main
git merge --no-ff <branch>
```

规则：

- 保留分支合并痕迹，方便回溯
- 合并前确认目标分支已经自测通过
- 合并后再次确认版本号和关键文档
- 正式版本必须在 `main` 合并提交上打 tag，不能在临时开发分支上打正式 tag

### 6.2 何时可用 cherry-pick

适用场景：

- 只需要从某个分支拿一个独立提交
- 例如单独补一份发布说明文档
- 修复已发布分支上的单点问题

不适合：

- 大量跨文件功能开发
- 复杂依赖改动

## 7. Tag 规范

### 7.1 版本 tag 格式

统一采用：

```text
v<major>.<minor>.<patch>
```

示例：

- `v0.1.0`
- `v0.2.0`
- `v0.2.1`

### 7.2 tag 使用原则

- 每个正式发布版本必须打 tag
- tag 必须打在 `main` 的发布提交上
- tag 一旦发布，不随意改指向

推荐命令：

```bash
git tag -a v0.2.0 -m "Release 0.2.0"
git push origin v0.2.0
```

### 7.3 为什么 tag 很重要

tag 用于：

- 标识正式版本
- 生成 GitHub Release
- 精确回滚
- 追踪对应资产和文档

## 8. GitHub Release 规范

代码仓库不承载大模型资产本体，正式发布采用：

- Git tag 对应版本
- GitHub Release 对应发布说明
- Release 附件承载模型和运行时二进制

### 8.1 仓库中可提交

- 代码
- 文档
- 清单文件
- Prompt 模板
- manifest 配置
- 发布说明

### 8.2 仓库中禁止提交

- `.gguf`
- `llama-cli`
- `llama-completion`
- 本地数据库
- 本地录音
- `Application Support` 目录内容
- 临时校验文件
- 本地缓存

### 8.3 Release 附件建议

- `qwen3-4b-instruct-2507-q4_k_m.gguf`
- `llama-cli`
- `llama-completion`
- `checksums.sha256`
- 安装包 `.dmg` 或 `.zip`

## 9. 分支生命周期规范

### 9.1 保留哪些分支

长期保留：

- `main`

短期保留：

- `codex/*`
- `feature/*`
- `fix/*`
- `docs/*`

### 9.2 何时删除分支

满足以下条件即可删除：

- 已合并进 `main`
- 已 push 到远端
- 有对应 tag 或 merge commit 可追踪
- 不再需要继续开发

删除后不会丢历史，因为：

- 提交已进入 `main`
- 版本已有 tag

### 9.3 删除命令

本地删除：

```bash
git branch -d codex/qwen3-4b-v0.2.0
```

远端删除：

```bash
git push origin --delete codex/qwen3-4b-v0.2.0
```

## 10. 版本号管理规范

版本号必须保持一致，至少同步以下位置：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- 发布说明
- Git tag
- GitHub Release 标题与正文

规则：

- 版本变更必须统一修改
- 发版前必须核对一致性
- 不允许出现代码 `0.2.0`，Release 写 `0.2.1` 这种情况

## 11. 文档同步规范

以下情况必须同步文档：

### 11.1 功能变更

需同步：

- PRD
- 技术设计
- 接口契约
- DDL 设计
- 开发规范

### 11.2 发布动作

需同步：

- 发布说明
- Release 正文
- 资产清单
- 已知限制

### 11.3 架构或流程变化

需同步：

- Git 规范
- 发布流程
- 分支策略
- 模型分发策略

## 12. 验证规范

任何代码改动后，至少执行最小验证。

常见验证：

- `cargo test`
- `cargo check`
- `tsc -b`
- `vite build`
- 关键链路联调
- UTF-8 无 BOM 检查

提交前必须明确：

- 改了什么
- 验证了什么
- 哪些没验证
- 有什么风险

## 13. 回滚规范

### 13.1 未 push 的情况

可通过本地修正或重做提交处理。

### 13.2 已 push 但未发布

优先追加修复提交，不建议改写公共历史。

### 13.3 已打 tag / 已发布

规则：

- 不改旧 tag 指向
- 不覆盖既有发布历史
- 如有问题，发补丁版本，例如：
  - `v0.2.1`
  - `v0.2.2`

## 14. 推荐日常命令清单

### 新建功能分支

```bash
git checkout main
git pull origin main
git checkout -b codex/feature-name
```

### 查看状态

```bash
git status
git branch --show-current
git log --oneline --decorate -5
```

### 提交

```bash
git add .
git commit -m "feat(scope): summary"
```

### 合并到 main

```bash
git checkout main
git merge --no-ff codex/feature-name
```

### 推送主分支

```bash
git push origin main
```

### 打 tag

```bash
git tag -a v0.2.0 -m "Release 0.2.0"
git push origin v0.2.0
```

### 删除已合并分支

```bash
git branch -d codex/feature-name
git push origin --delete codex/feature-name
```

## 15. 本项目建议落地策略

针对你这个项目，建议采用以下固定策略：

- 主分支：`main`
- 开发分支：`codex/*`
- 合并策略：`--no-ff`
- 版本策略：语义化版本 `vX.Y.Z`
- 正式发布：`tag + GitHub Release`
- 模型资产：Release 附件，不进 Git
- 合并后：默认删除开发分支
- 文档：功能改动必须同步 `AI文档`

## 16. 最终原则

这套规范的核心只有 6 条：

1. 不在 `main` 上直接开发
2. 功能完成后合并到 `main`
3. 正式版本必须打 tag
4. 大模型资产不进 Git 仓库
5. 代码变更必须验证
6. 功能改动必须同步文档
