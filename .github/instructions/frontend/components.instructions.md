---
applyTo: "src/**/*.tsx"
---

# 前端组件规范

## 当前结构

当前版本尚未拆分组件目录，主要界面集中在 `src/App.tsx`。如需拆分组件，应先围绕真实复用点拆分，而不是为抽象而抽象。

## 组件规则

1. 组件使用 function declaration。
2. Props 使用明确 interface，不使用 `any`。
3. 业务类型从 `src/types.ts` 导入。
4. 组件不直接访问 Tauri command；需要桌面能力时通过 `src/lib/desktop.ts` 或上层传入回调。
5. 交互元素使用 `<button>`、`<input>`、`<select>` 等语义元素。
6. 所有用户可见文案使用中文。

## 拆分建议

当 `App.tsx` 继续增长时，优先拆分以下稳定组件：

- `WindowTitleBar`
- `SidebarNav`
- `RuntimeStatusPanel`
- `TodoListPanel`
- `TodoDetailPanel`
- `SessionLogPanel`
- `SettingsPanel`
