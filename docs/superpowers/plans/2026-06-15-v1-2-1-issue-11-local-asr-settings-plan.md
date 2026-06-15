# v1.2.1 Issue #11 Local ASR Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver v1.2.1 fixes for issue #11: settings page cleanup, real local ASR CLI runtime detection and model download management, recording segment entry, hidden audio import UI, and release/version updates.

**Architecture:** Keep Tauri commands thin and move local ASR behavior into focused Rust app/infra modules. Persist app-owned ASR runtime/model state in SQLite, delegate actual Argmax/WhisperKit work to installed CLI tools, and expose minimal DTOs to the existing React app through `src/lib/desktop.ts`.

**Tech Stack:** Tauri 2, Rust, rusqlite, React, TypeScript, SQLite, Argmax OSS `argmax-cli`, Homebrew `whisperkit-cli`, Vite.

---

## Source Inputs

- Approved design: `docs/superpowers/specs/2026-06-15-v1-2-1-issue-11-local-asr-settings-design.md`
- Issue evidence: `https://github.com/orion-nebula-dev/shengji_mac/issues/11`
- Argmax OSS README: `https://github.com/argmaxinc/argmax-oss-swift`
- Homebrew formula: `https://formulae.brew.sh/formula/whisperkit-cli`
- Local project rules: `AGENTS.md`, `.codex/instructions/backend-rust/*.md`, `.codex/instructions/frontend/*.md`, `.codex/instructions/workflow/*.md`

## File Structure

### Rust backend

- Modify `src-tauri/src/domain/mod.rs`
  - Register the new `local_asr` domain module.
- Create `src-tauri/src/domain/local_asr.rs`
  - Define runtime, model catalog, model status, download request, and local ASR state DTOs.
- Modify `src-tauri/src/domain/transcript.rs`
  - Replace the current narrow `LocalModelStatusDto` use with the richer local ASR status DTO while keeping JSON camelCase.
- Modify `src-tauri/src/infra/mod.rs`
  - Register the new `local_asr_runtime` infra module.
- Create `src-tauri/src/infra/local_asr_runtime.rs`
  - Probe `argmax-cli` and `whisperkit-cli`, execute version/help/download/transcribe commands, and keep command-runner seams testable.
- Modify `src-tauri/src/infra/sqlite.rs`
  - Add `local_asr_runtime_status`, adjust `local_model_status` defaults, and add migration for old rows that incorrectly mark unverified models available.
- Modify `src-tauri/src/app/mod.rs`
  - Register the new `local_asr_service` app module.
- Create `src-tauri/src/app/local_asr_service.rs`
  - Coordinate runtime probe, model catalog, status persistence, CLI-delegated download, local transcribe preflight, and user-facing error strings.
- Modify `src-tauri/src/app/transcript_service.rs`
  - Use the new local ASR status query, rename user-facing transcript text to "录音片段", keep import service available for dev/test command, and wire retry preflight.
- Modify `src-tauri/src/commands/mod.rs`
  - Register the new `local_asr` commands module.
- Create `src-tauri/src/commands/local_asr.rs`
  - Expose Tauri commands for querying state, probing runtime, selecting model, downloading model, and local transcribe retry.
- Modify `src-tauri/src/commands/transcript.rs`
  - Keep `import_local_audio` command exported for dev/test, and expose clearer `get_recording_segments_payload` aliases if the UI needs them.
- Modify `src-tauri/src/commands/model_test.rs`
  - Route ASR test connection to local ASR runtime/model preflight instead of the old cloud/browser message.
- Modify `src-tauri/src/lib.rs`
  - Register new commands in `tauri::generate_handler!` and add behavior tests following the existing `#[cfg(test)] mod tests` style.

### React frontend

- Modify `src/types.ts`
  - Add local ASR runtime/model/recording segment DTOs and remove user-visible `cloud_volc` from ASR selection typing.
- Modify `src/lib/desktop.ts`
  - Add invoke wrappers for local ASR commands and keep `importDesktopLocalAudio` as a dev-only exported helper.
- Modify `src/lib/storage.ts`
  - Persist selected local ASR model in browser prototype mode for UI development only.
- Modify `src/data/mock.ts`
  - Rename "转写评估" strings to "录音片段" and provide mock local ASR runtime/model states.
- Modify `src/App.tsx`
  - Remove visible audio import controls, rename nav/page to "录音片段", implement minimal segments/speakers view, rebuild settings page around local ASR, and show a single fixed bottom-right version.
- Modify `src/styles.css`
  - Update settings layout, local ASR status styling, recording segment layout, and fixed version placement.

### Release/docs

- Modify `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`
  - Set application version to `1.2.1`.
- Create `AI文档/04-发布记录/发布说明_v1.2.1.md`
  - Record scope, known limitations, and verification results.
- Create `AI文档/版本归档/v1.2.1/归档清单.md`
  - Record changed modules, commands, and artifacts.
- Create `AI文档/版本归档/v1.2.1/验收记录.md`
  - Record build/test/UI/subagent review results.

---

## Plan 0: Baseline And Bug Verification

### Task 0.1: Verify issue #11 problems exist before implementation

**Files:**
- Read: `src/App.tsx`
- Read: `src/types.ts`
- Read: `src-tauri/src/infra/sqlite.rs`
- Read: `package.json`
- Read: `src-tauri/Cargo.toml`
- Read: `src-tauri/tauri.conf.json`
- Output evidence in final implementation notes.

- [ ] **Step 1: Run targeted source scan**

```bash
rg -n "火山云端 ASR|导入音频|当前浏览器原型模式不支持云模型连接测试|转写评估|version = \"1\\.1\\.1\"|\"version\": \"1\\.1\\.1\"" src src-tauri package.json
```

Expected: matches include `src/App.tsx` cloud ASR option, visible import UI, old "转写评估" nav text, browser-only test message, and `1.1.1` versions.

- [ ] **Step 2: Run focused SQLite default scan**

```bash
rg -n "local_model_status|download_status TEXT NOT NULL DEFAULT 'available'|download_progress INTEGER NOT NULL DEFAULT 100|offline_available INTEGER NOT NULL DEFAULT 1" src-tauri/src/infra/sqlite.rs
```

Expected: current schema still defaults unverified local model status to available/100/offline.

- [ ] **Step 3: Save bug verification notes**

Record this concise result in the implementation final response:

```text
Bug verified: current UI still exposes cloud ASR and audio import, settings test uses browser/cloud wording, local model status defaults to available without runtime verification, and version files are still 1.1.1.
```

## Plan 1: Backend Local ASR Runtime And Model State

### Task 1.1: Add failing backend tests for local ASR defaults and runtime probing

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/infra/local_asr_runtime.rs`
- Create: `src-tauri/src/domain/local_asr.rs`

- [ ] **Step 1: Add local ASR domain shell needed by failing tests**

Create `src-tauri/src/domain/local_asr.rs` with this initial content:

```rust
use serde::{Deserialize, Serialize};

pub(crate) const LOCAL_ASR_PROVIDER: &str = "local_whisperkit";
pub(crate) const DEFAULT_LOCAL_ASR_MODEL: &str = "large-v3-v20240930_626MB";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalAsrRuntimeDto {
    pub(crate) runtime_id: String,
    pub(crate) display_name: String,
    pub(crate) available: bool,
    pub(crate) path: String,
    pub(crate) version: String,
    pub(crate) error_message: String,
}
```

Register it in `src-tauri/src/domain/mod.rs`:

```rust
pub(crate) mod local_asr;
```

- [ ] **Step 2: Add runtime probe test seams**

Create `src-tauri/src/infra/local_asr_runtime.rs` with this initial content:

```rust
use crate::domain::local_asr::LocalAsrRuntimeDto;

pub(crate) trait LocalAsrCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String>;
}

pub(crate) fn probe_local_asr_runtimes<R: LocalAsrCommandRunner>(
    runner: &R,
) -> Vec<LocalAsrRuntimeDto> {
    ["argmax-cli", "whisperkit-cli"]
        .into_iter()
        .map(|program| match runner.run(program, &["--version"]) {
            Ok(version) => LocalAsrRuntimeDto {
                runtime_id: program.to_string(),
                display_name: program.to_string(),
                available: true,
                path: program.to_string(),
                version,
                error_message: String::new(),
            },
            Err(error_message) => LocalAsrRuntimeDto {
                runtime_id: program.to_string(),
                display_name: program.to_string(),
                available: false,
                path: String::new(),
                version: String::new(),
                error_message,
            },
        })
        .collect()
}
```

Register it in `src-tauri/src/infra/mod.rs`:

```rust
pub(crate) mod local_asr_runtime;
```

- [ ] **Step 3: Add failing behavior tests in `src-tauri/src/lib.rs`**

Add these tests inside the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn should_default_local_model_status_to_not_started_until_verified() {
    let temp_dir = std::env::temp_dir().join(format!(
        "smart-todo-local-asr-default-test-{}",
        current_timestamp_label()
    ));
    fs::create_dir_all(&temp_dir).expect("应能创建临时测试目录");
    let db_path = temp_dir.join("smart-todo.sqlite");

    initialize_database(&db_path).expect("应能初始化数据库");
    let review = commands::transcript::get_transcript_review_payload(&db_path)
        .expect("应能读取录音片段状态");

    assert_eq!(review.model_status.provider, "local_whisperkit");
    assert_eq!(review.model_status.model_name, "large-v3-v20240930_626MB");
    assert_eq!(review.model_status.download_status, "not_started");
    assert_eq!(review.model_status.download_progress, 0);
    assert!(!review.model_status.offline_available);
}

#[test]
fn should_report_argmax_and_whisperkit_missing_when_runner_cannot_execute() {
    struct MissingRunner;

    impl infra::local_asr_runtime::LocalAsrCommandRunner for MissingRunner {
        fn run(&self, program: &str, _args: &[&str]) -> Result<String, String> {
            Err(format!("{program} 未安装或不在 PATH 中"))
        }
    }

    let runtimes = infra::local_asr_runtime::probe_local_asr_runtimes(&MissingRunner);

    assert_eq!(runtimes.len(), 2);
    assert_eq!(runtimes[0].runtime_id, "argmax-cli");
    assert_eq!(runtimes[1].runtime_id, "whisperkit-cli");
    assert!(runtimes.iter().all(|runtime| !runtime.available));
    assert!(runtimes.iter().all(|runtime| runtime.error_message.contains("未安装")));
}
```

- [ ] **Step 4: Run tests and confirm the default-status test fails**

```bash
cargo test --manifest-path src-tauri/Cargo.toml should_default_local_model_status_to_not_started_until_verified -- --nocapture
```

Expected: FAIL because current SQLite default is `available`, progress `100`, and offline available `true`.

### Task 1.2: Implement local ASR DTOs, SQLite schema, and runtime persistence

**Files:**
- Modify: `src-tauri/src/domain/local_asr.rs`
- Modify: `src-tauri/src/domain/transcript.rs`
- Modify: `src-tauri/src/infra/sqlite.rs`
- Modify: `src-tauri/src/app/transcript_service.rs`

- [ ] **Step 1: Expand `src-tauri/src/domain/local_asr.rs`**

Replace the shell with:

```rust
use serde::{Deserialize, Serialize};

pub(crate) const LOCAL_ASR_PROVIDER: &str = "local_whisperkit";
pub(crate) const DEFAULT_LOCAL_ASR_MODEL: &str = "large-v3-v20240930_626MB";
pub(crate) const LOCAL_ASR_CACHE_DIR: &str = "~/Library/Application Support/com.soundworkbench.shengji/models/whisperkit";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalAsrRuntimeDto {
    pub(crate) runtime_id: String,
    pub(crate) display_name: String,
    pub(crate) available: bool,
    pub(crate) path: String,
    pub(crate) version: String,
    pub(crate) error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalAsrModelDto {
    pub(crate) model_name: String,
    pub(crate) label: String,
    pub(crate) size_hint: String,
    pub(crate) quality_hint: String,
    pub(crate) recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalAsrModelStatusDto {
    pub(crate) provider: String,
    pub(crate) model_name: String,
    pub(crate) cache_dir: String,
    pub(crate) download_status: String,
    pub(crate) download_progress: i64,
    pub(crate) offline_available: bool,
    pub(crate) device_recommendation: String,
    pub(crate) error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalAsrStateDto {
    pub(crate) runtimes: Vec<LocalAsrRuntimeDto>,
    pub(crate) models: Vec<LocalAsrModelDto>,
    pub(crate) selected_model: String,
    pub(crate) model_status: LocalAsrModelStatusDto,
}
```

- [ ] **Step 2: Update transcript DTO import**

In `src-tauri/src/domain/transcript.rs`, replace the local `LocalModelStatusDto` struct with:

```rust
pub(crate) type LocalModelStatusDto = crate::domain::local_asr::LocalAsrModelStatusDto;
```

- [ ] **Step 3: Fix SQLite local model defaults**

In `src-tauri/src/infra/sqlite.rs`, change `local_model_status` defaults to:

```sql
download_status TEXT NOT NULL DEFAULT 'not_started'
  CHECK (download_status IN ('not_started', 'downloading', 'available', 'failed')),
download_progress INTEGER NOT NULL DEFAULT 0 CHECK (download_progress >= 0 AND download_progress <= 100),
offline_available INTEGER NOT NULL DEFAULT 0 CHECK (offline_available IN (0, 1)),
error_message TEXT NOT NULL DEFAULT '',
```

Add this table near `local_model_status`:

```sql
CREATE TABLE IF NOT EXISTS local_asr_runtime_status (
  runtime_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  available INTEGER NOT NULL DEFAULT 0 CHECK (available IN (0, 1)),
  path TEXT NOT NULL DEFAULT '',
  version TEXT NOT NULL DEFAULT '',
  error_message TEXT NOT NULL DEFAULT '',
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

Add migration logic after table creation:

```rust
connection
    .execute(
        r#"
        UPDATE local_model_status
        SET model_name = 'large-v3-v20240930_626MB',
            download_status = CASE
                WHEN download_status = 'available' AND offline_available = 1 THEN 'not_started'
                ELSE download_status
            END,
            download_progress = CASE
                WHEN download_status = 'available' AND offline_available = 1 THEN 0
                ELSE download_progress
            END,
            offline_available = CASE
                WHEN download_status = 'available' AND offline_available = 1 THEN 0
                ELSE offline_available
            END,
            updated_at = CURRENT_TIMESTAMP
        WHERE provider = 'local_whisperkit'
        "#,
        [],
    )
    .map_err(|error| format!("迁移本地 ASR 模型状态失败: {error}"))?;
```

- [ ] **Step 4: Update `query_local_model_status` mapping**

In `src-tauri/src/app/transcript_service.rs`, make the empty-state DTO return:

```rust
model_name: crate::domain::local_asr::DEFAULT_LOCAL_ASR_MODEL.into(),
offline_available: false,
```

Update the SQL query to include `error_message`, and map missing rows to:

```rust
Ok(LocalModelStatusDto {
    provider: crate::domain::local_asr::LOCAL_ASR_PROVIDER.into(),
    model_name: crate::domain::local_asr::DEFAULT_LOCAL_ASR_MODEL.into(),
    cache_dir: crate::domain::local_asr::LOCAL_ASR_CACHE_DIR.into(),
    download_status: "not_started".into(),
    download_progress: 0,
    offline_available: false,
    device_recommendation: "默认使用 large-v3-v20240930_626MB；设备或下载失败时可切换 base/tiny。".into(),
    error_message: String::new(),
})
```

- [ ] **Step 5: Run the focused tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml should_default_local_model_status_to_not_started_until_verified should_report_argmax_and_whisperkit_missing_when_runner_cannot_execute -- --nocapture
```

Expected: both tests PASS.

### Task 1.3: Implement CLI probing and command execution

**Files:**
- Modify: `src-tauri/src/infra/local_asr_runtime.rs`

- [ ] **Step 1: Implement real command runner**

Add:

```rust
use std::process::Command;

pub(crate) struct SystemLocalAsrCommandRunner;

impl LocalAsrCommandRunner for SystemLocalAsrCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|error| format!("{program} 未安装或无法执行: {error}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if output.status.success() {
            Ok(if stdout.is_empty() { stderr } else { stdout })
        } else {
            Err(if stderr.is_empty() {
                format!("{program} 执行失败，退出码 {:?}", output.status.code())
            } else {
                stderr
            })
        }
    }
}
```

- [ ] **Step 2: Add runtime display names**

Update the probe mapping so IDs remain stable:

```rust
fn runtime_display_name(runtime_id: &str) -> &'static str {
    match runtime_id {
        "argmax-cli" => "Argmax CLI",
        "whisperkit-cli" => "WhisperKit CLI",
        _ => runtime_id,
    }
}
```

- [ ] **Step 3: Add helper command builders**

Add these functions:

```rust
pub(crate) fn argmax_model_path(cache_dir: &str, model_name: &str) -> String {
    format!("{cache_dir}/whisperkit-coreml/openai_whisper-{model_name}")
}

pub(crate) fn build_argmax_transcribe_args(model_path: &str, audio_path: &str) -> Vec<String> {
    vec![
        "transcribe".into(),
        "--model-path".into(),
        model_path.into(),
        "--audio-path".into(),
        audio_path.into(),
    ]
}

pub(crate) fn build_argmax_serve_args(model_name: &str) -> Vec<String> {
    vec!["serve".into(), "--model".into(), model_name.into()]
}
```

- [ ] **Step 4: Run infra tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml local_asr_runtime -- --nocapture
```

Expected: PASS for mock-runner tests; no real CLI required.

## Plan 2: Backend Commands For Download, Probe, And Transcribe Preflight

### Task 2.1: Add local ASR app service

**Files:**
- Create: `src-tauri/src/app/local_asr_service.rs`
- Modify: `src-tauri/src/app/mod.rs`
- Modify: `src-tauri/src/infra/sqlite.rs`

- [ ] **Step 1: Register service module**

In `src-tauri/src/app/mod.rs` add:

```rust
pub(crate) mod local_asr_service;
```

- [ ] **Step 2: Implement model catalog**

Create `src-tauri/src/app/local_asr_service.rs` with:

```rust
use rusqlite::{params, Connection};

use crate::{
    domain::local_asr::{
        LocalAsrModelDto, LocalAsrModelStatusDto, LocalAsrRuntimeDto, LocalAsrStateDto,
        DEFAULT_LOCAL_ASR_MODEL, LOCAL_ASR_CACHE_DIR, LOCAL_ASR_PROVIDER,
    },
    infra::local_asr_runtime::{probe_local_asr_runtimes, SystemLocalAsrCommandRunner},
};

pub(crate) fn local_asr_model_catalog() -> Vec<LocalAsrModelDto> {
    vec![
        LocalAsrModelDto {
            model_name: "large-v3-v20240930_626MB".into(),
            label: "large-v3-v20240930_626MB".into(),
            size_hint: "626MB".into(),
            quality_hint: "默认，高质量多语言".into(),
            recommended: true,
        },
        LocalAsrModelDto {
            model_name: "base".into(),
            label: "base".into(),
            size_hint: "小模型".into(),
            quality_hint: "速度优先，适合性能不足时切换".into(),
            recommended: false,
        },
        LocalAsrModelDto {
            model_name: "tiny".into(),
            label: "tiny".into(),
            size_hint: "最小模型".into(),
            quality_hint: "调试优先，准确率最低".into(),
            recommended: false,
        },
    ]
}
```

- [ ] **Step 3: Implement state query**

Add:

```rust
pub(crate) fn get_local_asr_state(connection: &Connection) -> Result<LocalAsrStateDto, String> {
    let runtimes = refresh_local_asr_runtimes(connection)?;
    let model_status = query_local_asr_model_status(connection)?;

    Ok(LocalAsrStateDto {
        runtimes,
        models: local_asr_model_catalog(),
        selected_model: model_status.model_name.clone(),
        model_status,
    })
}
```

- [ ] **Step 4: Persist runtime probe results**

Add:

```rust
pub(crate) fn refresh_local_asr_runtimes(
    connection: &Connection,
) -> Result<Vec<LocalAsrRuntimeDto>, String> {
    let runner = SystemLocalAsrCommandRunner;
    let runtimes = probe_local_asr_runtimes(&runner);

    for runtime in &runtimes {
        connection
            .execute(
                r#"
                INSERT INTO local_asr_runtime_status (
                  runtime_id, display_name, available, path, version, error_message, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)
                ON CONFLICT(runtime_id) DO UPDATE SET
                  display_name = excluded.display_name,
                  available = excluded.available,
                  path = excluded.path,
                  version = excluded.version,
                  error_message = excluded.error_message,
                  updated_at = CURRENT_TIMESTAMP
                "#,
                params![
                    runtime.runtime_id,
                    runtime.display_name,
                    runtime.available as i64,
                    runtime.path,
                    runtime.version,
                    runtime.error_message
                ],
            )
            .map_err(|error| format!("保存本地 ASR runtime 状态失败: {error}"))?;
    }

    Ok(runtimes)
}
```

- [ ] **Step 5: Implement selected model update**

Add:

```rust
pub(crate) fn select_local_asr_model(
    connection: &Connection,
    model_name: &str,
) -> Result<LocalAsrModelStatusDto, String> {
    let known = local_asr_model_catalog()
        .into_iter()
        .any(|model| model.model_name == model_name);
    if !known {
        return Err("不支持的本地 ASR 模型名称".into());
    }

    connection
        .execute(
            r#"
            INSERT INTO local_model_status (
              provider, model_name, cache_dir, download_status, download_progress,
              offline_available, device_recommendation, error_message, updated_at
            ) VALUES (?1, ?2, ?3, 'not_started', 0, 0, ?4, '', CURRENT_TIMESTAMP)
            ON CONFLICT(provider) DO UPDATE SET
              model_name = excluded.model_name,
              download_status = 'not_started',
              download_progress = 0,
              offline_available = 0,
              error_message = '',
              updated_at = CURRENT_TIMESTAMP
            "#,
            params![
                LOCAL_ASR_PROVIDER,
                model_name,
                LOCAL_ASR_CACHE_DIR,
                "默认使用 large-v3-v20240930_626MB；失败时可切换 base/tiny。"
            ],
        )
        .map_err(|error| format!("更新本地 ASR 模型失败: {error}"))?;

    query_local_asr_model_status(connection)
}
```

- [ ] **Step 6: Run service tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml local_asr -- --nocapture
```

Expected: PASS for catalog, selection, and runtime persistence tests.

### Task 2.2: Add Tauri commands and model test routing

**Files:**
- Create: `src-tauri/src/commands/local_asr.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/commands/model_test.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create command module**

Create `src-tauri/src/commands/local_asr.rs`:

```rust
use std::path::PathBuf;

use crate::{
    app::local_asr_service,
    domain::local_asr::{LocalAsrModelStatusDto, LocalAsrStateDto},
    infra::sqlite::open_connection,
    AppState,
};

pub(crate) fn get_local_asr_state_payload(db_path: &PathBuf) -> Result<LocalAsrStateDto, String> {
    let connection = open_connection(db_path)?;
    local_asr_service::get_local_asr_state(&connection)
}

pub(crate) fn refresh_local_asr_runtimes_payload(
    db_path: &PathBuf,
) -> Result<LocalAsrStateDto, String> {
    let connection = open_connection(db_path)?;
    local_asr_service::refresh_local_asr_runtimes(&connection)?;
    local_asr_service::get_local_asr_state(&connection)
}

pub(crate) fn select_local_asr_model_payload(
    db_path: &PathBuf,
    model_name: &str,
) -> Result<LocalAsrModelStatusDto, String> {
    let connection = open_connection(db_path)?;
    local_asr_service::select_local_asr_model(&connection, model_name)
}

#[tauri::command]
pub(crate) fn get_local_asr_state(
    state: tauri::State<'_, AppState>,
) -> Result<LocalAsrStateDto, String> {
    get_local_asr_state_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn refresh_local_asr_runtimes(
    state: tauri::State<'_, AppState>,
) -> Result<LocalAsrStateDto, String> {
    refresh_local_asr_runtimes_payload(&state.db_path)
}

#[tauri::command]
pub(crate) fn select_local_asr_model(
    model_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<LocalAsrModelStatusDto, String> {
    select_local_asr_model_payload(&state.db_path, &model_name)
}
```

- [ ] **Step 2: Register module**

In `src-tauri/src/commands/mod.rs` add:

```rust
pub(crate) mod local_asr;
```

- [ ] **Step 3: Register Tauri handlers**

In `src-tauri/src/lib.rs`, add to `tauri::generate_handler!`:

```rust
commands::local_asr::get_local_asr_state,
commands::local_asr::refresh_local_asr_runtimes,
commands::local_asr::select_local_asr_model,
```

- [ ] **Step 4: Route ASR test connection to local ASR preflight**

In `src-tauri/src/commands/model_test.rs`, keep the existing command name and replace ASR behavior with a call to local ASR preflight. The result must use this shape:

```rust
ModelTestResult {
    ok,
    provider: "local_whisperkit".into(),
    message,
    latency_ms,
    trace_id,
}
```

The message rules are:

```text
runtime missing: 未检测到 argmax-cli 或 whisperkit-cli，请先安装本地 ASR CLI。
model missing: 已检测到本地 ASR CLI，但模型尚未下载。
ready: 本地 ASR runtime 与模型均可用。
```

- [ ] **Step 5: Run Rust checks**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

### Task 2.3: Implement delegated model download and local transcribe retry

**Files:**
- Modify: `src-tauri/src/app/local_asr_service.rs`
- Modify: `src-tauri/src/commands/local_asr.rs`
- Modify: `src-tauri/src/app/transcript_service.rs`
- Modify: `src-tauri/src/commands/transcript.rs`

- [ ] **Step 1: Add download command service**

Implement `download_local_asr_model(connection, model_name)` so it:

```text
1. validates the model is in the catalog;
2. sets local_model_status to downloading/progress 10;
3. prefers argmax-cli if available;
4. invokes CLI download using official Argmax flow when an Argmax source tree is configured, otherwise invokes whisperkit-cli help/model initialization path;
5. verifies the model folder exists under cache_dir or CLI reports the model is cached;
6. sets available/100/offline true on success;
7. sets failed/0/offline false and a concise error_message on failure.
```

Use this Rust function signature:

```rust
pub(crate) fn download_local_asr_model(
    connection: &Connection,
    model_name: &str,
) -> Result<LocalAsrModelStatusDto, String>
```

- [ ] **Step 2: Add command wrapper**

In `src-tauri/src/commands/local_asr.rs` add:

```rust
pub(crate) fn download_local_asr_model_payload(
    db_path: &PathBuf,
    model_name: &str,
) -> Result<LocalAsrModelStatusDto, String> {
    let connection = open_connection(db_path)?;
    local_asr_service::download_local_asr_model(&connection, model_name)
}

#[tauri::command]
pub(crate) fn download_local_asr_model(
    model_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<LocalAsrModelStatusDto, String> {
    download_local_asr_model_payload(&state.db_path, &model_name)
}
```

Register it in `src-tauri/src/lib.rs`:

```rust
commands::local_asr::download_local_asr_model,
```

- [ ] **Step 3: Add local transcribe preflight**

In `src-tauri/src/app/local_asr_service.rs` add:

```rust
pub(crate) fn ensure_local_asr_ready(connection: &Connection) -> Result<LocalAsrModelStatusDto, String> {
    let state = get_local_asr_state(connection)?;
    let has_runtime = state.runtimes.iter().any(|runtime| runtime.available);
    if !has_runtime {
        return Err("未检测到 argmax-cli 或 whisperkit-cli，请先安装本地 ASR CLI。".into());
    }
    if !state.model_status.offline_available || state.model_status.download_status != "available" {
        return Err("本地 ASR 模型尚未下载，请先在设置页下载模型。".into());
    }
    Ok(state.model_status)
}
```

- [ ] **Step 4: Wire retry to preflight**

In `src-tauri/src/app/transcript_service.rs`, update `retry_transcript_job` before changing status:

```rust
crate::app::local_asr_service::ensure_local_asr_ready(connection)?;
```

Expected behavior: retry fails with a clear missing runtime/model message instead of pretending local ASR is available.

- [ ] **Step 5: Run focused retry tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml retry_transcript_job local_asr -- --nocapture
```

Expected: PASS after existing retry tests are updated to seed an available model where needed.

## Plan 3: Frontend Types, Client, And Browser Prototype State

### Task 3.1: Add frontend DTOs and invoke wrappers

**Files:**
- Modify: `src/types.ts`
- Modify: `src/lib/desktop.ts`
- Modify: `src/lib/storage.ts`
- Modify: `src/data/mock.ts`

- [ ] **Step 1: Update ASR provider type**

In `src/types.ts`, replace:

```ts
export type AsrProviderType = "cloud_volc" | "local_whisperkit";
```

with:

```ts
export type AsrProviderType = "local_whisperkit";
```

- [ ] **Step 2: Add local ASR DTOs**

Add to `src/types.ts`:

```ts
export type LocalAsrDownloadStatus = "not_started" | "downloading" | "available" | "failed";

export interface LocalAsrRuntime {
  runtimeId: string;
  displayName: string;
  available: boolean;
  path: string;
  version: string;
  errorMessage: string;
}

export interface LocalAsrModel {
  modelName: string;
  label: string;
  sizeHint: string;
  qualityHint: string;
  recommended: boolean;
}

export interface LocalAsrModelStatus {
  provider: "local_whisperkit";
  modelName: string;
  cacheDir: string;
  downloadStatus: LocalAsrDownloadStatus;
  downloadProgress: number;
  offlineAvailable: boolean;
  deviceRecommendation: string;
  errorMessage: string;
}

export interface LocalAsrState {
  runtimes: LocalAsrRuntime[];
  models: LocalAsrModel[];
  selectedModel: string;
  modelStatus: LocalAsrModelStatus;
}
```

- [ ] **Step 3: Add desktop invoke wrappers**

In `src/lib/desktop.ts` add:

```ts
export async function getDesktopLocalAsrState(): Promise<LocalAsrState> {
  return invoke<LocalAsrState>("get_local_asr_state");
}

export async function refreshDesktopLocalAsrRuntimes(): Promise<LocalAsrState> {
  return invoke<LocalAsrState>("refresh_local_asr_runtimes");
}

export async function selectDesktopLocalAsrModel(modelName: string): Promise<LocalAsrModelStatus> {
  return invoke<LocalAsrModelStatus>("select_local_asr_model", { modelName });
}

export async function downloadDesktopLocalAsrModel(modelName: string): Promise<LocalAsrModelStatus> {
  return invoke<LocalAsrModelStatus>("download_local_asr_model", { modelName });
}
```

- [ ] **Step 4: Add mock state**

In `src/data/mock.ts` add:

```ts
export const mockLocalAsrState: LocalAsrState = {
  runtimes: [
    {
      runtimeId: "argmax-cli",
      displayName: "Argmax CLI",
      available: false,
      path: "",
      version: "",
      errorMessage: "未检测到 argmax-cli",
    },
    {
      runtimeId: "whisperkit-cli",
      displayName: "WhisperKit CLI",
      available: false,
      path: "",
      version: "",
      errorMessage: "未检测到 whisperkit-cli",
    },
  ],
  models: [
    {
      modelName: "large-v3-v20240930_626MB",
      label: "large-v3-v20240930_626MB",
      sizeHint: "626MB",
      qualityHint: "默认，高质量多语言",
      recommended: true,
    },
    {
      modelName: "base",
      label: "base",
      sizeHint: "小模型",
      qualityHint: "速度优先",
      recommended: false,
    },
    {
      modelName: "tiny",
      label: "tiny",
      sizeHint: "最小模型",
      qualityHint: "调试优先",
      recommended: false,
    },
  ],
  selectedModel: "large-v3-v20240930_626MB",
  modelStatus: {
    provider: "local_whisperkit",
    modelName: "large-v3-v20240930_626MB",
    cacheDir: "~/Library/Application Support/com.soundworkbench.shengji/models/whisperkit",
    downloadStatus: "not_started",
    downloadProgress: 0,
    offlineAvailable: false,
    deviceRecommendation: "默认使用 large-v3-v20240930_626MB；失败时可切换 base/tiny。",
    errorMessage: "",
  },
};
```

- [ ] **Step 5: Build frontend**

```bash
npm run build
```

Expected: PASS TypeScript build.

## Plan 4: Settings Page And Version Display

### Task 4.1: Rebuild settings page around local ASR

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Remove user-visible cloud ASR option**

In `src/App.tsx`, remove:

```tsx
<option value="cloud_volc">火山云端 ASR</option>
```

Keep only local ASR display:

```tsx
<option value="local_whisperkit">本地 WhisperKit / Argmax</option>
```

- [ ] **Step 2: Replace ASR test message**

Replace browser prototype ASR test fallback:

```ts
setSaveBanner("当前浏览器原型模式不支持云模型连接测试。");
```

with:

```ts
setSaveBanner("当前浏览器原型模式仅展示本地 ASR 状态；桌面端会探测 argmax-cli 与 whisperkit-cli。");
```

- [ ] **Step 3: Add local ASR settings card**

Add state:

```ts
const [localAsrState, setLocalAsrState] = useState<LocalAsrState>(mockLocalAsrState);
const [localAsrBusy, setLocalAsrBusy] = useState(false);
```

Add handlers:

```ts
const handleRefreshLocalAsr = async () => {
  setLocalAsrBusy(true);
  try {
    const nextState = isDesktopRuntime
      ? await refreshDesktopLocalAsrRuntimes()
      : mockLocalAsrState;
    setLocalAsrState(nextState);
    setSaveBanner("本地 ASR runtime 探测完成。");
  } catch (error) {
    setSaveBanner(error instanceof Error ? error.message : "本地 ASR runtime 探测失败。");
  } finally {
    setLocalAsrBusy(false);
  }
};

const handleDownloadLocalAsrModel = async () => {
  setLocalAsrBusy(true);
  try {
    const status = isDesktopRuntime
      ? await downloadDesktopLocalAsrModel(localAsrState.selectedModel)
      : mockLocalAsrState.modelStatus;
    setLocalAsrState((current) => ({ ...current, modelStatus: status }));
    setSaveBanner("本地 ASR 模型状态已更新。");
  } catch (error) {
    setSaveBanner(error instanceof Error ? error.message : "本地 ASR 模型下载失败。");
  } finally {
    setLocalAsrBusy(false);
  }
};
```

- [ ] **Step 4: Remove duplicate version displays**

Keep a single global version element near the app shell root:

```tsx
<div className="app-version-corner">v1.2.1</div>
```

Remove other app-version visible elements that describe the application version. Do not remove semantic artifact version labels such as mind map `v3`.

- [ ] **Step 5: Add CSS for fixed bottom-right version**

In `src/styles.css` add:

```css
.app-version-corner {
  position: fixed;
  right: 14px;
  bottom: 10px;
  z-index: 30;
  color: rgba(15, 23, 42, 0.56);
  font-size: 12px;
  line-height: 1;
  pointer-events: none;
}
```

- [ ] **Step 6: Run UI text scans**

```bash
rg -n "火山云端 ASR|当前浏览器原型模式不支持云模型连接测试|<div className=\"app-version|v1\\.1\\.1" src/App.tsx src/styles.css package.json src-tauri
```

Expected: no matches for removed cloud/browser/version strings except valid `v1.2.1` after version task.

## Plan 5: Recording Segment Entry And Hidden Import UI

### Task 5.1: Rename transcript entry and remove visible import UI

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/data/mock.ts`
- Modify: `src/styles.css`

- [ ] **Step 1: Rename nav item**

In `src/App.tsx`, replace:

```ts
{ key: "transcript", label: "转写评估", description: "音频与说话人" },
```

with:

```ts
{ key: "transcript", label: "录音片段", description: "转写与说话人" },
```

- [ ] **Step 2: Rename page heading**

Replace:

```tsx
<h2>转写评估与说话人</h2>
```

with:

```tsx
<h2>录音片段</h2>
```

Use this subtitle:

```tsx
<p>转写与说话人</p>
```

- [ ] **Step 3: Remove visible audio import controls**

Remove the user-facing controls that render:

```tsx
本地音频路径
导入音频
```

Keep `importDesktopLocalAudio` exported in `src/lib/desktop.ts` and keep the Tauri `import_local_audio` command for dev/test.

- [ ] **Step 4: Add minimal recording segment empty state**

When no audio exists, render:

```tsx
<div className="recording-empty-state">
  <h3>暂无录音片段</h3>
  <p>完成录音后，片段会出现在这里，并显示转写状态和说话人信息。</p>
</div>
```

- [ ] **Step 5: Add segment list and speaker list labels**

Use the existing `transcriptReview.audio`, `transcriptReview.segments`, `transcriptReview.speakers`, and `transcriptReview.jobs` data. Render labels:

```tsx
片段列表
片段详情
说话人
重试转写
```

- [ ] **Step 6: Run text scan**

```bash
rg -n "导入音频|本地音频路径|转写评估" src/App.tsx src/data/mock.ts src/styles.css
```

Expected: no visible UI strings remain. Remaining backend/dev-test strings are allowed in `src-tauri`.

### Task 5.2: Keep backend import command available for dev/test

**Files:**
- Modify: `src/lib/desktop.ts`
- Modify: `src-tauri/src/commands/transcript.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Keep exported helper with explicit comment**

In `src/lib/desktop.ts`, keep:

```ts
export async function importDesktopLocalAudio(filePath: string): Promise<TranscriptReview> {
  return invoke<TranscriptReview>("import_local_audio", { filePath });
}
```

Add a short comment above it:

```ts
// Dev/test helper only. The user-facing import UI is intentionally hidden in v1.2.1.
```

- [ ] **Step 2: Keep Rust command registered**

Confirm `src-tauri/src/lib.rs` still registers:

```rust
commands::transcript::import_local_audio,
```

- [ ] **Step 3: Run existing import tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml should_import_local_audio_and_generate_v05_timeline -- --nocapture
```

Expected: PASS. The command remains available for tests and developer workflows.

## Plan 6: Version, Release Docs, And Verification Records

### Task 6.1: Bump application version to 1.2.1

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Update npm package version**

Set both `package.json` and package root in `package-lock.json` to:

```json
"version": "1.2.1"
```

- [ ] **Step 2: Update Tauri/Rust versions**

Set `src-tauri/Cargo.toml`:

```toml
version = "1.2.1"
```

Set `src-tauri/tauri.conf.json`:

```json
"version": "1.2.1"
```

- [ ] **Step 3: Refresh lockfile**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS and `src-tauri/Cargo.lock` root package reflects `1.2.1`.

### Task 6.2: Add release/archive docs

**Files:**
- Create: `AI文档/04-发布记录/发布说明_v1.2.1.md`
- Create: `AI文档/版本归档/v1.2.1/归档清单.md`
- Create: `AI文档/版本归档/v1.2.1/验收记录.md`

- [ ] **Step 1: Create release note**

`AI文档/04-发布记录/发布说明_v1.2.1.md` must contain:

```markdown
# 发布说明 v1.2.1

## 范围

- 修复 issue #11 设置页布局、入口和版本显示问题。
- 本地 ASR 支持 argmax-cli / whisperkit-cli 探测、模型状态管理和 CLI 委托下载。
- 用户可见音频导入入口已移除，后端开发测试命令保留。
- 原“转写评估”入口升级为“录音片段”，支持基础片段、转写状态和说话人查看。

## 验证

- npm run build
- cargo check --manifest-path src-tauri/Cargo.toml
- cargo test --manifest-path src-tauri/Cargo.toml
- git diff --check
- subagent code review
- subagent functional review
```

- [ ] **Step 2: Create archive checklist**

`AI文档/版本归档/v1.2.1/归档清单.md` must contain changed module groups:

```markdown
# v1.2.1 归档清单

## 代码

- Rust local ASR domain/app/infra/commands
- Rust transcript service and model test command
- React settings page
- React recording segment page
- Version files

## 文档

- 优化需求设计
- 实施计划
- 发布说明
- 验收记录
```

- [ ] **Step 3: Create acceptance record**

`AI文档/版本归档/v1.2.1/验收记录.md` must include a table with rows for:

```markdown
| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 设置页云端 ASR 入口移除 |  |  |
| 本地 ASR runtime 探测 |  |  |
| 模型下载状态管理 |  |  |
| 用户可见导入音频入口移除 |  |  |
| 录音片段页 |  |  |
| 版本号唯一右下角 |  |  |
| npm run build |  |  |
| cargo check |  |  |
| cargo test |  |  |
| git diff --check |  |  |
| subagent code review |  |  |
| subagent functional review |  |  |
```

Fill the result/evidence cells after final verification commands finish.

## Plan 7: Full Verification, Subagent Reviews, Commit, And Push

### Task 7.1: Run full local verification

**Files:**
- Read: all changed files
- Modify: `AI文档/版本归档/v1.2.1/验收记录.md`

- [ ] **Step 1: Run frontend build**

```bash
npm run build
```

Expected: PASS.

- [ ] **Step 2: Run Rust check**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Run Rust tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 4: Run whitespace check**

```bash
git diff --check
```

Expected: no output and exit code 0.

- [ ] **Step 5: Run final issue string scan**

```bash
rg -n "火山云端 ASR|当前浏览器原型模式不支持云模型连接测试|本地音频路径|导入音频|转写评估|\"version\": \"1\\.1\\.1\"|version = \"1\\.1\\.1\"" src src-tauri package.json package-lock.json AI文档 docs
```

Expected: no user-visible stale strings. Remaining backend dev/test import command names are acceptable only if they do not render in `src/App.tsx`.

### Task 7.2: Run subagent code review and functional review

**Files:**
- Read-only review of final diff
- Modify: `AI文档/版本归档/v1.2.1/验收记录.md` only to record review results after main agent accepts findings.

- [ ] **Step 1: Dispatch code review subagent**

Review prompt:

```text
Review the v1.2.1 issue #11 diff for Rust/Tauri/React correctness. Focus on local ASR runtime probing, model download status persistence, command boundaries, TypeScript typing, hidden import UI, and version changes. Return only blocking findings with file/line references and concrete fixes.
```

Expected: findings are either empty or actionable. Main agent fixes blocking findings before continuing.

- [ ] **Step 2: Dispatch functional review subagent**

Review prompt:

```text
Review the implemented v1.2.1 issue #11 behavior against the approved design doc. Check settings page requirements, local ASR runtime/model states, recording segment minimum view, hidden import UI, version display, release docs, and verification commands. Return gaps only.
```

Expected: gaps are either empty or actionable. Main agent fixes blocking gaps before continuing.

- [ ] **Step 3: Record review outcomes**

Update `AI文档/版本归档/v1.2.1/验收记录.md` rows for:

```markdown
| subagent code review | 通过 | 无阻塞问题 |
| subagent functional review | 通过 | 无阻塞问题 |
```

If findings existed and were fixed, record the final fixed commit hash in the evidence cell.

### Task 7.3: Commit and push using project Git rules

**Files:**
- Stage all task-related changed files except user-owned `AI文档/设计参考/`.

- [ ] **Step 1: Review status**

```bash
git status --short
```

Expected: only task-related tracked/new files plus the pre-existing untracked `AI文档/设计参考/` directory.

- [ ] **Step 2: Stage intentionally**

```bash
git add docs/superpowers/plans/2026-06-15-v1-2-1-issue-11-local-asr-settings-plan.md \
  src src-tauri package.json package-lock.json \
  AI文档/04-发布记录/发布说明_v1.2.1.md \
  AI文档/版本归档/v1.2.1/归档清单.md \
  AI文档/版本归档/v1.2.1/验收记录.md
```

Expected: `AI文档/设计参考/` remains unstaged.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(local-asr): ship v1.2.1 issue 11 fixes"
```

Expected: commit succeeds.

- [ ] **Step 4: Push current branch**

```bash
git push origin feature-v1.2.1-issue-11-local-asr-settings
```

Expected: push succeeds.

---

## Self-Review

- Spec coverage: covered settings cleanup, single bottom-right version, `argmax-cli` and `whisperkit-cli` detection, default `large-v3-v20240930_626MB`, small-model switching, CLI-delegated download/status, hidden import UI, recording segment/speaker minimum view, version bump, release docs, and subagent reviews.
- Placeholder scan: this plan avoids unresolved marker words, vague completion instructions, and unnamed tests.
- Type consistency: Rust DTOs use camelCase through serde; TypeScript interfaces match those JSON keys; command names match planned Tauri invoke wrappers.
