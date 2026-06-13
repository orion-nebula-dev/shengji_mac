---
applyTo: "src-tauri/src/**/*.rs, src/lib/desktop.ts"
---

# Tauri Command 规范

## 规则

1. Tauri commands 是前端与桌面核心的唯一边界。
2. command 名称使用清晰动词，如 `load_bootstrap_data`、`save_settings`、`toggle_todo_status`。
3. 参数和返回值必须可序列化。
4. 错误返回应能被前端转成中文可见提示。
5. 新增或修改 command 时，同步更新 `src/lib/desktop.ts` 与 `src/types.ts`。
6. 涉及接口契约变化时同步 `AI文档/OpenAPI接口契约.md`。
