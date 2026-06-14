---
name: shengji-release
description: Use when changing shengji_mac Git workflow, documentation, version records, release notes, archive records, version numbers, or preparing a version completion flow.
---

# shengji Release Skill

Use this skill for Git, docs, release, archive, and version consistency work.

## Read First

1. `.codex/instructions/workflow/git-docs-release.instructions.md`
2. `AI文档/05-规范制度/Git规范.md`
3. `AI文档/03-版本迭代/声记-版本迭代目标与代码归档方案.md`

## Core Rules

1. Use `feature-*` or `hotfix-*`; do not develop directly on `main`, `dev`, or `test`.
2. Keep commits single-purpose and use `<type>(<scope>): <summary>`.
3. Do not commit local recordings, SQLite DBs, model weights, caches, keys, or generated build artifacts.
4. Version completion requires matching version numbers, release notes, archive records, and verification evidence.
5. Do not perform destructive Git operations unless the user explicitly confirms.

## Verification

At minimum:

```bash
git status --short --branch
git diff --check
```

For version completion:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```
