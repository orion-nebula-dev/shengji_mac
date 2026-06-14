---
name: shengji-frontend
description: Use when changing shengji_mac React/TypeScript UI, Tauri invoke client, app state, CSS, macOS-style visual design, loading/error/empty states, or settings views.
---

# shengji Frontend Skill

Use this skill before changing `src/**/*.ts`, `src/**/*.tsx`, `src/**/*.css`, `src/lib/desktop.ts`, or user-visible UI.

## Read First

Read only the files relevant to the task:

1. `.codex/instructions/frontend/project.instructions.md`
2. `.codex/instructions/frontend/visual-style.instructions.md`
3. `src/types.ts`
4. `src/lib/desktop.ts`
5. Existing components or views touched by the task.

## Core Rules

1. First screen is the usable workbench, not a marketing page.
2. Components should not scatter raw Tauri `invoke()` calls; use `src/lib/desktop.ts`.
3. Rust DTO camelCase fields must match `src/types.ts`.
4. UI text defaults to Chinese.
5. Show clear recording, processing, success, failure, and privacy states.
6. Avoid decorative overuse: no card nesting, no large one-note gradients, no layout jumps.

## Verification

```bash
npm run build
git diff --check
```

For visible UI changes, also inspect the running UI when feasible.
