# .codex

本目录是项目级 Codex profile / 配套规范目录。

## Codex 是否会自动使用这里？

结论：只放 `.codex/instructions/*.instructions.md` 不会被 Codex 自动当作项目指令加载。

当前 Codex 自动发现项目规范的主入口是仓库根目录的 `AGENTS.md`。因此本项目同时提供：

1. `AGENTS.md`：Codex 默认自动加载的项目规则。
2. `.agents/skills/*/SKILL.md`：Codex 可发现的 repo-scoped skills。
3. `.codex/instructions/`：项目规范分片，供 `AGENTS.md` 和 skills 按需引用。
4. `.codex/AGENTS.md`：当使用 `CODEX_HOME=$(pwd)/.codex` 启动 Codex 时的 profile 级入口。

## 推荐使用

正常在仓库根目录启动 Codex：

```bash
codex
```

Codex 会自动加载根目录 `AGENTS.md`。

如需隔离项目专属 Codex home：

```bash
CODEX_HOME=$(pwd)/.codex codex
```

此时 Codex 会读取 `.codex/AGENTS.md` 作为 home 级规则。

## 目录说明

```text
.codex/
├── AGENTS.md       # CODEX_HOME 指向本目录时的全局入口
├── README.md       # 本说明
└── instructions/   # 细分项目规范，不是 Codex 自动加载入口
```

repo-scoped skills 放在仓库根目录：

```text
.agents/skills/
├── shengji-architecture/
├── shengji-frontend/
├── shengji-review/
└── shengji-release/
```
