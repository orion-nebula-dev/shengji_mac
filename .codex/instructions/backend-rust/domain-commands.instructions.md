---
applyTo: "src-tauri/src/domain/**/*.rs, src-tauri/src/commands/**/*.rs, src-tauri/src/lib.rs"
---

# Domain / DTO / Command Instructions

## Domain 层职责

`src-tauri/src/domain/` 存放业务实体、DTO、状态枚举、provider 输入输出和错误类型。它应尽量保持纯净、稳定、可测试。

允许：

1. `serde` 序列化派生，用于 Tauri 返回 DTO。
2. 标准库类型。
3. 项目内 domain 子模块之间的类型引用。

避免：

1. 直接访问 SQLite、HTTP、文件系统、Tauri runtime。
2. 依赖 `commands/`、`app/`、`infra/`、`providers/`、`jobs/`。
3. 在 domain DTO 中隐藏复杂副作用或解析网络响应。

## DTO 规范

1. 返回前端的 DTO 使用 `#[serde(rename_all = "camelCase")]`，保持 TypeScript 字段稳定。
2. DTO 字段名应与 `src/types.ts` 对齐。
3. DTO 增删字段时同步前端类型、README 或相关 `AI文档`。
4. 对外 DTO 优先使用明确状态枚举或字符串常量，不使用含义模糊的布尔组合。
5. 敏感字段不进入 DTO，例如 API Key、完整本地路径、完整原始音频内容。

## 状态枚举

会话状态至少遵循：

```text
collecting
idle_waiting
ready_for_extraction
extracted
failed
```

本地模型运行状态至少遵循：

```text
not_ready
starting
ready
failed
```

任务状态应能表达：

```text
pending
running
success
failed
retrying
```

## Command / 参数对象

写操作或复杂操作优先使用明确的请求结构，而不是长参数列表。

建议命名：

| 用途 | 命名 |
| --- | --- |
| 写操作输入 | `*Command` |
| 查询输入 | `*Query` |
| 操作结果 | `*Result` |
| 返回前端 | `*Dto` |

要求：

1. command 层负责把前端输入转换为领域输入。
2. app/service 层不依赖前端组件状态。
3. provider 层接收 provider 专属输入，不直接接收 UI settings 全量对象。

## 错误表达

1. 可恢复错误应有阶段、错误类型和建议。
2. 用户可见错误使用中文摘要。
3. 日志或 DTO 不暴露密钥、完整用户文稿、完整本地路径。
4. provider/infra 原始错误要在边界处转换为领域可理解错误。

## 测试建议

1. DTO 字段变更时加序列化契约测试，断言 camelCase 字段。
2. 状态流转变更时加状态机或 helper 测试。
3. 解析模型返回结构时覆盖空响应、非法 JSON、字段缺失和部分成功。
