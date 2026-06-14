---
name: shengji-architecture
description: Use when changing shengji_mac Rust/Tauri backend architecture, domain DTOs, commands, providers, SQLite infra, jobs, model calls, or removing legacy Qwen/llama.cpp Todo paths.
---

# shengji Architecture Skill

Use this skill before changing `src-tauri/src/**/*.rs`, provider configuration, SQLite schema, jobs, domain DTOs, or model invocation code.

## Read First

Read only the files relevant to the task:

1. `.codex/instructions/backend-rust/architecture.instructions.md`
2. `.codex/instructions/backend-rust/domain-commands.instructions.md`
3. `.codex/instructions/backend-rust/providers-infra-jobs.instructions.md`
4. `AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md`
5. `AI文档/05-规范制度/开发规范.md`

## Core Rules

1. Keep `commands/` thin.
2. Move business orchestration to `app/`.
3. Keep pure domain types in `domain/`.
4. Put SQLite, HTTP, file, process, and key storage in `infra/`.
5. Put replaceable ASR / Speaker / Semantic providers in `providers/`.
6. Put background task state machines and retries in `jobs/`.
7. Do not reintroduce old Qwen / llama.cpp Todo runtime.

## Verification

Choose the smallest useful verification:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```
