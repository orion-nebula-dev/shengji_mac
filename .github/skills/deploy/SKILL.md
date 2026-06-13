---
name: deploy
description: "声记 Mac App 发布辅助。用于准备版本分支、检查构建、生成 tag 和推送发布分支；不自动上传密钥或执行不可逆发布。"
---

# 声记发布辅助

## 适用范围

- 准备 `vX.Y.Z` 发布检查。
- 确认版本分支命名：`codex/gpt-5.5-vX.Y.Z`。
- 运行前端与 Tauri/Rust 构建检查。
- 生成发布摘要、tag 建议和推送命令建议。

## 发布前检查

1. 当前分支不得是 `main` 或 `develop`。
2. 工作区应无未解释的改动。
3. 运行：

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

4. 检查 `AI文档/发布说明_vX.Y.Z.md` 是否存在或需要新增。
5. 确认没有完整密钥、完整会话文稿、完整音频内容进入日志或文档。

## 约束

- 不自动删除分支、不自动打 tag、不自动 push，除非用户明确要求。
- 不绕过构建失败。
- 发布说明必须使用中文。
