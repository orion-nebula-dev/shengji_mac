---
applyTo: "**/*"
---

# Review / Subagent Workflow Instructions

## 适用场景

本文件用于约束 code review、功能 review 和 subagent 协作。它是工作流规则，不替代具体代码规范。

## Code Review

完成一个明确 plan 或较大代码切片后，可进行 code review。重点检查：

1. 是否符合当前版本目标。
2. 是否破坏分层边界。
3. 是否重新引入旧 Qwen / llama.cpp Todo runtime。
4. DTO / TypeScript 类型 / SQLite schema 是否一致。
5. 错误处理、日志、隐私边界是否合规。
6. 是否有必要测试，验证命令是否覆盖风险。

输出应优先列问题：

```text
阻塞问题
建议修复
测试缺口
结论
```

没有问题时明确写“未发现阻塞问题”，并说明剩余风险。

## 功能 Review

每个版本完成开发后进行功能 review。重点检查：

1. 是否满足 `AI文档/03-版本迭代/` 中本版本验收标准。
2. 是否有可操作的用户路径。
3. 前端是否体现录音、转写、语义、隐私、失败状态。
4. 文档、版本号、发布说明、归档记录是否一致。
5. 最小验证是否通过。

## Subagent 使用边界

使用 subagent 时应给出明确、只读或限定范围的任务：

1. “审查这些文件是否违反分层边界。”
2. “检查前端 UI 是否存在明显状态缺口。”
3. “对照版本验收标准做功能 review。”
4. “检查是否残留 Qwen / llama.cpp Todo 路径。”

避免：

1. 让 subagent 自行决定大范围重构。
2. 让多个 subagent 同时编辑同一批文件。
3. 让 subagent 读取或输出密钥、完整用户文稿、完整本地音频路径。
4. 为很小的单文件改动强制多轮 review，造成过度流程。

## 过度 Review 判断

以下情况通常不需要 subagent review：

1. 只改错别字、链接、注释。
2. 只新增无逻辑的文档索引。
3. 已有测试覆盖且改动低风险、单文件、无行为变化。

以下情况建议 review：

1. 跨 Rust / TypeScript 契约。
2. 修改 SQLite schema 或任务状态机。
3. 修改 provider、模型调用、隐私边界。
4. 版本验收前。
5. 修复高风险 bug 或删除旧路径。
