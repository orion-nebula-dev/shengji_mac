use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpeakerDto {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) display_name: String,
    pub(crate) color: String,
    pub(crate) segment_count: i64,
    pub(crate) corrected: bool,
}
