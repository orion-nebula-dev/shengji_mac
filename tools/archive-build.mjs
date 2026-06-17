import { existsSync } from "node:fs";
import { cp, mkdir, readFile, rename, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __filename = fileURLToPath(import.meta.url);

export function normalizeVersionLabel(version) {
  const value = String(version ?? "").trim();
  if (!value) {
    throw new Error("version is required");
  }
  return value.startsWith("v") ? value : `v${value}`;
}

export function getLocalDateLabel(date = new Date()) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function resolveBuildArchiveDir({ rootDir, version, buildDate }) {
  return join(rootDir, "其他文件", "build", normalizeVersionLabel(version), buildDate);
}

async function resolveAvailableArchiveDir({ rootDir, version, buildDate }) {
  const baseDir = resolveBuildArchiveDir({ rootDir, version, buildDate });
  if (!(await pathExists(baseDir))) {
    return baseDir;
  }

  for (let index = 2; index < 100; index += 1) {
    const candidate = resolveBuildArchiveDir({
      rootDir,
      version,
      buildDate: `${buildDate}-${index}`,
    });
    if (!(await pathExists(candidate))) {
      return candidate;
    }
  }

  throw new Error(`too many build archives for ${normalizeVersionLabel(version)} ${buildDate}`);
}

async function readPackageVersion(rootDir) {
  const packageJson = JSON.parse(await readFile(join(rootDir, "package.json"), "utf8"));
  return packageJson.version;
}

async function pathExists(path) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

async function moveDirectory({ rootDir, archiveDir, sourceName, targetName, dryRun }) {
  const source = join(rootDir, sourceName);
  const target = join(archiveDir, targetName);

  if (!(await pathExists(source))) {
    return null;
  }
  if (await pathExists(target)) {
    throw new Error(`archive target already exists: ${target}`);
  }
  if (!dryRun) {
    await rename(source, target);
  }
  return { sourceName, targetName, action: "moved" };
}

async function copyTauriBundle({ rootDir, archiveDir, dryRun }) {
  const source = join(rootDir, "src-tauri", "target", "release", "bundle");
  const target = join(archiveDir, "tauri-bundle");

  if (!(await pathExists(source))) {
    return null;
  }
  if (await pathExists(target)) {
    throw new Error(`archive target already exists: ${target}`);
  }
  if (!dryRun) {
    await cp(source, target, { recursive: true });
  }
  return { sourceName: "src-tauri/target/release/bundle", targetName: "tauri-bundle", action: "copied" };
}

function createNotes({ versionLabel, buildDate, moved }) {
  const entries = moved.length
    ? moved.map((entry) => `- ${entry.targetName} (${entry.action} from ${entry.sourceName})`).join("\n")
    : "- no build artifacts found";

  return `# ${versionLabel} Build Notes

## 产物

${entries}

## 来源

- 归档日期：${buildDate}
- 归档命令：\`npm run build:archive\`

## 说明

该目录为本地-only build 产物归档，不上传远端。
`;
}

export async function archiveBuildArtifacts({
  rootDir = process.cwd(),
  version,
  buildDate = getLocalDateLabel(),
  includeTauriBundle = false,
  dryRun = false,
} = {}) {
  const resolvedVersion = version ?? (await readPackageVersion(rootDir));
  const versionLabel = normalizeVersionLabel(resolvedVersion);
  const archiveDir = await resolveAvailableArchiveDir({ rootDir, version: versionLabel, buildDate });

  if (!dryRun) {
    await mkdir(archiveDir, { recursive: true });
  }

  const candidates = [
    await moveDirectory({
      rootDir,
      archiveDir,
      sourceName: "dist",
      targetName: "frontend-dist",
      dryRun,
    }),
    await moveDirectory({
      rootDir,
      archiveDir,
      sourceName: "release",
      targetName: "release-package",
      dryRun,
    }),
  ];

  if (includeTauriBundle) {
    candidates.push(await copyTauriBundle({ rootDir, archiveDir, dryRun }));
  }

  const moved = candidates.filter(Boolean);
  const notesPath = join(archiveDir, "notes.md");
  if (!dryRun && !(await pathExists(notesPath))) {
    await mkdir(dirname(notesPath), { recursive: true });
    await writeFile(notesPath, createNotes({ versionLabel, buildDate, moved }));
  }

  return { archiveDir, moved, notesPath };
}

function parseArgs(argv) {
  const flags = {
    includeTauriBundle: false,
    dryRun: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--version") {
      flags.version = argv[index + 1];
      index += 1;
    } else if (arg === "--date") {
      flags.buildDate = argv[index + 1];
      index += 1;
    } else if (arg === "--include-tauri-bundle") {
      flags.includeTauriBundle = true;
    } else if (arg === "--dry-run") {
      flags.dryRun = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return flags;
}

async function main() {
  const flags = parseArgs(process.argv.slice(2));
  const result = await archiveBuildArtifacts(flags);
  const moved = result.moved.map((entry) => `${entry.action}: ${entry.sourceName} -> ${entry.targetName}`);

  console.log(`archiveDir: ${result.archiveDir}`);
  console.log(moved.length ? moved.join("\n") : "no build artifacts found");
  if (existsSync(result.notesPath)) {
    console.log(`notes: ${result.notesPath}`);
  }
}

if (pathToFileURL(process.argv[1] ?? "").href === pathToFileURL(__filename).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
