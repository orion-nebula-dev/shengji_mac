export interface DesktopContext {
  runtime: string;
  platform: string;
  recorderStatus: string;
  storageStatus: string;
  modelsStatus: string;
}

export interface SettingsPayload {
  recordEnabled: boolean;
  language: string;
  chunkSeconds: number;
  idleTriggerSeconds: number;
  providerMode: "cloud" | "local";
  asrProviderType: "cloud_volc" | "local_whisperkit";
  speakerProviderType: "local_speakerkit";
  todoProviderType: "semantic_m3";
  semanticProviderType: "minimax_m3";
  embeddingProviderType: "reserved";
  exportProviderType: "local_file";
  asrSubmitUrl: string;
  asrQueryUrl: string;
  asrResourceId: string;
  asrModelName: string;
  asrApiKeyMasked: string;
  semanticBaseUrl: string;
  semanticModelName: string;
  semanticApiKeyMasked: string;
  allowCloudFallback: boolean;
}

export interface TodoPayload {
  id: string;
  title: string;
  note: string;
  status: "open" | "in_progress" | "done" | "dismissed";
  createdAt: string;
  conversationSessionId: string;
  sourceText: string;
  owner: string;
  dueAt: string;
  priority: "low" | "medium" | "high";
  sourceSpanRefs: string[];
  candidateId: string;
}

export interface TodoCandidateItemPayload {
  id: string;
  sessionId: string;
  artifactId: string;
  title: string;
  detail: string;
  owner: string;
  dueAt: string;
  priority: "low" | "medium" | "high";
  confidence: number;
  status: "proposed" | "accepted" | "dismissed" | "merged";
  sourceSpanRefs: string[];
  sourceText: string;
  todoId: string;
}

export interface UpdateTodoCandidatePayload {
  candidateId: string;
  title: string;
  detail: string;
  owner: string;
  dueAt: string;
  priority: "low" | "medium" | "high";
}

export interface AcceptTodoCandidatePayload {
  candidateId: string;
  title: string;
  detail: string;
  owner: string;
  dueAt: string;
  priority: "low" | "medium" | "high";
}

export interface SessionPayload {
  id: string;
  mergedText: string;
  startedAt: string;
  endedAt: string;
  triggerReason: string;
  extractionStatus: "success" | "failed" | "pending";
  extractionProviderUsed: string;
  extractionFallbackUsed: boolean;
  extractionFallbackReason: string;
  transcriptCount: number;
  relatedTodoIds: string[];
}

export interface RuntimeStatusPayload {
  runtimeLabel: string;
  currentSessionStatus:
    | "collecting"
    | "idle_waiting"
    | "ready_for_extraction"
    | "extracted"
    | "failed";
  lastSliceAt: string;
  lastExtractionAt: string;
  lastExtractionSummary: string;
}

export interface RecoveryTaskPayload {
  taskId: string;
  taskType: string;
  targetId: string;
  audioSegmentId: string;
  status: string;
  retryCount: number;
  maxRetryCount: number;
  errorMessage: string;
  provider: string;
  modelName: string;
  retryCommand: string;
  updatedAt: string;
}

export interface RuntimeMetricSummaryPayload {
  commandName: string;
  totalCount: number;
  successCount: number;
  failedCount: number;
  p50DurationMs: number;
  p95DurationMs: number;
  latestStatus: string;
  latestErrorMessage: string;
}

export interface RuntimeDashboardPayload {
  recoveryTasks: RecoveryTaskPayload[];
  metricSummaries: RuntimeMetricSummaryPayload[];
}

export interface TaskTimelineEventPayload {
  id: string;
  stage: string;
  title: string;
  status: string;
  timestamp: string;
  detail: string;
}

export interface SegmentTimelinePayload {
  audioSegmentId: string;
  fileName: string;
  events: TaskTimelineEventPayload[];
}

export interface ProcessingJobPayload {
  id: string;
  jobType: string;
  targetId: string;
  status: string;
  retryCount: number;
  maxRetryCount: number;
  errorMessage: string;
}

export interface TranscriptAudioPayload {
  id: string;
  fileName: string;
  durationMs: number;
  status: string;
  provider: string;
  modelName: string;
  offlineAvailable: boolean;
}

export interface SpeakerPayload {
  id: string;
  label: string;
  displayName: string;
  color: string;
  segmentCount: number;
  corrected: boolean;
}

export interface TranscriptSegmentPayload {
  id: string;
  audioSegmentId: string;
  speakerId: string;
  speakerLabel: string;
  startMs: number;
  endMs: number;
  text: string;
  confidence: number;
  provider: string;
  reviewStatus: "normal" | "flagged" | "corrected";
  reviewReason: string;
}

export interface TranscriptJobPayload {
  id: string;
  audioSegmentId: string;
  status: "queued" | "running" | "succeeded" | "failed" | "retrying";
  retryCount: number;
  maxRetryCount: number;
  errorMessage: string;
  provider: string;
  modelName: string;
}

export interface LocalModelStatusPayload {
  provider: string;
  modelName: string;
  cacheDir: string;
  downloadStatus: "not_started" | "downloading" | "available" | "failed";
  downloadProgress: number;
  offlineAvailable: boolean;
  deviceRecommendation: string;
}

export interface TranscriptReviewPayload {
  audio: TranscriptAudioPayload;
  segments: TranscriptSegmentPayload[];
  speakers: SpeakerPayload[];
  jobs: TranscriptJobPayload[];
  modelStatus: LocalModelStatusPayload;
}

export interface TranscriptRevisionPayload {
  id: string;
  sessionId: string;
  sourceSegmentId: string;
  speakerLabel: string;
  startMs: number;
  endMs: number;
  originalText: string;
  revisedText: string;
  changeLevel: "none" | "punctuation" | "wording" | "meaning_affecting";
  correctionType: string;
  reasonSummary: string;
  status: "proposed" | "rejected";
}

export interface CorrectionPatternPayload {
  id: string;
  phrase: string;
  replacement: string;
  patternType: string;
  scope: string;
  confidence: number;
  enabled: boolean;
}

export interface DeletedCorrectionPatternPayload {
  deletedId: string;
}

export interface SemanticArtifactPayload {
  id: string;
  sessionId: string;
  artifactType:
    | "transcript_revision"
    | "recording_type"
    | "summary"
    | "meeting_minutes"
    | "todo_extraction"
    | "mind_map"
    | "moment"
    | "deep_research"
    | "translation";
  status: "pending" | "running" | "succeeded" | "failed";
  provider: string;
  modelName: string;
  schemaVersion: string;
  sourceSpanRefs: string[];
  payloadJson: string;
  errorMessage: string;
}

export interface ModelInvocationPayload {
  id: string;
  provider: string;
  modelName: string;
  capability: string;
  status: "pending" | "running" | "succeeded" | "failed";
  requestSummary: string;
  responseSummary: string;
  errorMessage: string;
}

export interface RecordingTypePayload {
  value: string;
  label: string;
  templateId: string;
  confidence: number;
}

export interface SummaryArtifactPayload {
  title: string;
  basis: string;
  bullets: string[];
  sourceSegmentIds: string[];
}

export interface MeetingMinutesPayload {
  templateId: string;
  decisions: string[];
  risks: string[];
  openQuestions: string[];
  sourceSegmentIds: string[];
}

export interface TodoCandidatePayload {
  title: string;
  detail: string;
  owner: string;
  priority: string;
  confidence: number;
  sourceSegmentIds: string[];
}

export interface TranscriptTranslationPayload {
  sourceSegmentId: string;
  speakerLabel: string;
  startMs: number;
  endMs: number;
  originalText: string;
  translatedText: string;
}

export interface SummaryTranslationPayload {
  sourceArtifactType: string;
  originalTitle: string;
  translatedTitle: string;
  originalBasis: string;
  translatedBasis: string;
  translatedBullets: string[];
}

export interface TranslationArtifactPayload {
  targetLanguage: string;
  transcriptTranslations: TranscriptTranslationPayload[];
  summaryTranslation: SummaryTranslationPayload;
  sourceSpanRefs: string[];
}

export interface MomentArtifactPayload {
  id: string;
  title: string;
  momentType: string;
  summary: string;
  importance: number;
  startMs: number;
  endMs: number;
  sourceSpanRefs: string[];
  actionHint: string;
}

export interface DeepResearchDraftPayload {
  id: string;
  question: string;
  background: string;
  hypotheses: string[];
  searchDirections: string[];
  nextSteps: string[];
  sourceSpanRefs: string[];
  convertedTodoId: string;
  mindMapNodeId: string;
}

export interface MindMapNodePayload {
  id: string;
  label: string;
  kind: string;
  note: string;
  sourceSpanRefs: string[];
  collapsed: boolean;
}

export interface MindMapEdgePayload {
  id: string;
  from: string;
  to: string;
  label: string;
}

export interface MindMapArtifactPayload {
  root: string;
  nodes: MindMapNodePayload[];
  edges: MindMapEdgePayload[];
  summary: string;
  sourceSpans: string[];
  edited: boolean;
  version: number;
  parentArtifactId: string;
}

export interface UpdateMindMapNodePayload {
  artifactId: string;
  nodeId: string;
  label: string;
  note: string;
}

export interface ToggleMindMapNodePayload {
  artifactId: string;
  nodeId: string;
  collapsed: boolean;
}

export interface MindMapExportPayload {
  format: "markdown" | "json";
  fileName: string;
  content: string;
}

export interface GenerateExportBundlePayload {
  formats: Array<"markdown" | "srt" | "json" | "snapshot">;
  targetLanguages?: string[];
}

export interface ExportItemPayload {
  id: string;
  format: string;
  fileName: string;
  mimeType: string;
  content: string;
  status: "pending" | "running" | "succeeded" | "failed";
  sourceSpanRefs: string[];
  errorMessage: string;
}

export interface ShareSnapshotPayload {
  id: string;
  fileName: string;
  title: string;
  html: string;
  sourceSpanRefs: string[];
  privacySummary: string;
}

export interface ExportBundlePayload {
  id: string;
  sessionId: string;
  provider: string;
  status: "pending" | "running" | "succeeded" | "failed";
  privacySummary: string;
  items: ExportItemPayload[];
  snapshot: ShareSnapshotPayload | null;
}

export interface StartResearchFromSegmentPayload {
  segmentId: string;
  question: string;
}

export interface GenerateTranslationPayload {
  targetLanguage: string;
}

export interface ConvertResearchToTodoPayload {
  artifactId: string;
  researchId: string;
}

export interface AddResearchToMindMapPayload {
  artifactId: string;
  researchId: string;
}

export interface SemanticWorkbenchPayload {
  sessionId: string;
  recordingType: RecordingTypePayload;
  revisions: TranscriptRevisionPayload[];
  correctionPatterns: CorrectionPatternPayload[];
  summary: SummaryArtifactPayload;
  meetingMinutes: MeetingMinutesPayload;
  todoCandidates: TodoCandidatePayload[];
  translations: TranslationArtifactPayload[];
  mindMap: MindMapArtifactPayload | null;
  moments: MomentArtifactPayload[];
  deepResearch: DeepResearchDraftPayload[];
  artifacts: SemanticArtifactPayload[];
  modelInvocations: ModelInvocationPayload[];
}

export interface RecordingActionPayload {
  message: string;
  runtime: RuntimeStatusPayload;
  latestSession: SessionPayload | null;
}

export interface ProcessingActionPayload {
  message: string;
  runtime: RuntimeStatusPayload;
  latestSession: SessionPayload | null;
  todos: TodoPayload[];
  sessions: SessionPayload[];
}

export interface ModelTestPayload {
  provider: "asr" | "todo";
  success: boolean;
  statusCode: number;
  message: string;
  responseExcerpt: string;
}

export interface BootstrapDataPayload {
  settings: SettingsPayload;
  todos: TodoPayload[];
  sessions: SessionPayload[];
  runtime: RuntimeStatusPayload;
}

export function isTauriEnvironment(): boolean {
  return Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

export async function loadDesktopContext(): Promise<DesktopContext | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DesktopContext>("get_desktop_context");
}

export async function loadBootstrapData(): Promise<BootstrapDataPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<BootstrapDataPayload>("get_bootstrap_data");
}

export async function saveDesktopSettings(payload: SettingsPayload): Promise<SettingsPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SettingsPayload>("save_settings", { payload });
}

export async function toggleDesktopTodoStatus(todoId: string): Promise<TodoPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TodoPayload>("toggle_todo_status", { todoId });
}

export async function updateDesktopTodoStatus(
  todoId: string,
  status: TodoPayload["status"],
): Promise<TodoPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TodoPayload>("update_todo_status", { todoId, status });
}

export async function syncDesktopTodoCandidates(): Promise<TodoCandidateItemPayload[] | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TodoCandidateItemPayload[]>("sync_todo_candidates");
}

export async function listDesktopTodoCandidates(): Promise<TodoCandidateItemPayload[] | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TodoCandidateItemPayload[]>("list_todo_candidates");
}

export async function acceptDesktopTodoCandidate(
  command: AcceptTodoCandidatePayload,
): Promise<TodoPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TodoPayload>("accept_todo_candidate", { command });
}

export async function updateDesktopTodoCandidate(
  command: UpdateTodoCandidatePayload,
): Promise<TodoCandidateItemPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TodoCandidateItemPayload>("update_todo_candidate", { command });
}

export async function dismissDesktopTodoCandidate(
  candidateId: string,
): Promise<TodoCandidateItemPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TodoCandidateItemPayload>("dismiss_todo_candidate", { candidateId });
}

export async function flushDesktopSession(): Promise<SessionPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SessionPayload>("flush_current_session");
}

export async function startDesktopRecording(): Promise<RecordingActionPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<RecordingActionPayload>("start_recording");
}

export async function stopDesktopRecording(): Promise<RecordingActionPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<RecordingActionPayload>("stop_recording");
}

export async function simulateDesktopAudioSlice(
  hasEffectiveVoice: boolean,
): Promise<RecordingActionPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<RecordingActionPayload>("simulate_audio_slice", {
    hasEffectiveVoice,
  });
}

export async function testDesktopModelConnection(
  provider: "asr" | "todo",
  settings: SettingsPayload,
): Promise<ModelTestPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModelTestPayload>("test_model_connection", {
    payload: {
      provider,
      settings,
    },
  });
}

export async function processDesktopPendingJobs(): Promise<ProcessingActionPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ProcessingActionPayload>("process_pending_jobs");
}

export async function loadDesktopRuntimeDashboard(): Promise<RuntimeDashboardPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<RuntimeDashboardPayload>("get_runtime_dashboard");
}

export async function loadDesktopSegmentTimeline(
  audioSegmentId: string,
): Promise<SegmentTimelinePayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SegmentTimelinePayload>("get_segment_timeline", { audioSegmentId });
}

export async function retryDesktopProcessingJob(
  jobId: string,
): Promise<ProcessingJobPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ProcessingJobPayload>("retry_processing_job", { jobId });
}

export async function loadTranscriptReview(): Promise<TranscriptReviewPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TranscriptReviewPayload>("get_transcript_review");
}

export async function importDesktopLocalAudio(
  filePath: string,
): Promise<TranscriptReviewPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TranscriptReviewPayload>("import_local_audio", { filePath });
}

export async function renameDesktopSpeaker(
  speakerId: string,
  label: string,
): Promise<SpeakerPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SpeakerPayload>("rename_speaker", { speakerId, label });
}

export async function markDesktopTranscriptSegment(
  segmentId: string,
  issueType: string,
  reason: string,
): Promise<TranscriptSegmentPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TranscriptSegmentPayload>("mark_transcript_segment", {
    segmentId,
    issueType,
    reason,
  });
}

export async function retryDesktopTranscriptJob(
  jobId: string,
): Promise<TranscriptJobPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TranscriptJobPayload>("retry_transcript_job", { jobId });
}

export async function loadSemanticWorkbench(): Promise<SemanticWorkbenchPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticWorkbenchPayload>("get_semantic_workbench");
}

export async function generateSemanticWorkbench(): Promise<SemanticWorkbenchPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticWorkbenchPayload>("generate_semantic_workbench");
}

export async function setDesktopCorrectionPatternEnabled(
  patternId: string,
  enabled: boolean,
): Promise<CorrectionPatternPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<CorrectionPatternPayload>("set_correction_pattern_enabled", {
    patternId,
    enabled,
  });
}

export async function deleteDesktopCorrectionPattern(
  patternId: string,
): Promise<DeletedCorrectionPatternPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DeletedCorrectionPatternPayload>("delete_correction_pattern", { patternId });
}

export async function retryDesktopSemanticArtifact(
  artifactId: string,
): Promise<SemanticArtifactPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticArtifactPayload>("retry_semantic_artifact", { artifactId });
}

export async function rejectDesktopTranscriptRevision(
  revisionId: string,
): Promise<TranscriptRevisionPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TranscriptRevisionPayload>("reject_transcript_revision", { revisionId });
}

export async function generateDesktopMindMap(): Promise<SemanticArtifactPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticArtifactPayload>("generate_mind_map");
}

export async function updateDesktopMindMapNode(
  command: UpdateMindMapNodePayload,
): Promise<SemanticArtifactPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticArtifactPayload>("update_mind_map_node", { command });
}

export async function toggleDesktopMindMapNode(
  command: ToggleMindMapNodePayload,
): Promise<SemanticArtifactPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticArtifactPayload>("toggle_mind_map_node", { command });
}

export async function exportDesktopMindMap(
  artifactId: string,
  format: "markdown" | "json",
): Promise<MindMapExportPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MindMapExportPayload>("export_mind_map", { artifactId, format });
}

export async function generateDesktopExportBundle(
  command: GenerateExportBundlePayload,
): Promise<ExportBundlePayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ExportBundlePayload>("generate_export_bundle", { command });
}

export async function generateDesktopValueDiscovery(): Promise<SemanticArtifactPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticArtifactPayload>("generate_value_discovery");
}

export async function generateDesktopTranslation(
  command: GenerateTranslationPayload,
): Promise<SemanticArtifactPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticArtifactPayload>("generate_translation", { command });
}

export async function startDesktopResearchFromSegment(
  command: StartResearchFromSegmentPayload,
): Promise<SemanticArtifactPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticArtifactPayload>("start_research_from_segment", { command });
}

export async function convertDesktopResearchToTodo(
  command: ConvertResearchToTodoPayload,
): Promise<TodoPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<TodoPayload>("convert_research_to_todo", { command });
}

export async function addDesktopResearchToMindMap(
  command: AddResearchToMindMapPayload,
): Promise<SemanticArtifactPayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SemanticArtifactPayload>("add_research_to_mind_map", { command });
}
