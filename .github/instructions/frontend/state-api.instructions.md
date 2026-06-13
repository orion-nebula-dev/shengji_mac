---
applyTo: "src/lib/*.ts, src/App.tsx"
---

# 状态与桌面接口规范

## 状态来源

- Tauri 环境：通过 `src/lib/desktop.ts` 调用 Rust commands。
- 浏览器原型：通过 `src/lib/storage.ts` 和 `src/data/mock.ts` 兜底。
- React state 负责页面级交互状态，如当前视图、筛选条件、选中 Todo、提示条。

## 桌面接口规则

1. 所有 Tauri command 调用集中在 `src/lib/desktop.ts`。
2. 返回结构必须能用 `src/types.ts` 表达。
3. 失败时返回清晰中文提示，不能让 UI 静默失败。
4. 浏览器原型模式下必须保留明确兜底文案。
5. 敏感字段只展示脱敏值，如 `sk-****`。

## 状态规则

- 会话状态至少覆盖：`collecting`、`idle_waiting`、`ready_for_extraction`、`extracted`、`failed`。
- 本地模型状态至少覆盖：`not_ready`、`starting`、`ready`、`failed`。
- Todo 状态当前为：`pending`、`completed`。
- 运行状态、失败原因、回退路径属于排障能力，不能随 UI 优化删除。
