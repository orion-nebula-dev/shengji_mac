---
applyTo: "src/**/*.ts, src/**/*.tsx, src-tauri/src/**/*.rs, tools/**/*.mjs, package.json, src-tauri/Cargo.toml"
---

# Testing / Verification Instructions

## 基本原则

发生文件或代码改动后，必须执行最小可行验证。验证选择按改动范围决定，不能用“看起来没问题”替代。

## 常用命令

前端或类型相关：

```bash
npm run build
```

Rust 编译检查：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Rust 测试：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

工具脚本：

```bash
npm run test:tools
```

提交前格式空白检查：

```bash
git diff --check
```

查看变更范围：

```bash
git status --short
```

## 测试策略

1. DTO 字段、serde rename、前端类型契约变更：加序列化或类型契约测试。
2. provider 解析逻辑：覆盖成功、空响应、非法 JSON、字段缺失、错误状态。
3. jobs 状态机：覆盖 pending、success、failed、retry 路径。
4. SQLite schema 或查询：覆盖新增字段默认值、查询排序、空数据。
5. UI 状态：至少人工检查默认态、空态、失败态、长文本态。

## 完成前检查

交付前至少确认：

1. 变更是否只覆盖本次任务范围。
2. 是否误碰用户已有未提交改动。
3. 是否更新必要文档。
4. 是否运行了对应验证命令。
5. 是否有不能验证的内容，并明确风险。

## 失败处理

1. 验证失败先定位根因，不直接跳过。
2. 修复后重新运行失败命令。
3. 如果失败来自环境缺失，记录缺失项、影响面和用户可执行补救。
4. 不提交未验证或已知失败的代码，除非用户明确要求保留中间状态。
