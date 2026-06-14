use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptRevisionDto {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) source_segment_id: String,
    pub(crate) speaker_label: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) original_text: String,
    pub(crate) revised_text: String,
    pub(crate) change_level: String,
    pub(crate) correction_type: String,
    pub(crate) reason_summary: String,
    #[serde(default = "default_revision_status")]
    pub(crate) status: String,
}

fn default_revision_status() -> String {
    "proposed".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CorrectionPatternDto {
    pub(crate) id: String,
    pub(crate) phrase: String,
    pub(crate) replacement: String,
    pub(crate) pattern_type: String,
    pub(crate) scope: String,
    pub(crate) confidence: f64,
    pub(crate) enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeletedCorrectionPatternDto {
    pub(crate) deleted_id: String,
}
