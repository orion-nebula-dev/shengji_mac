**Findings**
- No actionable P0/P1/P2 findings remain.

**2026-06-14 Follow-up: Workbench Sizing And Scroll Contract**
- User concern addressed: the previous mac shell duplicated brand/title/version, kept "运行摘要" inside the left sidebar, and made 今日工作台 sub-tabs behave like cross-page shortcuts instead of complete secondary views.
- Updated titlebar contract: sidebar owns the app logo/name; centered titlebar owns the current location only, e.g. `今日工作台 / 转写`; version moved to the bottom status bar and settings.
- Updated scroll contract: `window-frame` is fixed, `window-body` is the split shell, sidebar navigation gets its own overflow only when needed, `.content-area` is the primary page scroll container, and `.window-statusbar` is a fixed bottom summary bar.
- Updated 今日工作台 sub-pages:
  - `转写`: waveform, transcript list, selected segment detail, revision comparison.
  - `摘要`: summary, decisions, risks, open questions, artifact status, latest model call.
  - `Todo`: candidates, accept/dismiss controls, current Todo detail, execution summary.
  - `隐私边界`: local/cloud/export boundaries, semantic key/model status, local model status.
- New web preview favicon uses the packaged app icon via `public/favicon.png`, avoiding the browser default `/favicon.ico` 404 during QA.

**2026-06-14 Follow-up Screenshots**
- Viewport: `1200x760`
- Screenshots:
  - `/tmp/shengji-issue-1/overview-panel-transcript-1200.png`
  - `/tmp/shengji-issue-1/overview-panel-summary-1200.png`
  - `/tmp/shengji-issue-1/overview-panel-todo-1200.png`
  - `/tmp/shengji-issue-1/overview-panel-privacy-1200.png`

**2026-06-14 Follow-up Verification**
- `npm run build`: passed.
- `npm run test:tools`: passed, 9/9 tests.
- `git diff --check`: passed.
- Chrome DevTools at `1200x760`: all 10 primary routes rendered expected title/content, status bar stayed visible at the window bottom, and horizontal overflow was `0`.
- Chrome DevTools at `1200x760`: all 4 今日工作台 sub-pages rendered both `.overview-main-panel` and `.overview-detail-panel` with non-empty content and horizontal overflow `0`.
- Chrome DevTools console after reload: no application errors, no resource 404, no form-field accessibility issue.
- Click QA for changed surfaces: 126/130 visible enabled buttons clicked in an isolated context with no errors; the 4 missed entries were statusbar buttons whose indexes shifted during mutation-heavy clicks, then all 4 were clicked directly by label and routed correctly to `#semantic`, `#system`, `#actions`, and `#settings`.

**Source Visual Truth**
- Issue reference screenshots:
  - `/tmp/shengji-issue-1/ref-1.png`
  - `/tmp/shengji-issue-1/ref-2.png`
  - `/tmp/shengji-issue-1/ref-3.png`
- Confirmed brief: macOS desktop productivity app with a left brand sidebar, top toolbar, multi-column workbench, and production-ready settings flow.
- Note: the reference images describe the reported production mismatch and old-looking settings prompt; they are not a pixel-perfect target to preserve.

**Implementation Screenshots**
- Viewport: `1440x1000`
- State: browser prototype mode with default local mock data and hash-routed tabs.
- Screenshots:
  - `/tmp/shengji-issue-1/overview-redesign.png`
  - `/tmp/shengji-issue-1/actions-redesign.png`
  - `/tmp/shengji-issue-1/transcript-redesign.png`
  - `/tmp/shengji-issue-1/semantic-redesign.png`
  - `/tmp/shengji-issue-1/research-redesign.png`
  - `/tmp/shengji-issue-1/mindmap-redesign.png`
  - `/tmp/shengji-issue-1/export-redesign.png`
  - `/tmp/shengji-issue-1/history-redesign.png`
  - `/tmp/shengji-issue-1/system-redesign.png`
  - `/tmp/shengji-issue-1/settings-redesign.png`
- Minimum desktop viewport checks, matching the Tauri minimum window size `1200x760`:
  - `/tmp/shengji-issue-1/size-1200-overview.png`
  - `/tmp/shengji-issue-1/size-1200-actions.png`
  - `/tmp/shengji-issue-1/size-1200-settings.png`
  - `/tmp/shengji-issue-1/size-1200-research-v3.png`
  - `/tmp/shengji-issue-1/size-1200-mindmap-v2.png`
  - `/tmp/shengji-issue-1/size-1200-export.png`
- Shell scroll and titlebar pass at `1200x760`:
  - `/tmp/shengji-issue-1/shell-scroll-research-1200.png`
  - `/tmp/shengji-issue-1/shell-scroll-settings-1200.png`
  - `/tmp/shengji-issue-1/shell-scroll-overview-1200.png`
- Final per-page screenshots after route/click QA at `1200x760`:
  - `/tmp/shengji-issue-1/final-pages-1200/overview.png`
  - `/tmp/shengji-issue-1/final-pages-1200/actions.png`
  - `/tmp/shengji-issue-1/final-pages-1200/transcript.png`
  - `/tmp/shengji-issue-1/final-pages-1200/semantic.png`
  - `/tmp/shengji-issue-1/final-pages-1200/research.png`
  - `/tmp/shengji-issue-1/final-pages-1200/mindmap.png`
  - `/tmp/shengji-issue-1/final-pages-1200/export.png`
  - `/tmp/shengji-issue-1/final-pages-1200/history.png`
  - `/tmp/shengji-issue-1/final-pages-1200/system.png`
  - `/tmp/shengji-issue-1/final-pages-1200/settings.png`

**Full-View Comparison Evidence**
- Compared the issue settings screenshots against the new settings implementation screenshot.
- The settings page now uses the current app version label, production-ready copy, real app icon asset, unified sidebar, and a failure-specific save banner path instead of reporting success when persistence is unavailable.
- Compared all rendered tab screenshots for shared shell consistency: sidebar, titlebar, page headers, surfaces, input controls, dense lists, canvas, export preview, and status chips use the same visual system.
- Compared minimum desktop captures against Tauri window constraints (`1440x920` default, `1200x760` minimum). The overview, action center, settings, research, mindmap, and export pages avoid horizontal layout pressure at the minimum supported size.

**Focused Region Comparison Evidence**
- Settings header and save action: current version label is visible, stale `v0.4` and `v1.0` user-facing copy are absent, and the save path has failure-specific copy covered by regression tests.
- Sidebar brand: CSS-drawn placeholder mark was replaced with the real app icon asset from `src-tauri/icons/128x128.png`.
- Workbench density: overview, actions, research, and export pages were checked for readable multi-column layout at `1440x1000`.
- Mac shell contract: the body/window frame are fixed to the app viewport, the left sidebar has its own nav scroll only when the minimum height cannot fit all entries, the right content area is the primary page scroll container, and only dense local widgets such as textareas/export previews retain nested scrolling.
- Titlebar contract: the center title now shows app icon, app name, active page name, and `v1.1.1`; the right toolbar keeps the main commands in a compact Apple-style titlebar group.
- Route/click contract: hash routes now stay synchronized after initial load, so direct `/#settings`, sidebar clicks, titlebar actions, and browser hash changes all render the matching page content.

**Required Fidelity Surfaces**
- Fonts and typography: retained the app's PingFang/system stack, normalized heading weights, avoided negative letter spacing, and kept dense panel text at compact desktop sizes.
- Spacing and layout rhythm: pages now share the same titlebar/sidebar/page-header/panel rhythm; dense pages use predictable 2-3 column grids without visible overlap in captured states.
- Viewport resilience: at `1200x760`, overview keeps the primary transcript readable, research preserves a three-column workbench with tightened columns, mindmap preserves canvas plus editor, settings becomes a stacked form flow, and export actions stack vertically to preserve button and input widths.
- Colors and visual tokens: unified light macOS glass surfaces, current blue action color, green success, yellow pending, and red failure tokens across pages.
- Image quality and asset fidelity: visible brand image uses the packaged app icon, not CSS art or placeholder imagery.
- Copy and content: removed stale milestone labels from user-facing save banners and settings/workflow copy; settings now communicates production-ready behavior.

**Patches Made Since Previous QA Pass**
- Rebuilt the global desktop shell, sidebar brand area, overview record workbench, action center header, all page panel/list surfaces, settings cards, and hash-routed tab entry points.
- Added `tools/ui-copy-regression.test.mjs` coverage for settings save failure copy, current visible version labels, and stale milestone label regressions.
- Added `src/vite-env.d.ts` so Vite image assets type-check.

**Verification**
- `npm run build`: passed.
- `npm run test:tools`: passed.
- `git diff --check`: passed.
- Chrome headless screenshots captured for all 10 tabs at `1440x1000`.
- Chrome headless screenshots captured for representative minimum-size pages at `1200x760`; research was adjusted twice after review because the first single-column fallback was not information-complete.
- Chrome headless screenshots captured for the explicit shell scroll/titlebar pass at `1200x760`.
- Chrome/CDP route QA at `1200x760`: all 10 routes rendered the expected active page title and main content with 0 horizontal overflow and 0 console errors.
- Chrome/CDP isolated click QA at `1200x760`: 234 visible enabled buttons clicked from clean reloads across all 10 routes, 0 failures. Report: `/tmp/shengji-issue-1/button-click-qa-1200.json`.
- Regression test added for hash route synchronization because the first route QA pass found that hash changes after initial load stayed on the previous React tab.

**Open Questions**
- None blocking. The next decision is visual acceptance from the product owner before committing and opening a PR.

**Implementation Checklist**
- Keep dev server available for review at `http://127.0.0.1:4173/`.
- Review each tab via `/#overview`, `/#actions`, `/#transcript`, `/#semantic`, `/#research`, `/#mindmap`, `/#export`, `/#history`, `/#system`, and `/#settings`.
- After approval, commit, push, and open the PR for issue #1.

**Follow-up Polish**
- P3: add iconography to toolbar buttons once the project selects a standard icon library.

final result: passed
