---
applyTo: "README.md, AI文档/**/*.md, package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json"
---

# Project Context Instructions

## 项目定位

`shengji_mac` 是面向 macOS 的桌面端声记工作台，当前基线是智能 Todo 原型，演进方向是：

```text
录音 / 音频导入
-> 语音转写
-> 说话人分离
-> 转写修正
-> MiniMax M3 类型化语义理解
-> semantic_artifacts / model_invocations
-> Todo / 摘要 / 纪要 / 脑图 / 导出
```

## 技术栈

| 层级 | 当前技术 |
| --- | --- |
| 桌面容器 | Tauri 2 |
| 后端 | Rust 2021 |
| 前端 | React 19 + TypeScript + Vite |
| 本地存储 | SQLite |
| 音频 | cpal + hound |
| 网络调用 | reqwest blocking client |
| 语义模型 | MiniMax M3 |

## 版本路线

1. `v0.4`：架构止血与路线切换。
2. `v0.5`：转写与说话人分离评估。
3. `v0.6`：转写修正与类型化纪要。
4. `v0.7`：待办中枢。
5. `v0.8`：思维脑图。
6. `v0.9`：Moment 与深度研究。
7. `v1.0`：分享、导出与产品化。
8. `v1.1`：翻译与多语言导出。

具体交付边界以 `AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md` 为准。

## 全局边界

1. Todo 语义入口固定为 MiniMax M3，通过 `semantic_artifacts` 承载候选产物。
2. 旧 Qwen / llama.cpp Todo runtime 不再作为默认路径、兜底路径或 legacy 路径。
3. 无老用户前提下，数据库迁移可以优先保持当前版本清晰，不为旧原型路径做复杂兼容。
4. 本地 ASR / Speaker 能力按 WhisperKit / SpeakerKit / Argmax 路线推进，未正式接入前可保留 ASR 云端兜底。
5. 所有用户音频、完整转写文本、密钥、模型缓存策略都按隐私敏感内容处理。

## 文档使用

开发前按需读取：

1. 版本目标：`AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md`
2. 架构方案：`AI文档/02-技术方案/声记-版本迭代与项目架构方案.md`
3. 开发规范：`AI文档/05-规范制度/开发规范.md`
4. Git 规范：`AI文档/05-规范制度/Git规范.md`
5. 项目索引：`AI文档/README.md`

变更长期规则时，同步更新本文档或相关分片；不要只改聊天记录。
