use crate::{domain::desktop::DesktopContext, is_recording, providers, AppState};

pub(crate) fn build_desktop_context(recording: bool, provider_count: usize) -> DesktopContext {
    DesktopContext {
        runtime: "tauri".into(),
        platform: std::env::consts::OS.into(),
        recorder_status: if recording {
            "真实麦克风录音中".into()
        } else {
            "录音已停止，可启动真实麦克风录音".into()
        },
        storage_status: format!(
            "SQLite 已接入 settings / audio_segments / sessions / semantic_artifacts / model_invocations / todos；{} 个 provider 边界已注册",
            provider_count
        ),
        models_status: "Todo 语义入口已固定为 MiniMax M3；产物统一进入 semantic_m3 边界".into(),
    }
}

#[tauri::command]
pub(crate) fn get_desktop_context(
    state: tauri::State<'_, AppState>,
) -> Result<DesktopContext, String> {
    let recording = is_recording(&state)?;
    let provider_count = providers::provider_catalog().len();
    Ok(build_desktop_context(recording, provider_count))
}
