use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenerateExportBundleCommand {
    pub(crate) formats: Vec<String>,
    #[serde(default)]
    pub(crate) target_languages: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportItemDto {
    pub(crate) id: String,
    pub(crate) format: String,
    pub(crate) file_name: String,
    pub(crate) mime_type: String,
    pub(crate) content: String,
    pub(crate) status: String,
    pub(crate) source_span_refs: Vec<String>,
    pub(crate) error_message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShareSnapshotDto {
    pub(crate) id: String,
    pub(crate) file_name: String,
    pub(crate) title: String,
    pub(crate) html: String,
    pub(crate) source_span_refs: Vec<String>,
    pub(crate) privacy_summary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportBundleDto {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) provider: String,
    pub(crate) status: String,
    pub(crate) privacy_summary: String,
    pub(crate) items: Vec<ExportItemDto>,
    pub(crate) snapshot: Option<ShareSnapshotDto>,
}
