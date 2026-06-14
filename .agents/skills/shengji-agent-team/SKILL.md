---
name: shengji-agent-team
description: Use when shengji_mac work benefits from multiple agents, parallel review, context isolation, long-log analysis, release validation, or avoiding main-thread context pollution.
---

# shengji Agent Team Skill

Use this skill before coordinating multiple agents or deciding whether to delegate work.

## Read First

1. `.codex/instructions/workflow/agent-team.instructions.md`
2. `.codex/instructions/workflow/review-subagents.instructions.md`
3. The relevant backend, frontend, quality, or release instruction for the task.

## Default Pattern

1. Main agent defines scope and assigns bounded tasks.
2. Subagents default to read-only report mode.
3. Parallelize independent read-heavy work.
4. Serialize edits, schema changes, version changes, and release records.
5. Main agent merges findings and decides.
6. Main conversation gets concise findings, not raw exploration.

## Good Delegations

1. Architecture review of Rust/Tauri boundaries.
2. Frontend UX state review.
3. Privacy/security scan for secrets, logs, audio, transcripts.
4. Test and verification gap analysis.
5. Release/docs consistency check.

## Output Limit

Each subagent should return no more than 20 lines unless it finds a blocker.
