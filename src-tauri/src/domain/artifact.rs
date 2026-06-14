use serde::{Deserialize, Serialize};

use crate::domain::correction::{CorrectionPatternDto, TranscriptRevisionDto};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticArtifactDto {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) artifact_type: String,
    pub(crate) status: String,
    pub(crate) provider: String,
    pub(crate) model_name: String,
    pub(crate) schema_version: String,
    pub(crate) source_span_refs: Vec<String>,
    pub(crate) payload_json: String,
    pub(crate) error_message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelInvocationDto {
    pub(crate) id: String,
    pub(crate) provider: String,
    pub(crate) model_name: String,
    pub(crate) capability: String,
    pub(crate) status: String,
    pub(crate) request_summary: String,
    pub(crate) response_summary: String,
    pub(crate) error_message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordingTypeDto {
    pub(crate) value: String,
    pub(crate) label: String,
    pub(crate) template_id: String,
    pub(crate) confidence: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SummaryDto {
    pub(crate) title: String,
    pub(crate) basis: String,
    pub(crate) bullets: Vec<String>,
    pub(crate) source_segment_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingMinutesDto {
    pub(crate) template_id: String,
    pub(crate) decisions: Vec<String>,
    pub(crate) risks: Vec<String>,
    pub(crate) open_questions: Vec<String>,
    pub(crate) source_segment_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TodoCandidateDto {
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) owner: String,
    pub(crate) priority: String,
    pub(crate) confidence: f64,
    pub(crate) source_segment_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticWorkbenchDto {
    pub(crate) session_id: String,
    pub(crate) recording_type: RecordingTypeDto,
    pub(crate) revisions: Vec<TranscriptRevisionDto>,
    pub(crate) correction_patterns: Vec<CorrectionPatternDto>,
    pub(crate) summary: SummaryDto,
    pub(crate) meeting_minutes: MeetingMinutesDto,
    pub(crate) todo_candidates: Vec<TodoCandidateDto>,
    pub(crate) artifacts: Vec<SemanticArtifactDto>,
    pub(crate) model_invocations: Vec<ModelInvocationDto>,
}
