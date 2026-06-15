use serde::{Deserialize, Serialize};

pub(crate) const LOCAL_ASR_PROVIDER: &str = "local_whisperkit";
pub(crate) const DEFAULT_LOCAL_ASR_MODEL: &str = "large-v3-v20240930_626MB";
pub(crate) const LOCAL_ASR_CACHE_DIR: &str =
    "~/Library/Application Support/com.soundworkbench.shengji/models/whisperkit";

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
