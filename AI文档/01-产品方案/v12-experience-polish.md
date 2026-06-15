# v1.2 — 体验打磨与稳定性收口

## 版本定位

在启动 SwiftUI 迁移（v1.6）之前的**最后一公里 Tauri 体验收口版本**。不再堆新业务能力，而是把 v0.4~v1.1 已堆出的 9 步业务流（录音 → 转写 → 修正 → 摘要/纪要 → Todo → 脑图 → Moment/Research → 翻译/导出）中的 UX 摩擦点、错误恢复路径、可观测性补齐；同时把 Tauri 阶段特有的"React 端样式资产"沉淀成可被 SwiftUI 复用的设计 Token 雏形。

承接 v1.1.1（多语言导出已上线），不引入新业务 schema。

## 平台状态

| 维度 | 状态 |
| --- | --- |
| 运行时 | Tauri 2.10 + React 19 + Vite |
| 后端 | Rust（commands/app/domain/infra/providers/jobs） |
| 数据 | SQLite（14+ 表，schema 稳定） |
| AI | local_whisperkit (占位) + volc 兜底 ASR / local_speakerkit (占位) / **minimax_m3 (MiniMax-M3) 语义硬编码** |
| 目标 | UX 资产化 + 错误恢复 + 埋点可观测 |

## 一级/二级功能评估

### ✅ 保留并强化

| 一级 | 二级 | 强化点 | 原因 |
| --- | --- | --- | --- |
| 录音 | 切片转写 | 失败任务一键重试 UI 完整化 | 当前 retry_transcript_job 有 3 次重试保护，但 UI 上看不到 retry_count / error_message |
| 转写 | 说话人管理 | speaker label 持久化 + 全局别名表 | rename_speaker 已实现，缺跨会话别名复用 |
| 修正 | M3 修正对照 | 修正前/后 diff 高亮 + 一键回退 | transcript_segments.review_status 支持 corrected 但 UI 未完整展示 |
| 产物 | 摘要/纪要 | 长文摘要分页 + 关键片段锚点跳转 | 已生成 artifacts，但无导航 |
| Todo | 候选管理 | 候选合并/拆分/优先级编辑 | 当前只支持 accept/dismiss，缺中间态编辑 |
| 脑图 | 编辑 | 编辑版与生成版并排对比 | insert_edited_mind_map 写入新 artifact，但 UI 无对比视图 |
| 导出 | 多语言 | 导出包含原文 + 译文双版本 | 已有 _<lang> 后缀，但导出 Markdown 模板未排版优化 |

### ⚠️ 需调整定位

| 功能 | 原定位 | 建议调整 | 原因 |
| --- | --- | --- | --- |
| **本地 WhisperKit 转写** | 主力 ASR | **明确为「实验位 + fallback 兜底」** | 当前实际是 `local_whisperkit::PROVIDER_ID` 占位，转写主力仍走 volc 兜底；继续标"主力"会误导用户 |
| **本地 SpeakerKit 说话人分离** | 主力 | **同上调为「实验位」** | 同上原因，代码中实际未实装模型调用 |
| **设置页"成本展示"** | 显示 provider 调用成本 | **改为显示"调用次数 + 模型名"** | 真实成本依赖 provider 内部计费，本地无法精确统计；改为可观测指标 |

### ❌ 建议弱化/移除

| 功能 | 原因 |
| --- | --- |
| 录音"占位会话"逻辑 | is_placeholder_session_text 是 v0.4 临时调试用，v1.1.1 已可走真实路径，移除避免污染 session 列表 |
| 模型管理页"下载进度"动画 | local_whisperkit 仍是占位，下载进度永远 0% 会让用户困惑 |
| 旧 styles.css 中 v0.4 时代的固定 padding/颜色硬编码 | 沉淀进 design tokens，避免 v1.6 SwiftUI 端再写一遍 |

### 🆕 需新增

| 功能 | 归属 | 服务于核心流程 | 验收标准 |
| --- | --- | --- | --- |
| **错误恢复面板** | 全局 | 流程 1-9 任一步失败 | 列出 transcript_jobs.status=failed / processing_jobs.status=failed 任务，一键重试 + 显示 error_message + 最近一次重试时间 |
| **任务状态时间线** | 录音 → 转写 → 修正 → 产物 | 流程 1-5 | 单个 audio_segment 的全生命周期：started_at → 转写 succeeded → M3 succeeded → 用户操作时间戳 |
| **可观测性埋点** | 全局 | 流程 1-9 | 关键 Tauri 命令耗时（import_local_audio、generate_semantic_workbench、generate_mind_map、generate_export_bundle）写入 `app_settings` 或新表 `runtime_metrics` |
| **Design Token 雏形** | 前端 | 流程 2（转写编辑器） | 提取 `styles.css` 中所有 hex 颜色 / 字号 / 间距为 CSS 变量；与 `设计参考/00-design-tokens.md` 双实现映射 |
| **设置页"隐私边界"显式化** | 设置 | 流程 1（录音） | 明确写出"本地 ASR / 本地说话人 / 云端语义"三段边界，匹配 `边界_constraints[0]` 已有的 schema |
| **导入音频元数据面板** | 录音 | 流程 1 | 显示 file_path（脱敏）/ sample_rate / channels / duration / has_effective_voice |

## 核心场景适配度评分

| 核心流程步骤 | v1.1.1 覆盖度 | v1.2 目标 | 关键举措 | 平台 |
| --- | --- | --- | --- | --- |
| 1. 录音+说话人识别 | 75% | 90% | 真实录音链路验证 + 说话人别名表 | Tauri |
| 2. 切片转写 | 85% | 95% | 失败重试 UI + 时间码跳转精度 | Tauri |
| 3. 说话人管理 | 70% | 90% | 跨会话别名 + 声纹标识 | Tauri |
| 4. M3 智能修正 | 80% | 90% | 修正前后 diff + 回退 | Tauri |
| 5. 结构化产物 | 90% | 95% | 摘要分页 + 锚点跳转 | Tauri |
| 6. 思维脑图 | 85% | 95% | 编辑/生成并排 | Tauri |
| 7. 行动中心 + 导出 | 90% | 95% | 双语言排版 + 来源追溯面板 | Tauri |

## 详细功能规划

### 增强类

#### E1. 失败任务重试 UI
- **用户故事**：作为会议记录者，我希望在转写失败时看到具体错误信息和一键重试入口，而不是看到空转写稿
- **验收标准**：
  - 错误恢复面板按 `audio_segment_id` 聚合
  - 单个任务支持重试（触发 `retry_transcript_job` 或 `update_processing_job` 状态机）
  - 重试时禁用其他重试按钮（防止竞态）
  - `transcript_jobs.error_message` 完整显示，包含 provider 原始错误
- **文件**：`src/views/RecoveryView.tsx`（新）+ `src/components/TaskTimeline.tsx`（新）
- **关键 Rust**：`transcript_service::retry_transcript_job` 已有，复用

#### E2. 转写修正前后 diff 视图
- **用户故事**：作为用户，我想清楚看到 M3 改了哪些字、回退时不丢原文
- **验收标准**：
  - 原文（transcript_segments.text）+ 修正文稿（transcript_revision artifact payload）并排显示
  - 高亮"修改"段（改动级别=change / add / remove）
  - 单条"回退"按钮，把该 segment 的 `review_status` 改回 `normal`
- **文件**：`src/components/RevisionDiff.tsx`（新）
- **关键 Rust**：`transcript_service::mark_transcript_segment` 复用

#### E3. 脑图编辑/生成并排对比
- **用户故事**：作为用户，我重写脑图节点后能看到原版与我的版本差异
- **验收标准**：
  - 同 `conversation_session_id` 下，所有 `semantic_artifacts(type='mind_map')` 按 version 排序
  - 选中"原版" vs "我的版本"切换显示
  - 不能静默覆盖（已由 `next_mind_map_artifact_id` 保护，需 UI 端体现）
- **文件**：`src/components/MindMapCompare.tsx`（新）

#### E4. 多语言导出排版优化
- **用户故事**：作为用户，我希望导出的 Markdown 译文可读性强，不只是机械拼接
- **验收标准**：
  - 译文与原文用 `>` blockquote 区分
  - 关键术语表（自动从 `transcript_correction_patterns` 提取）在文末追加
  - 导出包文件名包含 `<lang>` 后缀（已实现）
- **文件**：`src-tauri/src/app/export_service.rs::render_multilingual_markdown` 改写

### 新增类

#### N1. 任务状态时间线
- **用户故事**：作为用户，我想看到一个录音的"全生命周期"——什么时候开始录、什么时候转写完、什么时候 M3 处理完
- **验收标准**：
  - 单页 `src/views/SegmentTimelineView.tsx`
  - 数据源：`audio_segments` + `transcript_jobs` + `processing_jobs` + `model_invocations` JOIN
  - 时间线按 `created_at` / `finished_at` 排序
- **关键 Tauri 命令**：新 `get_segment_timeline(audio_segment_id)` command

#### N2. 可观测性埋点
- **用户故事**：作为用户/支持者，我想知道某个转写为什么慢
- **验收标准**：
  - 关键 Tauri 命令进入/退出时打点
  - 写入新表 `runtime_metrics(audio_segment_id, command_name, started_at, duration_ms, status)`
  - 设置页新增"性能" tab，显示近 7 天 P50/P95 耗时
- **关键 Rust**：新表 + 新 `record_metric` command
- **数据迁移考虑**：v2.0 SwiftData 端需保留此表

#### N3. Design Token 雏形（前端 CSS 变量）
- **用户故事**：作为前端开发者，我希望所有颜色/字号/间距都通过 CSS 变量引用，而不是硬编码
- **验收标准**：
  - `src/styles.css` 顶部新增 `:root { --color-bg-window: ...; --color-accent: ...; --font-size-13: ...; --space-1: 4px; ... }`
  - 全文件替换硬编码值
  - 同步产出 `设计参考/00-design-tokens.md`（队列 B 第 1 项）
- **Tauri/SwiftUI 双实现映射**：CSS 变量值必须与 SwiftUI `Color/ColorScheme` 一一对应

### 架构/基础类

#### A1. 设置页隐私边界文案
- 把 `boundary_constraints[0]` 中已有的语义 schema 翻译成用户可读的隐私声明
- 涉及：本地 ASR / 本地说话人 / 云端语义（minimax_m3）三段
- 文件：`src/views/SettingsView.tsx::PrivacySection`（扩充）

#### A2. 移除占位会话逻辑
- 删除 `is_placeholder_session_text` 全部调用点
- 影响：`src-tauri/src/lib.rs:124-129` + `src-tauri/src/jobs/todo_extraction.rs:88-100`
- 风险：若有用户正在用占位 demo 流程，需先发预告

#### A3. Provider 模型名外置
- 当前 `semantic_service` 四处 INSERT 硬编码 `minimax_m3::DEFAULT_MODEL_NAME`
- v1.2 把模型名挪到 `app_settings.semantic_model_name`，默认值仍为 `MiniMax-M3`
- 涉及：`semantic_service.rs::upsert_artifact/upsert_artifact_with_schema/insert_artifact_with_id/upsert_artifact_with_id`
- 风险：v1.1.1 用户期望"硬编码保证一致性"，需保留默认值不能改

## 平台迁移事项

v1.2 **不启动迁移**，但必须做以下迁移准备工作：

1. **Design Token 雏形**（N3）是 v1.6 SwiftUI 端 design system 的基础
2. **可观测性埋点表 `runtime_metrics`**（N2）的 schema 必须按 SwiftData 可表达的形式设计
3. **Provider 模型名外置**（A3）让 SwiftUI 端 settings 页能直接读写
4. **错误恢复面板**（E1）的 UI 结构是 macOS 系统级 "错误恢复" 模式（HIG 有规范）的预演

## 依赖与风险

### 前置依赖
- v1.1.1 已实现 9 步业务流
- `app_settings` 表已存在，可扩展

### 外部依赖
- MiniMax-M3 调用不变
- 火山引擎 ASR 兜底不变

### 风险
| 风险 | 等级 | 缓解 |
| --- | --- | --- |
| 移除占位会话逻辑破坏用户数据 | 中 | 灰度开关 `keep_placeholder_sessions` 默认 true，v1.3 再关 |
| Design Token 重构引入 CSS 回归 | 中 | 保留 `styles.css.backup`，按页面分批替换 |
| 可观测性埋点拖慢 Tauri 命令 | 低 | 异步写入，不阻塞主流程 |
| Provider 模型名外置破坏历史 artifact 兼容 | 低 | 读取时 fallback 到硬编码 |

## 与未来版本的衔接

- **v1.3**：错误恢复面板的"批量重试"扩展 + 任务时间线接入 Todo 候选流
- **v1.4**：Design Token 完整化（与队列 B 同步推进）
- **v1.5**：Tauri 阶段收官，本版本的可观测性埋点 + design token 是迁移前置条件
- **v1.6**：SwiftUI 实验壳直接消费 v1.4/v1.5 的 design tokens，跳过 React 端 CSS 变量层

## UI/UX 关联改动

- 引入 design tokens（CSS 变量 + SwiftUI 端 Color/Font）
- 错误恢复面板走 macOS HIG 错误恢复模式（`recoveryView` 后续可直接在 SwiftUI 端以 `NSAlert` 风格落地）
- 任务状态时间线为后续 SwiftUI `NavigationSplitView` 三栏布局埋点

## ⚠️ 定位偏差预警

**v1.1.1 的实际功能范围已超出 `AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md` 中描述的 v0.4~v1.1 路线图**：
- v0.4 路线图定位的「embedding provider」仅预留接口，但 v1.1.1 已实装 deep_research
- v0.6 路线图定位的「meeting_minutes」是手工摘要，但 v1.1.1 实际由 M3 自动生成
- v1.1 路线图定位的"翻译与多语言导出"在 v1.1.1 已实现，但同时引入了 convert_research_to_todo（属 v0.9 范围）

**建议动作**：
- v1.2 启动后，同步产出 `声记-版本迭代目标与代码归档方案.md` 的 v1.1.1 勘误（把"已实现"与"路线图描述"对齐）
- v1.3 起所有规划以 **v1.1.1 实际能力** 为基线，不再以路线图描述为基线

## 更新日志

- 2026-06-15：v1.2 规划定稿（首版）
