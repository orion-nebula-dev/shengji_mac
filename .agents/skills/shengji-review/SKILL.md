---
name: shengji-review
description: Use when reviewing shengji_mac code, checking implementation plans, running post-plan code review, version-level functional review, or deciding whether subagent review is warranted.
---

# shengji Review Skill

Use this skill for code review, functional review, or review process decisions.

## Read First

Read only the relevant files:

1. `.codex/instructions/workflow/review-subagents.instructions.md`
2. `.codex/instructions/workflow/agent-team.instructions.md`
3. `.codex/instructions/quality/testing-verification.instructions.md`
4. The relevant backend or frontend instruction file for the changed area.
5. The actual diff or files under review.

## Code Review Focus

Lead with findings. Check:

1. Version scope and acceptance criteria.
2. Layer boundaries.
3. DTO / TypeScript / SQLite consistency.
4. Provider and model-call privacy boundaries.
5. Absence of old Qwen / llama.cpp Todo runtime.
6. Test coverage and executed verification.

## Functional Review Focus

Check:

1. Whether the user path works end to end.
2. Whether visible states cover loading, empty, success, failure, and privacy.
3. Whether docs, release notes, version numbers, and archive records align.
4. Whether minimal verification passed.

## Avoid Over-review

Do not require subagent review for typo-only docs, link fixes, comments, or low-risk single-file non-behavioral changes.

## Agent Team Defaults

1. Main agent owns scope, final edits, decisions, verification summary, and user-facing delivery.
2. Subagents are report-only by default.
3. Use parallel subagents for independent read-heavy work.
4. Use serial work for shared files, schema changes, versioning, release records, and dependent decisions.
5. Keep subagent summaries short: findings, evidence, recommendation, conclusion.
6. Do not paste large logs or code excerpts back into the main conversation.
