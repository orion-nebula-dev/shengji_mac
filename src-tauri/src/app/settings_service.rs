use rusqlite::{params, Connection};

use crate::{
    normalize_asr_provider_type, SettingsDto, DEFAULT_SEMANTIC_PROVIDER_TYPE,
    DEFAULT_TODO_PROVIDER_TYPE,
};

pub(crate) fn load_settings(connection: &Connection) -> Result<SettingsDto, String> {
    connection
        .query_row(
            r#"
      SELECT
        record_enabled,
        language,
        chunk_seconds,
        idle_trigger_seconds,
        provider_mode,
        asr_provider_type,
        speaker_provider_type,
        todo_provider_type,
        semantic_provider_type,
        embedding_provider_type,
        export_provider_type,
        asr_submit_url,
        asr_query_url,
        asr_resource_id,
        asr_model_name,
        asr_api_key_ref,
        semantic_base_url,
        semantic_model_name,
        semantic_api_key_ref,
        allow_cloud_fallback
      FROM app_settings
      WHERE id = 'default'
      "#,
            [],
            |row| {
                Ok(SettingsDto {
                    record_enabled: row.get::<_, i64>(0)? == 1,
                    language: row.get(1)?,
                    chunk_seconds: row.get(2)?,
                    idle_trigger_seconds: row.get(3)?,
                    provider_mode: row.get(4)?,
                    asr_provider_type: normalize_asr_provider_type(&row.get::<_, String>(5)?),
                    speaker_provider_type: row.get(6)?,
                    todo_provider_type: DEFAULT_TODO_PROVIDER_TYPE.to_string(),
                    semantic_provider_type: DEFAULT_SEMANTIC_PROVIDER_TYPE.to_string(),
                    embedding_provider_type: row.get(9)?,
                    export_provider_type: row.get(10)?,
                    asr_submit_url: row.get(11)?,
                    asr_query_url: row.get(12)?,
                    asr_resource_id: row.get(13)?,
                    asr_model_name: row.get(14)?,
                    asr_api_key_masked: row.get(15)?,
                    semantic_base_url: row.get(16)?,
                    semantic_model_name: row.get(17)?,
                    semantic_api_key_masked: row.get(18)?,
                    allow_cloud_fallback: row.get::<_, i64>(19)? == 1,
                })
            },
        )
        .map_err(|error| format!("读取设置失败: {error}"))
}

pub(crate) fn save_settings(connection: &Connection, payload: &SettingsDto) -> Result<(), String> {
    connection
        .execute(
            r#"
      UPDATE app_settings
      SET
        record_enabled = ?1,
        language = ?2,
        chunk_seconds = ?3,
        idle_trigger_seconds = ?4,
        provider_mode = ?5,
        asr_provider_type = ?6,
        speaker_provider_type = ?7,
        todo_provider_type = ?8,
        semantic_provider_type = ?9,
        embedding_provider_type = ?10,
        export_provider_type = ?11,
        asr_base_url = ?12,
        asr_submit_url = ?13,
        asr_query_url = ?14,
        asr_resource_id = ?15,
        asr_model_name = ?16,
        asr_api_key_ref = ?17,
        semantic_base_url = ?18,
        semantic_model_name = ?19,
        semantic_api_key_ref = ?20,
        allow_cloud_fallback = ?21,
        updated_at = CURRENT_TIMESTAMP
      WHERE id = 'default'
      "#,
            params![
                if payload.record_enabled { 1 } else { 0 },
                payload.language.as_str(),
                payload.chunk_seconds,
                payload.idle_trigger_seconds,
                payload.provider_mode.as_str(),
                normalize_asr_provider_type(&payload.asr_provider_type),
                payload.speaker_provider_type.as_str(),
                DEFAULT_TODO_PROVIDER_TYPE,
                DEFAULT_SEMANTIC_PROVIDER_TYPE,
                payload.embedding_provider_type.as_str(),
                payload.export_provider_type.as_str(),
                payload.asr_submit_url.as_str(),
                payload.asr_submit_url.as_str(),
                payload.asr_query_url.as_str(),
                payload.asr_resource_id.as_str(),
                payload.asr_model_name.as_str(),
                payload.asr_api_key_masked.as_str(),
                payload.semantic_base_url.as_str(),
                payload.semantic_model_name.as_str(),
                payload.semantic_api_key_masked.as_str(),
                if payload.allow_cloud_fallback { 1 } else { 0 },
            ],
        )
        .map_err(|error| format!("保存设置失败: {error}"))?;
    Ok(())
}

pub(crate) fn set_record_enabled(connection: &Connection, enabled: bool) -> Result<(), String> {
    connection
        .execute(
            "UPDATE app_settings SET record_enabled = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 'default'",
            params![if enabled { 1 } else { 0 }],
        )
        .map_err(|error| format!("更新录音状态失败: {error}"))?;
    Ok(())
}
