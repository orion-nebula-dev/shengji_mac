use crate::domain::local_asr::LocalAsrRuntimeDto;
use std::process::Command;
use std::{fs, path::Path};

pub(crate) trait LocalAsrCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String>;

    fn path_exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn create_dir_all(&self, path: &str) -> Result<(), String> {
        fs::create_dir_all(path).map_err(|error| format!("创建本地 ASR 缓存目录失败: {error}"))
    }

    fn write_file(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("创建本地 ASR 探测文件目录失败: {error}"))?;
        }
        fs::write(path, bytes).map_err(|error| format!("写入本地 ASR 探测音频失败: {error}"))
    }
}

pub(crate) struct SystemLocalAsrCommandRunner;

impl LocalAsrCommandRunner for SystemLocalAsrCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|error| format!("{program} 未安装或无法执行: {error}"))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                return Ok(stdout);
            }

            return Ok(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            return Err(stderr);
        }

        match output.status.code() {
            Some(code) => Err(format!("{program} 执行失败，退出码 {code}")),
            None => Err(format!("{program} 执行失败，进程被信号终止")),
        }
    }
}

pub(crate) fn probe_local_asr_runtimes<R: LocalAsrCommandRunner>(
    runner: &R,
) -> Vec<LocalAsrRuntimeDto> {
    ["argmax-cli", "whisperkit-cli"]
        .into_iter()
        .map(|program| match runner.run(program, &["--version"]) {
            Ok(version) => LocalAsrRuntimeDto {
                runtime_id: program.to_string(),
                display_name: local_asr_display_name(program),
                available: true,
                path: program.to_string(),
                version,
                error_message: String::new(),
            },
            Err(error_message) => LocalAsrRuntimeDto {
                runtime_id: program.to_string(),
                display_name: local_asr_display_name(program),
                available: false,
                path: String::new(),
                version: String::new(),
                error_message,
            },
        })
        .collect()
}

fn local_asr_display_name(program: &str) -> String {
    match program {
        "argmax-cli" => "Argmax CLI",
        "whisperkit-cli" => "WhisperKit CLI",
        _ => program,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ProbeRunner;

    impl LocalAsrCommandRunner for ProbeRunner {
        fn run(&self, program: &str, _args: &[&str]) -> Result<String, String> {
            match program {
                "argmax-cli" => Ok("argmax 0.1.0".to_string()),
                "whisperkit-cli" => Err("whisperkit-cli 未安装或不在 PATH 中".to_string()),
                unexpected => panic!("unexpected program: {unexpected}"),
            }
        }
    }

    #[test]
    fn probe_local_asr_runtimes_uses_stable_display_names_and_statuses() {
        let runtimes = probe_local_asr_runtimes(&ProbeRunner);

        assert_eq!(runtimes.len(), 2);
        assert_eq!(runtimes[0].runtime_id, "argmax-cli");
        assert_eq!(runtimes[0].display_name, "Argmax CLI");
        assert!(runtimes[0].available);
        assert_eq!(runtimes[0].version, "argmax 0.1.0");
        assert!(runtimes[0].error_message.is_empty());

        assert_eq!(runtimes[1].runtime_id, "whisperkit-cli");
        assert_eq!(runtimes[1].display_name, "WhisperKit CLI");
        assert!(!runtimes[1].available);
        assert!(runtimes[1].version.is_empty());
        assert!(runtimes[1].error_message.contains("未安装"));
    }

    #[test]
    fn argmax_model_path_uses_whisperkit_coreml_cache_layout() {
        let path = argmax_model_path("/tmp/shengji-models", "large-v3-v20240930_626MB");

        assert_eq!(
            path,
            "/tmp/shengji-models/whisperkit-coreml/openai_whisper-large-v3-v20240930_626MB"
        );
    }

    #[test]
    fn build_argmax_transcribe_args_uses_model_and_audio_paths() {
        let args = build_argmax_transcribe_args(
            "/tmp/models/openai_whisper-large",
            "/tmp/audio/sample.wav",
        );

        assert_eq!(
            args,
            vec![
                "transcribe",
                "--model-path",
                "/tmp/models/openai_whisper-large",
                "--audio-path",
                "/tmp/audio/sample.wav"
            ]
        );
    }

    #[test]
    fn build_argmax_serve_args_uses_model_name() {
        let args = build_argmax_serve_args("large-v3-v20240930_626MB");

        assert_eq!(args, vec!["serve", "--model", "large-v3-v20240930_626MB"]);
    }

    fn argmax_model_path(cache_dir: &str, model_name: &str) -> String {
        format!("{cache_dir}/whisperkit-coreml/openai_whisper-{model_name}")
    }

    fn build_argmax_transcribe_args(model_path: &str, audio_path: &str) -> Vec<String> {
        vec![
            "transcribe".to_string(),
            "--model-path".to_string(),
            model_path.to_string(),
            "--audio-path".to_string(),
            audio_path.to_string(),
        ]
    }

    fn build_argmax_serve_args(model_name: &str) -> Vec<String> {
        vec![
            "serve".to_string(),
            "--model".to_string(),
            model_name.to_string(),
        ]
    }
}
