use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeStatusDto {
    pub(crate) runtime_label: String,
    pub(crate) current_session_status: String,
    pub(crate) last_slice_at: String,
    pub(crate) last_extraction_at: String,
    pub(crate) last_extraction_summary: String,
}
