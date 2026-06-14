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
pub(crate) struct GenerateTranslationCommand {
    pub(crate) target_language: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptTranslationDto {
    pub(crate) source_segment_id: String,
    pub(crate) speaker_label: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) original_text: String,
    pub(crate) translated_text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SummaryTranslationDto {
    pub(crate) source_artifact_type: String,
    pub(crate) original_title: String,
    pub(crate) translated_title: String,
    pub(crate) original_basis: String,
    pub(crate) translated_basis: String,
    pub(crate) translated_bullets: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationArtifactDto {
    pub(crate) target_language: String,
    pub(crate) transcript_translations: Vec<TranscriptTranslationDto>,
    pub(crate) summary_translation: SummaryTranslationDto,
    pub(crate) source_span_refs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MomentDto {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) moment_type: String,
    pub(crate) summary: String,
    pub(crate) importance: f64,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) source_span_refs: Vec<String>,
    pub(crate) action_hint: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeepResearchDraftDto {
    pub(crate) id: String,
    pub(crate) question: String,
    pub(crate) background: String,
    pub(crate) hypotheses: Vec<String>,
    pub(crate) search_directions: Vec<String>,
    pub(crate) next_steps: Vec<String>,
    pub(crate) source_span_refs: Vec<String>,
    pub(crate) converted_todo_id: String,
    pub(crate) mind_map_node_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MindMapNodeDto {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) kind: String,
    pub(crate) note: String,
    pub(crate) source_span_refs: Vec<String>,
    pub(crate) collapsed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MindMapEdgeDto {
    pub(crate) id: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MindMapDto {
    pub(crate) root: String,
    pub(crate) nodes: Vec<MindMapNodeDto>,
    pub(crate) edges: Vec<MindMapEdgeDto>,
    pub(crate) summary: String,
    pub(crate) source_spans: Vec<String>,
    pub(crate) edited: bool,
    pub(crate) version: i64,
    pub(crate) parent_artifact_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateMindMapNodeCommand {
    pub(crate) artifact_id: String,
    pub(crate) node_id: String,
    pub(crate) label: String,
    pub(crate) note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToggleMindMapNodeCommand {
    pub(crate) artifact_id: String,
    pub(crate) node_id: String,
    pub(crate) collapsed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MindMapExportDto {
    pub(crate) format: String,
    pub(crate) file_name: String,
    pub(crate) content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartResearchFromSegmentCommand {
    pub(crate) segment_id: String,
    pub(crate) question: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConvertResearchToTodoCommand {
    pub(crate) artifact_id: String,
    pub(crate) research_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddResearchToMindMapCommand {
    pub(crate) artifact_id: String,
    pub(crate) research_id: String,
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
    pub(crate) translations: Vec<TranslationArtifactDto>,
    pub(crate) mind_map: Option<MindMapDto>,
    pub(crate) moments: Vec<MomentDto>,
    pub(crate) deep_research: Vec<DeepResearchDraftDto>,
    pub(crate) artifacts: Vec<SemanticArtifactDto>,
    pub(crate) model_invocations: Vec<ModelInvocationDto>,
}
