use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TodoDto {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) note: String,
    pub(crate) status: String,
    pub(crate) created_at: String,
    pub(crate) conversation_session_id: String,
    pub(crate) source_text: String,
}
