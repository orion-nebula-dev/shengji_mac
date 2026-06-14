use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionDto {
    pub(crate) id: String,
    pub(crate) merged_text: String,
    pub(crate) started_at: String,
    pub(crate) ended_at: String,
    pub(crate) trigger_reason: String,
    pub(crate) extraction_status: String,
    pub(crate) extraction_provider_used: String,
    pub(crate) extraction_fallback_used: bool,
    pub(crate) extraction_fallback_reason: String,
    pub(crate) transcript_count: i64,
    pub(crate) related_todo_ids: Vec<String>,
}
