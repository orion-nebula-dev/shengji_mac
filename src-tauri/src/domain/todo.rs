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
    pub(crate) owner: String,
    pub(crate) due_at: String,
    pub(crate) priority: String,
    pub(crate) source_span_refs: Vec<String>,
    pub(crate) candidate_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TodoCandidateDto {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) artifact_id: String,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) owner: String,
    pub(crate) due_at: String,
    pub(crate) priority: String,
    pub(crate) confidence: f64,
    pub(crate) status: String,
    pub(crate) source_span_refs: Vec<String>,
    pub(crate) source_text: String,
    pub(crate) todo_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcceptTodoCandidateCommand {
    pub(crate) candidate_id: String,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) owner: String,
    pub(crate) due_at: String,
    pub(crate) priority: String,
}
