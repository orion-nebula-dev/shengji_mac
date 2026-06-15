use rusqlite::{params, Connection, Error::QueryReturnedNoRows};
use std::{env, path::PathBuf};

use crate::{
    domain::local_asr::{
        LocalAsrModelDto, LocalAsrModelStatusDto, LocalAsrRuntimeDto, LocalAsrStateDto,
        DEFAULT_LOCAL_ASR_MODEL, LOCAL_ASR_CACHE_DIR, LOCAL_ASR_PROVIDER,
    },
    infra::local_asr_runtime::{
        probe_local_asr_runtimes, LocalAsrCommandRunner, SystemLocalAsrCommandRunner,
    },
};

const LOCAL_ASR_DEVICE_RECOMMENDATION: &str =
    "默认使用 large-v3-v20240930_626MB；设备或下载失败时可切换 base/tiny。";
const LOCAL_ASR_RUNTIME_MISSING_ERROR: &str =
    "未检测到 argmax-cli 或 whisperkit-cli，请先安装本地 ASR CLI。";
const LOCAL_ASR_MODEL_MISSING_ERROR: &str = "本地 ASR 模型尚未下载，请先在设置页下载模型。";
const LOCAL_ASR_MODEL_VERIFY_ERROR: &str =
    "本地 ASR 下载命令已结束，但未在应用模型缓存目录找到模型文件，请重试或切换 runtime。";
const STALE_LOCAL_ASR_DOWNLOAD_ERROR: &str = "上次下载未完成，请重试。";
const MAX_LOCAL_ASR_ERROR_MESSAGE_CHARS: usize = 200;

pub(crate) fn local_asr_model_catalog() -> Vec<LocalAsrModelDto> {
    vec![
        LocalAsrModelDto {
            model_name: DEFAULT_LOCAL_ASR_MODEL.into(),
            label: "Large v3".into(),
            size_hint: "626MB".into(),
            quality_hint: "默认，高质量多语言".into(),
            recommended: true,
        },
        LocalAsrModelDto {
            model_name: "base".into(),
            label: "Base".into(),
            size_hint: "约140MB".into(),
            quality_hint: "速度优先，基础质量".into(),
            recommended: false,
        },
        LocalAsrModelDto {
            model_name: "tiny".into(),
            label: "Tiny".into(),
            size_hint: "约75MB".into(),
            quality_hint: "最快，轻量试用".into(),
            recommended: false,
        },
    ]
}

pub(crate) fn query_local_asr_model_status(
    connection: &Connection,
) -> Result<LocalAsrModelStatusDto, String> {
    match connection.query_row(
        r#"
        SELECT provider, model_name, cache_dir, download_status, download_progress, offline_available, device_recommendation, error_message
        FROM local_model_status
        WHERE provider = ?1
        "#,
        params![LOCAL_ASR_PROVIDER],
        |row| {
            Ok(LocalAsrModelStatusDto {
                provider: row.get(0)?,
                model_name: row.get(1)?,
                cache_dir: row.get(2)?,
                download_status: row.get(3)?,
                download_progress: row.get(4)?,
                offline_available: row.get::<_, i64>(5)? == 1,
                device_recommendation: row.get(6)?,
                error_message: row.get(7)?,
            })
        },
    ) {
        Ok(status) => Ok(status),
        Err(QueryReturnedNoRows) => Ok(default_local_asr_model_status()),
        Err(error) => Err(format!("读取本地 ASR 模型状态失败: {error}")),
    }
}

pub(crate) fn get_local_asr_state(connection: &Connection) -> Result<LocalAsrStateDto, String> {
    let model_status = query_local_asr_model_status(connection)?;

    Ok(LocalAsrStateDto {
        runtimes: query_local_asr_runtime_statuses(connection)?,
        models: local_asr_model_catalog(),
        selected_model: model_status.model_name.clone(),
        model_status,
    })
}

pub(crate) fn refresh_local_asr_runtimes(
    connection: &Connection,
) -> Result<Vec<LocalAsrRuntimeDto>, String> {
    refresh_local_asr_runtimes_with_runner(connection, &SystemLocalAsrCommandRunner)
}

pub(crate) fn refresh_local_asr_runtimes_with_runner<R: LocalAsrCommandRunner>(
    connection: &Connection,
    runner: &R,
) -> Result<Vec<LocalAsrRuntimeDto>, String> {
    let runtimes = probe_local_asr_runtimes(runner)
        .into_iter()
        .map(normalize_runtime_path)
        .collect::<Vec<_>>();

    for runtime in &runtimes {
        connection
            .execute(
                r#"
                INSERT INTO local_asr_runtime_status (
                  runtime_id,
                  display_name,
                  available,
                  path,
                  version,
                  error_message,
                  updated_at
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
                    if runtime.available { 1 } else { 0 },
                    runtime.path,
                    runtime.version,
                    clip_local_asr_error_message(&runtime.error_message)
                ],
            )
            .map_err(|error| format!("更新本地 ASR runtime 状态失败: {error}"))?;
    }

    Ok(runtimes)
}

pub(crate) fn ensure_local_asr_ready(
    connection: &Connection,
) -> Result<LocalAsrModelStatusDto, String> {
    let runtimes = query_local_asr_runtime_statuses(connection)?;
    if !runtimes.iter().any(|runtime| runtime.available) {
        return Err(LOCAL_ASR_RUNTIME_MISSING_ERROR.to_string());
    }

    let model_status = query_local_asr_model_status(connection)?;
    if model_status.download_status != "available" || !model_status.offline_available {
        return Err(LOCAL_ASR_MODEL_MISSING_ERROR.to_string());
    }

    Ok(model_status)
}

pub(crate) fn transcribe_local_asr_audio_with_runner<R: LocalAsrCommandRunner>(
    connection: &Connection,
    audio_path: &str,
    runner: &R,
) -> Result<String, String> {
    let model_status = ensure_local_asr_ready(connection)?;
    let transcribe_runtime = select_local_asr_download_runtime(connection)?;
    let Some(runtime_id) = transcribe_runtime else {
        return Err(LOCAL_ASR_RUNTIME_MISSING_ERROR.to_string());
    };
    let expanded_cache_dir = expand_local_asr_cache_dir(&model_status.cache_dir);
    let model_path = local_asr_model_path(&expanded_cache_dir, &model_status.model_name);

    let result = match runtime_id {
        "argmax-cli" => runner.run(
            "argmax-cli",
            &[
                "transcribe",
                "--model-path",
                model_path.as_str(),
                "--audio-path",
                audio_path,
            ],
        ),
        "whisperkit-cli" => runner.run(
            "whisperkit-cli",
            &[
                "transcribe",
                "--model-path",
                model_path.as_str(),
                "--audio-path",
                audio_path,
            ],
        ),
        _ => Err("不支持的本地 ASR runtime".to_string()),
    };

    match result {
        Ok(output) => Ok(normalize_local_asr_transcript(&output)),
        Err(error) => {
            let mut error_message = clip_local_asr_error_message(&error);
            if error_message.is_empty() {
                error_message = "本地 ASR 转写失败".to_string();
            }
            Err(error_message)
        }
    }
}

pub(crate) fn download_local_asr_model(
    connection: &Connection,
    model_name: &str,
) -> Result<LocalAsrModelStatusDto, String> {
    download_local_asr_model_with_runner(connection, model_name, &SystemLocalAsrCommandRunner)
}

pub(crate) fn download_local_asr_model_with_runner<R: LocalAsrCommandRunner>(
    connection: &Connection,
    model_name: &str,
    runner: &R,
) -> Result<LocalAsrModelStatusDto, String> {
    let selected_model = validate_local_asr_model(model_name)?;
    converge_stale_downloading_status(connection)?;
    let download_runtime = select_local_asr_download_runtime(connection)?;
    let Some(runtime_id) = download_runtime else {
        update_local_asr_model_status(
            connection,
            selected_model,
            "failed",
            0,
            false,
            LOCAL_ASR_RUNTIME_MISSING_ERROR,
        )?;
        return Err(LOCAL_ASR_RUNTIME_MISSING_ERROR.to_string());
    };

    update_local_asr_model_status(connection, selected_model, "downloading", 10, false, "")?;

    let cache_dir = expand_local_asr_cache_dir(LOCAL_ASR_CACHE_DIR);
    if verify_downloaded_local_asr_model(runner, &cache_dir, selected_model, "").is_ok() {
        return update_local_asr_model_status(
            connection,
            selected_model,
            "available",
            100,
            true,
            "",
        );
    }

    let download_result = match runtime_id {
        "argmax-cli" | "whisperkit-cli" => {
            run_local_asr_model_bootstrap(runner, runtime_id, &cache_dir, selected_model)
        }
        _ => Err("不支持的本地 ASR runtime".to_string()),
    };

    match download_result {
        Ok(output) => {
            match verify_downloaded_local_asr_model(runner, &cache_dir, selected_model, &output) {
                Ok(()) => update_local_asr_model_status(
                    connection,
                    selected_model,
                    "available",
                    100,
                    true,
                    "",
                ),
                Err(error_message) => {
                    update_local_asr_model_status(
                        connection,
                        selected_model,
                        "failed",
                        0,
                        false,
                        &error_message,
                    )?;
                    Err(error_message)
                }
            }
        },
        Err(error) => {
            let mut error_message = clip_local_asr_error_message(&error);
            if error_message.is_empty() {
                error_message = "本地 ASR 模型下载失败".to_string();
            }
            update_local_asr_model_status(
                connection,
                selected_model,
                "failed",
                0,
                false,
                &error_message,
            )?;
            Err(error_message)
        }
    }
}

fn converge_stale_downloading_status(connection: &Connection) -> Result<(), String> {
    let model_status = query_local_asr_model_status(connection)?;
    if model_status.download_status == "downloading" {
        update_local_asr_model_status(
            connection,
            &model_status.model_name,
            "failed",
            0,
            false,
            STALE_LOCAL_ASR_DOWNLOAD_ERROR,
        )?;
    }
    Ok(())
}

pub(crate) fn select_local_asr_model(
    connection: &Connection,
    model_name: &str,
) -> Result<LocalAsrStateDto, String> {
    let selected_model = validate_local_asr_model(model_name)?;

    connection
        .execute(
            r#"
            INSERT INTO local_model_status (
              provider,
              model_name,
              cache_dir,
              download_status,
              download_progress,
              offline_available,
              device_recommendation,
              error_message,
              updated_at
            ) VALUES (?1, ?2, ?3, 'not_started', 0, 0, ?4, '', CURRENT_TIMESTAMP)
            ON CONFLICT(provider) DO UPDATE SET
              model_name = excluded.model_name,
              cache_dir = excluded.cache_dir,
              download_status = 'not_started',
              download_progress = 0,
              offline_available = 0,
              device_recommendation = excluded.device_recommendation,
              error_message = '',
              updated_at = CURRENT_TIMESTAMP
            "#,
            params![
                LOCAL_ASR_PROVIDER,
                selected_model,
                LOCAL_ASR_CACHE_DIR,
                LOCAL_ASR_DEVICE_RECOMMENDATION
            ],
        )
        .map_err(|error| format!("选择本地 ASR 模型失败: {error}"))?;

    get_local_asr_state(connection)
}

fn validate_local_asr_model(model_name: &str) -> Result<&str, String> {
    let selected_model = model_name.trim();
    if !local_asr_model_catalog()
        .iter()
        .any(|model| model.model_name == selected_model)
    {
        return Err("不支持的本地 ASR 模型".to_string());
    }
    Ok(selected_model)
}

fn local_asr_model_repo_dir(cache_dir: &str) -> String {
    format!("{cache_dir}/whisperkit-coreml")
}

fn local_asr_model_path(cache_dir: &str, model_name: &str) -> String {
    format!(
        "{}/openai_whisper-{model_name}",
        local_asr_model_repo_dir(cache_dir)
    )
}

fn expand_local_asr_cache_dir(cache_dir: &str) -> String {
    let trimmed = cache_dir.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Ok(home_dir) = env::var("HOME") {
            return PathBuf::from(home_dir)
                .join(rest)
                .to_string_lossy()
                .into_owned();
        }
    }
    trimmed.to_string()
}

fn verify_downloaded_local_asr_model<R: LocalAsrCommandRunner>(
    runner: &R,
    cache_dir: &str,
    model_name: &str,
    cli_output: &str,
) -> Result<(), String> {
    let model_path = local_asr_model_path(cache_dir, model_name);
    if runner.path_exists(&model_path) || cli_output_reports_model_path(cli_output, &model_path) {
        Ok(())
    } else {
        Err(LOCAL_ASR_MODEL_VERIFY_ERROR.to_string())
    }
}

fn run_local_asr_model_bootstrap<R: LocalAsrCommandRunner>(
    runner: &R,
    runtime_id: &str,
    cache_dir: &str,
    model_name: &str,
) -> Result<String, String> {
    let model_repo_dir = local_asr_model_repo_dir(cache_dir);
    runner.create_dir_all(&model_repo_dir)?;

    let probe_audio_path = format!("{cache_dir}/.shengji-local-asr-bootstrap.wav");
    runner.write_file(&probe_audio_path, &build_local_asr_silent_wav())?;

    runner.run(
        runtime_id,
        &[
            "transcribe",
            "--audio-path",
            probe_audio_path.as_str(),
            "--model",
            model_name,
            "--download-model-path",
            model_repo_dir.as_str(),
            "--download-tokenizer-path",
            model_repo_dir.as_str(),
            "--without-timestamps",
            "--verbose",
        ],
    )
}

fn cli_output_reports_model_path(output: &str, model_path: &str) -> bool {
    if output.trim().is_empty() {
        return false;
    }
    output.contains(model_path)
        && (output.contains("Model folder")
            || output.contains("Model initialization complete")
            || output.contains("Transcription of"))
}

fn build_local_asr_silent_wav() -> Vec<u8> {
    let sample_rate = 16_000u32;
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let samples = 1_600u32;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = samples * block_align as u32;
    let riff_size = 36 + data_size;

    let mut bytes = Vec::with_capacity((44 + data_size) as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    bytes.resize((44 + data_size) as usize, 0);
    bytes
}

fn normalize_local_asr_transcript(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn select_local_asr_download_runtime(
    connection: &Connection,
) -> Result<Option<&'static str>, String> {
    let runtimes = query_local_asr_runtime_statuses(connection)?;
    if runtimes
        .iter()
        .any(|runtime| runtime.runtime_id == "argmax-cli" && runtime.available)
    {
        return Ok(Some("argmax-cli"));
    }
    if runtimes
        .iter()
        .any(|runtime| runtime.runtime_id == "whisperkit-cli" && runtime.available)
    {
        return Ok(Some("whisperkit-cli"));
    }
    Ok(None)
}

fn update_local_asr_model_status(
    connection: &Connection,
    model_name: &str,
    download_status: &str,
    download_progress: i64,
    offline_available: bool,
    error_message: &str,
) -> Result<LocalAsrModelStatusDto, String> {
    let clipped_error_message = clip_local_asr_error_message(error_message);
    connection
        .execute(
            r#"
            INSERT INTO local_model_status (
              provider,
              model_name,
              cache_dir,
              download_status,
              download_progress,
              offline_available,
              device_recommendation,
              error_message,
              updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CURRENT_TIMESTAMP)
            ON CONFLICT(provider) DO UPDATE SET
              model_name = excluded.model_name,
              cache_dir = excluded.cache_dir,
              download_status = excluded.download_status,
              download_progress = excluded.download_progress,
              offline_available = excluded.offline_available,
              device_recommendation = excluded.device_recommendation,
              error_message = excluded.error_message,
              updated_at = CURRENT_TIMESTAMP
            "#,
            params![
                LOCAL_ASR_PROVIDER,
                model_name,
                LOCAL_ASR_CACHE_DIR,
                download_status,
                download_progress,
                if offline_available { 1 } else { 0 },
                LOCAL_ASR_DEVICE_RECOMMENDATION,
                clipped_error_message
            ],
        )
        .map_err(|error| format!("更新本地 ASR 模型状态失败: {error}"))?;

    query_local_asr_model_status(connection)
}

fn clip_local_asr_error_message(error_message: &str) -> String {
    let trimmed = error_message.trim();
    let sanitized = redact_local_asr_error_message(trimmed);
    sanitized
        .chars()
        .take(MAX_LOCAL_ASR_ERROR_MESSAGE_CHARS)
        .collect()
}

fn redact_local_asr_error_message(message: &str) -> String {
    let without_user_paths =
        redact_sensitive_spans(message, "/Users/", "[local-path]", is_local_path_delimiter);
    let without_volume_paths = redact_sensitive_spans(
        &without_user_paths,
        "/Volumes/",
        "[local-path]",
        is_local_path_delimiter,
    );
    let without_private_paths = redact_sensitive_spans(
        &without_volume_paths,
        "/private/var/",
        "[local-path]",
        is_local_path_delimiter,
    );
    let without_bearer = redact_sensitive_spans(
        &without_private_paths,
        "Bearer",
        "[redacted]",
        is_secret_token_delimiter,
    );
    redact_sensitive_spans(
        &without_bearer,
        "sk-",
        "[redacted]",
        is_secret_token_delimiter,
    )
}

fn redact_sensitive_spans(
    message: &str,
    marker: &str,
    replacement: &str,
    is_delimiter: fn(char) -> bool,
) -> String {
    let mut output = String::new();
    let mut rest = message;

    while let Some(start) = rest.find(marker) {
        output.push_str(&rest[..start]);
        output.push_str(replacement);
        let after_marker = &rest[start + marker.len()..];
        let span_length = if marker == "Bearer" {
            bearer_token_span_length(after_marker, is_delimiter)
        } else {
            sensitive_span_length(after_marker, is_delimiter)
        };
        rest = &after_marker[span_length..];
    }

    output.push_str(rest);
    output
}

fn sensitive_span_length(message: &str, is_delimiter: fn(char) -> bool) -> usize {
    for (index, ch) in message.char_indices() {
        if is_delimiter(ch) {
            return index;
        }
    }
    message.len()
}

fn bearer_token_span_length(message: &str, is_delimiter: fn(char) -> bool) -> usize {
    let mut token_started = false;
    for (index, ch) in message.char_indices() {
        if !token_started && ch.is_whitespace() {
            continue;
        }
        token_started = true;
        if is_delimiter(ch) {
            return index;
        }
    }
    message.len()
}

fn is_local_path_delimiter(ch: char) -> bool {
    matches!(
        ch,
        ',' | '，' | ';' | '；' | '\n' | '\r' | '"' | '\'' | ')' | '）'
    )
}

fn is_secret_token_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            ',' | '，' | ';' | '；' | '\n' | '\r' | '"' | '\'' | ')' | '）'
        )
}

fn query_local_asr_runtime_statuses(
    connection: &Connection,
) -> Result<Vec<LocalAsrRuntimeDto>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT runtime_id, display_name, available, path, version, error_message
            FROM local_asr_runtime_status
            ORDER BY CASE runtime_id
              WHEN 'argmax-cli' THEN 0
              WHEN 'whisperkit-cli' THEN 1
              ELSE 2
            END, runtime_id
            "#,
        )
        .map_err(|error| format!("读取本地 ASR runtime 状态失败: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(LocalAsrRuntimeDto {
                runtime_id: row.get(0)?,
                display_name: row.get(1)?,
                available: row.get::<_, i64>(2)? == 1,
                path: row.get(3)?,
                version: row.get(4)?,
                error_message: row.get(5)?,
            })
        })
        .map_err(|error| format!("读取本地 ASR runtime 状态失败: {error}"))?;

    let runtimes = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("解析本地 ASR runtime 状态失败: {error}"))?;

    if runtimes.is_empty() {
        return Ok(default_local_asr_runtimes());
    }

    Ok(runtimes.into_iter().map(normalize_runtime_path).collect())
}

fn default_local_asr_model_status() -> LocalAsrModelStatusDto {
    LocalAsrModelStatusDto {
        provider: LOCAL_ASR_PROVIDER.into(),
        model_name: DEFAULT_LOCAL_ASR_MODEL.into(),
        cache_dir: LOCAL_ASR_CACHE_DIR.into(),
        download_status: "not_started".into(),
        download_progress: 0,
        offline_available: false,
        device_recommendation: LOCAL_ASR_DEVICE_RECOMMENDATION.into(),
        error_message: String::new(),
    }
}

fn default_local_asr_runtimes() -> Vec<LocalAsrRuntimeDto> {
    vec![
        default_local_asr_runtime("argmax-cli", "Argmax CLI"),
        default_local_asr_runtime("whisperkit-cli", "WhisperKit CLI"),
    ]
}

fn default_local_asr_runtime(runtime_id: &str, display_name: &str) -> LocalAsrRuntimeDto {
    LocalAsrRuntimeDto {
        runtime_id: runtime_id.into(),
        display_name: display_name.into(),
        available: false,
        path: String::new(),
        version: String::new(),
        error_message: "尚未检测本地 ASR runtime".into(),
    }
}

fn normalize_runtime_path(mut runtime: LocalAsrRuntimeDto) -> LocalAsrRuntimeDto {
    runtime.path = if runtime.available {
        runtime.runtime_id.clone()
    } else {
        String::new()
    };
    runtime.error_message = clip_local_asr_error_message(&runtime.error_message);
    runtime
}
