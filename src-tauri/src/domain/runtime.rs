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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryTaskDto {
    pub(crate) task_id: String,
    pub(crate) task_type: String,
    pub(crate) target_id: String,
    pub(crate) audio_segment_id: String,
    pub(crate) status: String,
    pub(crate) retry_count: i64,
    pub(crate) max_retry_count: i64,
    pub(crate) error_message: String,
    pub(crate) provider: String,
    pub(crate) model_name: String,
    pub(crate) retry_command: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeMetricSummaryDto {
    pub(crate) command_name: String,
    pub(crate) total_count: i64,
    pub(crate) success_count: i64,
    pub(crate) failed_count: i64,
    pub(crate) p50_duration_ms: i64,
    pub(crate) p95_duration_ms: i64,
    pub(crate) latest_status: String,
    pub(crate) latest_error_message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeDashboardDto {
    pub(crate) recovery_tasks: Vec<RecoveryTaskDto>,
    pub(crate) metric_summaries: Vec<RuntimeMetricSummaryDto>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskTimelineEventDto {
    pub(crate) id: String,
    pub(crate) stage: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) timestamp: String,
    pub(crate) detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SegmentTimelineDto {
    pub(crate) audio_segment_id: String,
    pub(crate) file_name: String,
    pub(crate) events: Vec<TaskTimelineEventDto>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcessingJobDto {
    pub(crate) id: String,
    pub(crate) job_type: String,
    pub(crate) target_id: String,
    pub(crate) status: String,
    pub(crate) retry_count: i64,
    pub(crate) max_retry_count: i64,
    pub(crate) error_message: String,
}
