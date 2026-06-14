use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsDto {
    pub(crate) record_enabled: bool,
    pub(crate) language: String,
    pub(crate) chunk_seconds: i64,
    pub(crate) idle_trigger_seconds: i64,
    pub(crate) provider_mode: String,
    pub(crate) asr_provider_type: String,
    pub(crate) speaker_provider_type: String,
    pub(crate) todo_provider_type: String,
    pub(crate) semantic_provider_type: String,
    pub(crate) embedding_provider_type: String,
    pub(crate) export_provider_type: String,
    pub(crate) asr_submit_url: String,
    pub(crate) asr_query_url: String,
    pub(crate) asr_resource_id: String,
    pub(crate) asr_model_name: String,
    pub(crate) asr_api_key_masked: String,
    pub(crate) semantic_base_url: String,
    pub(crate) semantic_model_name: String,
    pub(crate) semantic_api_key_masked: String,
    pub(crate) allow_cloud_fallback: bool,
}
