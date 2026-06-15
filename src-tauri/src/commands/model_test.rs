use crate::{
    domain::model_test::{ModelTestRequest, ModelTestResult},
    infra::sqlite::open_connection,
    test_asr_provider, AppState,
};
use rusqlite::Connection;

#[cfg(test)]
pub(crate) fn test_model_connection_payload(
    payload: ModelTestRequest,
) -> Result<ModelTestResult, String> {
    test_model_connection_payload_with_connection(payload, None)
}

pub(crate) fn test_model_connection_payload_with_connection(
    payload: ModelTestRequest,
    connection: Option<&Connection>,
) -> Result<ModelTestResult, String> {
    match payload.provider.as_str() {
        "todo" => Ok(ModelTestResult {
            provider: "todo".into(),
            success: true,
            status_code: 0,
            message: "MiniMax M3 语义 Todo 边界已登记；此测试不发起实际 Todo 生成调用".into(),
            response_excerpt: "semantic_artifacts(type='todo_extraction')".into(),
        }),
        "asr" => test_asr_provider(connection, &payload.settings),
        other => Err(format!("不支持的模型测试类型: {other}")),
    }
}

#[tauri::command]
pub(crate) fn test_model_connection(
    payload: ModelTestRequest,
    state: tauri::State<'_, AppState>,
) -> Result<ModelTestResult, String> {
    let connection = open_connection(&state.db_path)?;
    test_model_connection_payload_with_connection(payload, Some(&connection))
}
