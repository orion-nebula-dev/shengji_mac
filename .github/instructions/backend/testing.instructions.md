---
applyTo: "src-tauri/src/**/*.rs, src/**/*.ts, src/**/*.tsx"
---

# 测试与验证规范

## 最小验证

- 前端改动：`npm run build`
- Rust/Tauri 改动：`cargo check --manifest-path src-tauri/Cargo.toml`
- Rust 逻辑测试存在时：`cargo test --manifest-path src-tauri/Cargo.toml`

## 核心覆盖方向

- 录音开关。
- 参数配置保存。
- ASR 与 Todo Provider 配置。
- 会话聚合触发。
- Todo 提取与状态切换。
- 本地模型运行时状态。
- 失败原因、回退路径、日志/排障信息展示。

## 文档检查

代码变更后检查 `AI文档` 是否需要同步，确认中文可读且无乱码。
