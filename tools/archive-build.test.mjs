import assert from "node:assert/strict";
import { mkdtemp, readFile, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";

import {
  archiveBuildArtifacts,
  normalizeVersionLabel,
  resolveBuildArchiveDir,
} from "./archive-build.mjs";

test("normalizeVersionLabel adds the v prefix once", () => {
  assert.equal(normalizeVersionLabel("1.2.2"), "v1.2.2");
  assert.equal(normalizeVersionLabel("v1.2.2"), "v1.2.2");
});

test("resolveBuildArchiveDir uses 其他文件/build/vX.Y.Z/YYYY-MM-DD", () => {
  assert.equal(
    resolveBuildArchiveDir({
      rootDir: "/repo",
      version: "1.2.2",
      buildDate: "2026-06-18",
    }),
    join("/repo", "其他文件", "build", "v1.2.2", "2026-06-18"),
  );
});

test("archiveBuildArtifacts moves dist and release into the dated version archive", async () => {
  const rootDir = await mkdtemp(join(tmpdir(), "shengji-archive-build-"));
  await writeFile(join(rootDir, "package.json"), JSON.stringify({ version: "1.2.2" }));

  const fs = await import("node:fs/promises");
  await fs.mkdir(join(rootDir, "dist"), { recursive: true });
  await fs.mkdir(join(rootDir, "release"), { recursive: true });
  await writeFile(join(rootDir, "dist", "index.html"), "<html></html>");
  await writeFile(join(rootDir, "release", "app.zip"), "zip");

  const result = await archiveBuildArtifacts({
    rootDir,
    version: "1.2.2",
    buildDate: "2026-06-18",
  });

  assert.equal(result.archiveDir, join(rootDir, "其他文件", "build", "v1.2.2", "2026-06-18"));
  assert.deepEqual(result.moved.map((entry) => entry.targetName), [
    "frontend-dist",
    "release-package",
  ]);
  await stat(join(result.archiveDir, "frontend-dist", "index.html"));
  await stat(join(result.archiveDir, "release-package", "app.zip"));

  const notes = await readFile(join(result.archiveDir, "notes.md"), "utf8");
  assert.match(notes, /v1\.2\.2/);
  assert.match(notes, /frontend-dist/);
  assert.match(notes, /release-package/);
});

test("archiveBuildArtifacts uses a numbered date suffix when the archive date already exists", async () => {
  const rootDir = await mkdtemp(join(tmpdir(), "shengji-archive-build-existing-"));
  const fs = await import("node:fs/promises");

  await writeFile(join(rootDir, "package.json"), JSON.stringify({ version: "1.2.2" }));
  await fs.mkdir(join(rootDir, "dist"), { recursive: true });
  await fs.mkdir(join(rootDir, "其他文件", "build", "v1.2.2", "2026-06-18"), {
    recursive: true,
  });
  await writeFile(join(rootDir, "dist", "index.html"), "<html></html>");

  const result = await archiveBuildArtifacts({
    rootDir,
    version: "1.2.2",
    buildDate: "2026-06-18",
  });

  assert.equal(result.archiveDir, join(rootDir, "其他文件", "build", "v1.2.2", "2026-06-18-2"));
  await stat(join(result.archiveDir, "frontend-dist", "index.html"));
});
