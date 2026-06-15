use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub(crate) struct TranscriptRecord {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) trace_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptAudioDto {
    pub(crate) id: String,
    pub(crate) file_name: String,
    pub(crate) duration_ms: i64,
    pub(crate) status: String,
    pub(crate) provider: String,
    pub(crate) model_name: String,
    pub(crate) offline_available: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptSegmentDto {
    pub(crate) id: String,
    pub(crate) audio_segment_id: String,
    pub(crate) speaker_id: String,
    pub(crate) speaker_label: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) text: String,
    pub(crate) confidence: f64,
    pub(crate) provider: String,
    pub(crate) review_status: String,
    pub(crate) review_reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptJobDto {
    pub(crate) id: String,
    pub(crate) audio_segment_id: String,
    pub(crate) status: String,
    pub(crate) retry_count: i64,
    pub(crate) max_retry_count: i64,
    pub(crate) error_message: String,
    pub(crate) provider: String,
    pub(crate) model_name: String,
}

pub(crate) type LocalModelStatusDto = crate::domain::local_asr::LocalAsrModelStatusDto;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptReviewDto {
    pub(crate) audio: TranscriptAudioDto,
    pub(crate) segments: Vec<TranscriptSegmentDto>,
    pub(crate) speakers: Vec<crate::domain::speaker::SpeakerDto>,
    pub(crate) jobs: Vec<TranscriptJobDto>,
    pub(crate) model_status: LocalModelStatusDto,
}
