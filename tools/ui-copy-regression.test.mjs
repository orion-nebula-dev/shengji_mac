import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const stylesSource = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const mockSource = readFileSync(new URL("../src/data/mock.ts", import.meta.url), "utf8");
const storageSource = readFileSync(new URL("../src/lib/storage.ts", import.meta.url), "utf8");

function extractFunctionBody(name) {
  const start = appSource.indexOf(`function ${name}`);
  assert.notEqual(start, -1, `${name} should exist`);
  const bodyStart = appSource.indexOf("{", start);
  let depth = 0;

  for (let index = bodyStart; index < appSource.length; index += 1) {
    const char = appSource[index];
    if (char === "{") {
      depth += 1;
    } else if (char === "}") {
      depth -= 1;
      if (depth === 0) {
        return appSource.slice(bodyStart + 1, index);
      }
    }
  }

  throw new Error(`${name} body should be balanced`);
}

function extractSettingsSection() {
  const start = appSource.indexOf('{activeTab === "settings"');
  assert.notEqual(start, -1, "settings tab section should exist");
  const end = appSource.indexOf("{activeTab ===", start + 1);
  return end === -1 ? appSource.slice(start) : appSource.slice(start, end);
}

test("settings save feedback distinguishes desktop persistence failure", () => {
  const saveSettingsBody = extractFunctionBody("saveSettings");

  assert.match(saveSettingsBody, /if\s*\(\s*!persisted\s*\)/);
  assert.match(saveSettingsBody, /保存失败|未保存/);
});

test("settings visible copy uses current production-ready version labels", () => {
  const settingsSection = extractSettingsSection();
  const versionMatch = appSource.match(/const appVersionLabel = "(v\d+\.\d+\.\d+)";/);

  assert.equal(versionMatch?.[1], "v1.2.0");
  assert.doesNotMatch(settingsSection, /v0\.4|v1\.0/);
  assert.match(settingsSection, /appVersionLabel/);
  assert.match(settingsSection, /生产可用/);
});

test("user-facing workflow feedback avoids stale milestone labels", () => {
  const bannerCalls = Array.from(appSource.matchAll(/setSaveBanner\(([^;]+)\);/g))
    .map((match) => match[1])
    .join("\n");

  assert.doesNotMatch(bannerCalls, /v0\.|v1\.0/);
  assert.doesNotMatch(appSource, /前不写入正式|请先生成 v0\./);
});

test("hash routes stay synchronized after the initial page load", () => {
  assert.match(appSource, /function getHashTab\(\)/);
  assert.match(appSource, /window\.history\.replaceState\(null,\s*"",\s*nextHash\)/);
  assert.match(appSource, /window\.addEventListener\("hashchange",\s*handleHashChange\)/);
  assert.match(appSource, /window\.removeEventListener\("hashchange",\s*handleHashChange\)/);
});

test("desktop chrome uses the native app frame and exposes pause recording controls", () => {
  assert.equal(appSource.includes('className="window-frame"'), false);
  assert.equal(/traffic-lights|traffic-dot|traffic-close|traffic-minimize|traffic-maximize/.test(appSource), false);
  assert.equal(/\.traffic-|\.window-frame/.test(stylesSource), false);
  assert.match(appSource, /暂停录音/);
  assert.match(appSource, /runtime\.currentSessionStatus === "collecting"/);
  assert.match(appSource, /setSettings\(payload\.settings\)/);
  assert.doesNotMatch(appSource, /runtimeLabel\.includes/);
  assert.match(stylesSource, /\.chip-recording/);
  assert.match(mockSource, /currentSessionStatus:\s*"collecting"/);
  assert.match(storageSource, /normalizedRuntime\.runtimeLabel === "录音中"/);
  assert.match(storageSource, /normalizedRuntime\.currentSessionStatus = "collecting"/);
});

test("v1.2 experience polish surfaces recovery, timeline, metrics, diff, compare, and candidate editing", () => {
  assert.match(appSource, /错误恢复面板/);
  assert.match(appSource, /任务状态时间线/);
  assert.match(appSource, /性能指标/);
  assert.match(appSource, /修正 diff|Diff/);
  assert.match(appSource, /脑图版本对比/);
  assert.match(appSource, /候选编辑/);
  assert.match(appSource, /updateDesktopTodoCandidate/);
  assert.match(appSource, /loadDesktopRuntimeDashboard/);
  assert.match(appSource, /loadDesktopSegmentTimeline/);
});

test("v1.2 design tokens define SwiftUI-ready semantic aliases before component rules", () => {
  const rootStart = stylesSource.indexOf(":root");
  const firstComponentStart = stylesSource.indexOf(".app-shell");
  const rootBlock = stylesSource.slice(rootStart, firstComponentStart);

  assert.notEqual(rootStart, -1, "styles.css should define :root tokens");
  assert.match(rootBlock, /--color-bg-window/);
  assert.match(rootBlock, /--color-surface-panel/);
  assert.match(rootBlock, /--color-accent/);
  assert.match(rootBlock, /--font-size-13/);
  assert.match(rootBlock, /--space-1/);
  assert.match(rootBlock, /--radius-panel/);
  assert.match(stylesSource, /var\(--color-bg-window\)/);
});
