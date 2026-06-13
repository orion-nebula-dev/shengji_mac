---
applyTo: "src/**/*.ts, src/**/*.tsx, index.html, vite.config.ts, tsconfig*.json, package.json"
---

# Frontend Project Instructions

## 当前结构

```text
src/
├── App.tsx        # 当前主界面，后续应逐步拆分
├── main.tsx       # React 入口
├── styles.css     # 全局样式
├── types.ts       # 前端类型
├── data/mock.ts   # 浏览器原型 fallback 数据
└── lib/
    ├── desktop.ts # Tauri command 客户端
    └── storage.ts # 浏览器原型本地状态 fallback
```

## 演进方向

随着 v0.4 之后功能增长，前端应逐步拆分为：

```text
src/
├── components/
├── views/
├── hooks/
├── lib/
├── state/
├── types.ts
├── App.tsx
├── main.tsx
└── styles.css
```

不要一次性大重构。新增功能优先按业务边界拆出局部组件，再逐步瘦身 `App.tsx`。

## TypeScript 规则

1. 生产代码不使用 `any`。
2. Tauri 返回 DTO 字段使用 camelCase，并与 Rust DTO 保持一致。
3. nullable 使用 `null` 表达后端空值，不用 `undefined` 代替后端字段。
4. `import type` 用于纯类型导入。
5. 新增字段时同步 `src/types.ts`、Rust DTO 和必要文档。

## Tauri 客户端规则

1. 前端通过 `src/lib/desktop.ts` 访问 Tauri command。
2. 组件不直接散落 `invoke()` 调用。
3. 浏览器原型 fallback 可以保留，但不能掩盖桌面端真实失败。
4. 错误处理要给用户可理解的中文提示，并保留开发可定位的错误摘要。

## 状态管理

1. 当前轻量状态可继续使用 React state。
2. 跨页面、长期共享、需要持久化的状态再引入独立 state 模块。
3. 后端数据库是桌面端事实来源；localStorage 只作为浏览器原型 fallback。
4. 任务、录音、模型状态要能刷新后恢复，不只存在内存中。

## 页面拆分建议

按版本逐步形成：

1. `OverviewView`：当前状态、待办、最近会话。
2. `RecordingView`：录音、切片、任务处理。
3. `TranscriptView`：时间轴转写、音频跳转、speaker label。
4. `SemanticArtifactsView`：摘要、纪要、Todo 候选、追溯。
5. `SettingsView`：ASR、Speaker、Semantic 独立配置和隐私边界。
6. `SystemView`：运行时、模型、任务、错误诊断。

## 增加功能顺序

1. 定义或更新 `src/types.ts`。
2. 在 `src/lib/desktop.ts` 增加 Tauri command 客户端。
3. 增加 hook 或局部状态 helper。
4. 增加组件或 view。
5. 接入 `App.tsx`。
6. 更新样式和空态、加载态、错误态。
7. 运行 `npm run build`。
