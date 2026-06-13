---
applyTo: "src-tauri/src/**/*.rs, src/types.ts"
---

# 数据结构映射规范

## 规则

1. Rust 返回结构、Tauri 序列化字段、TypeScript 类型必须一致。
2. 时间字段统一使用字符串传给前端。
3. 密钥只返回脱敏值。
4. 错误原因返回摘要，避免泄露堆栈、完整路径或完整用户文稿。
5. 新增字段时同步 mock 数据和 UI 展示。
