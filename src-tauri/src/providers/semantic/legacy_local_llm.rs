use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

pub const MODEL_VERSION: &str = "qwen3-4b-instruct-2507-q4_k_m";
const RUNTIME_TIMEOUT_SECONDS: u64 = 60;
const HEALTH_CHECK_TIMEOUT_SECONDS: u64 = 60;
const GENERATION_TIMEOUT_SECONDS: u64 = 300;
const MANIFEST_BYTES: &[u8] = include_bytes!(
    "../../../resources/embedded_models/todo/qwen3-4b-instruct-2507-q4_k_m/manifest.json"
);
const PROMPT_TEMPLATE_BYTES: &[u8] = include_bytes!(
    "../../../resources/embedded_models/todo/qwen3-4b-instruct-2507-q4_k_m/prompt_template.txt"
);
const README_BYTES: &[u8] = include_bytes!(
    "../../../resources/embedded_models/todo/qwen3-4b-instruct-2507-q4_k_m/README.txt"
);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRequest {
    pub action: String,
    pub model_version: String,
    pub runtime_dir: String,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTodo {
    pub title: String,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResponse {
    pub success: bool,
    pub runtime_status: String,
    pub model_version: String,
    pub message: String,
    pub todos: Vec<RuntimeTodo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManifestAsset {
    pub path: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub model_version: String,
    pub runtime_type: String,
    pub engine: String,
    pub description: String,
    pub executable_rel_path: String,
    pub model_rel_path: String,
    pub prompt_template_rel_path: String,
    pub context_length: u32,
    pub max_output_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub gpu_layers: i32,
    pub assets: Vec<ManifestAsset>,
}

#[derive(Debug)]
pub struct RuntimeConfig {
    executable_path: PathBuf,
    completion_executable_path: PathBuf,
    model_path: PathBuf,
    prompt_template_path: PathBuf,
    manifest: Manifest,
}

struct EmbeddedAsset {
    relative_path: &'static str,
    bytes: &'static [u8],
}

pub fn normalize_model_version(version: &str) -> String {
    let trimmed = version.trim();
    if trimmed.is_empty() || trimmed == "todo-embedded-v1" {
        MODEL_VERSION.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn model_dir(models_dir: &PathBuf, version: &str) -> PathBuf {
    models_dir.join("todo").join(version)
}

pub fn manifest_path(models_dir: &PathBuf, version: &str) -> PathBuf {
    model_dir(models_dir, version).join("manifest.json")
}

pub fn checksums_path(models_dir: &PathBuf, version: &str) -> PathBuf {
    model_dir(models_dir, version).join("checksums.sha256")
}

pub fn embedded_manifest_text() -> Result<&'static str, String> {
    std::str::from_utf8(MANIFEST_BYTES)
        .map_err(|error| format!("解析内嵌模型清单文本失败: {error}"))
}

pub fn parse_manifest_text(input: &str) -> Result<Manifest, String> {
    serde_json::from_str::<Manifest>(input)
        .map_err(|error| format!("解析内嵌模型清单失败: {error}"))
}

pub fn build_prompt(template: &str, merged_text: &str) -> String {
    template.replace("{{input_text}}", merged_text.trim())
}

pub fn ensure_runtime_files(models_dir: &PathBuf) -> Result<(), String> {
    let target_model_dir = model_dir(models_dir, MODEL_VERSION);
    fs::create_dir_all(&target_model_dir)
        .map_err(|error| format!("创建本地模型目录失败: {error}"))?;

    let mut checksum_lines = Vec::new();
    for asset in embedded_assets() {
        let target_path = target_model_dir.join(asset.relative_path);
        let should_write = if target_path.exists() {
            let existing =
                fs::read(&target_path).map_err(|error| format!("读取本地模型文件失败: {error}"))?;
            sha256_hex(&existing) != sha256_hex(asset.bytes)
        } else {
            true
        };

        if should_write {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("创建本地模型子目录失败: {error}"))?;
            }
            fs::write(&target_path, asset.bytes)
                .map_err(|error| format!("释放本地模型文件失败: {error}"))?;
        }

        checksum_lines.push(format!(
            "{}  {}",
            sha256_hex(asset.bytes),
            asset.relative_path
        ));
    }

    fs::write(
        checksums_path(models_dir, MODEL_VERSION),
        checksum_lines.join("\n"),
    )
    .map_err(|error| format!("写入本地模型校验文件失败: {error}"))?;

    let manifest = parse_manifest_text(embedded_manifest_text()?)?;
    let executable_parent = target_model_dir.join(&manifest.executable_rel_path);
    if let Some(parent) = executable_parent.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 llama.cpp 目录失败: {error}"))?;
    }
    let model_parent = target_model_dir.join(&manifest.model_rel_path);
    if let Some(parent) = model_parent.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 GGUF 模型目录失败: {error}"))?;
    }

    Ok(())
}

pub fn verify_runtime_files(models_dir: &PathBuf, version: &str) -> Result<(), String> {
    let target_model_dir = model_dir(models_dir, version);
    for asset in embedded_assets() {
        let target_path = target_model_dir.join(asset.relative_path);
        if !target_path.exists() {
            return Err(format!("缺少本地模型文件: {}", asset.relative_path));
        }

        let existing =
            fs::read(&target_path).map_err(|error| format!("读取本地模型文件失败: {error}"))?;
        if sha256_hex(&existing) != sha256_hex(asset.bytes) {
            return Err(format!("本地模型文件校验失败: {}", asset.relative_path));
        }
    }

    let checksum_path = checksums_path(models_dir, version);
    if !checksum_path.exists() {
        return Err("缺少本地模型校验文件".to_string());
    }

    Ok(())
}

pub fn load_runtime_config(models_dir: &PathBuf, version: &str) -> Result<RuntimeConfig, String> {
    verify_runtime_files(models_dir, version)?;
    let runtime_dir = model_dir(models_dir, version);
    let manifest = read_manifest(models_dir, version)?;
    let executable_path = resolve_runtime_override_path("SMART_TODO_LLAMA_CLI_PATH")
        .unwrap_or_else(|| runtime_dir.join(&manifest.executable_rel_path));
    let completion_executable_path = executable_path
        .parent()
        .map(|parent| parent.join("llama-completion"))
        .unwrap_or_else(|| PathBuf::from("llama-completion"));
    let model_path = resolve_runtime_override_path("SMART_TODO_QWEN_GGUF_PATH")
        .unwrap_or_else(|| runtime_dir.join(&manifest.model_rel_path));
    let prompt_template_path = runtime_dir.join(&manifest.prompt_template_rel_path);

    if !prompt_template_path.exists() {
        return Err(format!(
            "缺少 Prompt 模板文件: {}",
            prompt_template_path.display()
        ));
    }
    if !executable_path.exists() {
        return Err(format!(
            "未找到 llama.cpp 可执行文件，请放入 {} 或设置 SMART_TODO_LLAMA_CLI_PATH",
            executable_path.display()
        ));
    }
    if !completion_executable_path.exists() {
        return Err(format!(
            "未找到 llama-completion 可执行文件，请放入 {}",
            completion_executable_path.display()
        ));
    }
    if !model_path.exists() {
        return Err(format!(
            "未找到 Qwen GGUF 模型文件，请放入 {} 或设置 SMART_TODO_QWEN_GGUF_PATH",
            model_path.display()
        ));
    }

    Ok(RuntimeConfig {
        executable_path,
        completion_executable_path,
        model_path,
        prompt_template_path,
        manifest,
    })
}

pub fn health_check(config: &RuntimeConfig) -> Result<String, String> {
    let mut command = Command::new(&config.executable_path);
    command.arg("--version");
    let output = run_command_with_timeout(command, HEALTH_CHECK_TIMEOUT_SECONDS)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "llama.cpp 健康检查失败: {}",
            if stderr.is_empty() {
                "未知错误".to_string()
            } else {
                stderr
            }
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if stdout.is_empty() {
        "llama.cpp 可执行文件已就绪".to_string()
    } else {
        clip_text(&stdout, 120)
    })
}

pub fn parse_runtime_todos(output_text: &str) -> Result<Vec<RuntimeTodo>, String> {
    let items = extract_json_array(output_text)?;
    let todos = items
        .into_iter()
        .filter_map(|item| {
            let title = item.get("title")?.as_str()?.trim().to_string();
            let note = item
                .get("note")
                .and_then(|entry| entry.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if title.is_empty() {
                return None;
            }
            Some(RuntimeTodo {
                title: clip_text(&title, 18),
                note,
            })
        })
        .collect::<Vec<_>>();

    Ok(todos)
}

pub fn invoke_todo_extraction(
    config: &RuntimeConfig,
    merged_text: &str,
) -> Result<Vec<RuntimeTodo>, String> {
    let prompt_template = fs::read_to_string(&config.prompt_template_path)
        .map_err(|error| format!("读取 Prompt 模板失败: {error}"))?;
    let prompt = build_prompt(&prompt_template, merged_text);

    let mut command = Command::new(&config.completion_executable_path);
    command
        .arg("-m")
        .arg(&config.model_path)
        .arg("-c")
        .arg(config.manifest.context_length.to_string())
        .arg("-n")
        .arg(config.manifest.max_output_tokens.to_string())
        .arg("--temp")
        .arg(config.manifest.temperature.to_string())
        .arg("--top-p")
        .arg(config.manifest.top_p.to_string())
        .arg("--repeat-penalty")
        .arg(config.manifest.repeat_penalty.to_string())
        .arg("--no-conversation")
        .arg("--no-warmup")
        .arg("--reasoning")
        .arg("off")
        .arg("--simple-io")
        .arg("-p")
        .arg(prompt);

    if config.manifest.gpu_layers >= 0 {
        command
            .arg("-ngl")
            .arg(config.manifest.gpu_layers.to_string());
    }

    let output = run_command_with_timeout(command, GENERATION_TIMEOUT_SECONDS)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "llama.cpp 推理失败: {}",
            if stderr.is_empty() {
                "未知错误".to_string()
            } else {
                clip_text(&stderr, 300)
            }
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("llama.cpp 未返回任何文本输出".to_string());
    }

    parse_runtime_todos(&stdout)
}

pub fn spawn_runtime_request(request: &RuntimeRequest) -> Result<RuntimeResponse, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("读取当前应用路径失败: {error}"))?;
    let request_bytes = serde_json::to_vec(request)
        .map_err(|error| format!("序列化本地运行时请求失败: {error}"))?;

    let mut child = Command::new(executable)
        .env_remove("SMART_TODO_PROCESS_PENDING_ONCE")
        .env_remove("SMART_TODO_DB_PATH")
        .env("SMART_TODO_EMBEDDED_TODO_RUNTIME", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动本地 Todo 子进程失败: {error}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(&request_bytes)
            .map_err(|error| format!("写入本地运行时请求失败: {error}"))?;
    }

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = tx.send(output);
    });

    let output = match rx.recv_timeout(Duration::from_secs(RUNTIME_TIMEOUT_SECONDS)) {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => return Err(format!("等待本地 Todo 子进程失败: {error}")),
        Err(_) => return Err("本地 Todo 子进程执行超时".to_string()),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "本地 Todo 子进程执行失败: {}",
            if stderr.is_empty() {
                "未知错误".to_string()
            } else {
                stderr
            }
        ));
    }

    serde_json::from_slice::<RuntimeResponse>(&output.stdout)
        .map_err(|error| format!("解析本地运行时响应失败: {error}"))
}

pub fn request_todo_extraction(
    models_dir: &PathBuf,
    model_version: &str,
    merged_text: &str,
) -> Result<Vec<RuntimeTodo>, String> {
    let response = spawn_runtime_request(&RuntimeRequest {
        action: "extract_todos".into(),
        model_version: normalize_model_version(model_version),
        runtime_dir: models_dir.to_string_lossy().to_string(),
        text: merged_text.to_string(),
    })?;

    if !response.success {
        return Err(response.message);
    }

    Ok(response.todos)
}

pub fn run_once() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("读取本地运行时输入失败: {error}"))?;

    let request: RuntimeRequest =
        serde_json::from_str(&input).map_err(|error| format!("解析本地运行时请求失败: {error}"))?;
    let runtime_dir = PathBuf::from(request.runtime_dir.as_str());
    let response = match request.action.as_str() {
        "health_check" => match load_runtime_config(&runtime_dir, &request.model_version) {
            Ok(config) => match health_check(&config) {
                Ok(message) => RuntimeResponse {
                    success: true,
                    runtime_status: "ready".into(),
                    model_version: request.model_version,
                    message,
                    todos: Vec::new(),
                },
                Err(error) => RuntimeResponse {
                    success: false,
                    runtime_status: "failed".into(),
                    model_version: request.model_version,
                    message: error,
                    todos: Vec::new(),
                },
            },
            Err(error) => RuntimeResponse {
                success: false,
                runtime_status: "not_ready".into(),
                model_version: request.model_version,
                message: error,
                todos: Vec::new(),
            },
        },
        "extract_todos" => match load_runtime_config(&runtime_dir, &request.model_version) {
            Ok(config) => match invoke_todo_extraction(&config, &request.text) {
                Ok(todos) => RuntimeResponse {
                    success: true,
                    runtime_status: "ready".into(),
                    model_version: request.model_version,
                    message: if todos.is_empty() {
                        "Qwen3-4B Q4_K_M 未识别到明确待办".into()
                    } else {
                        format!("Qwen3-4B Q4_K_M 识别出 {} 条待办", todos.len())
                    },
                    todos,
                },
                Err(error) => RuntimeResponse {
                    success: false,
                    runtime_status: "failed".into(),
                    model_version: request.model_version,
                    message: error,
                    todos: Vec::new(),
                },
            },
            Err(error) => RuntimeResponse {
                success: false,
                runtime_status: "not_ready".into(),
                model_version: request.model_version,
                message: error,
                todos: Vec::new(),
            },
        },
        _ => return Err(format!("不支持的本地运行时动作: {}", request.action)),
    };

    let stdout = serde_json::to_string(&response)
        .map_err(|error| format!("序列化本地运行时响应失败: {error}"))?;
    println!("{stdout}");
    Ok(())
}

fn embedded_assets() -> [EmbeddedAsset; 3] {
    [
        EmbeddedAsset {
            relative_path: "manifest.json",
            bytes: MANIFEST_BYTES,
        },
        EmbeddedAsset {
            relative_path: "prompt_template.txt",
            bytes: PROMPT_TEMPLATE_BYTES,
        },
        EmbeddedAsset {
            relative_path: "README.txt",
            bytes: README_BYTES,
        },
    ]
}

fn read_manifest(models_dir: &PathBuf, version: &str) -> Result<Manifest, String> {
    let manifest_path = manifest_path(models_dir, version);
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|error| format!("读取模型清单失败: {error}"))?;
    parse_manifest_text(&manifest_text)
}

fn resolve_runtime_override_path(env_name: &str) -> Option<PathBuf> {
    std::env::var(env_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn run_command_with_timeout(
    mut command: Command,
    timeout_seconds: u64,
) -> Result<std::process::Output, String> {
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动外部命令失败: {error}"))?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = tx.send(output);
    });

    match rx.recv_timeout(Duration::from_secs(timeout_seconds)) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(format!("等待外部命令失败: {error}")),
        Err(_) => Err(format!("外部命令执行超时（{} 秒）", timeout_seconds)),
    }
}

fn extract_json_array(input: &str) -> Result<Vec<serde_json::Value>, String> {
    if let Ok(value) = serde_json::from_str::<Vec<serde_json::Value>>(input) {
        return Ok(value);
    }

    let starts = input
        .match_indices('[')
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();
    let ends = input
        .match_indices(']')
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();

    for start in starts.iter().rev() {
        for end in ends.iter().rev() {
            if end < start {
                continue;
            }

            let candidate = &input[*start..=*end];
            if let Ok(value) = serde_json::from_str::<Vec<serde_json::Value>>(candidate) {
                return Ok(value);
            }
        }
    }

    Err("无法从模型响应中解析 Todo JSON 数组".to_string())
}

fn clip_text(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}
