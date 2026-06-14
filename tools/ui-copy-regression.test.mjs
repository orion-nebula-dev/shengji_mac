import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");

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

  assert.equal(versionMatch?.[1], "v1.1.1");
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
