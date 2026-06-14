export type TodoStatus = "open" | "in_progress" | "done" | "dismissed";
export type SessionStatus =
  | "collecting"
  | "idle_waiting"
  | "ready_for_extraction"
  | "extracted"
  | "failed";
export type ProviderType = "semantic_m3";
export type AsrProviderType = "cloud_volc" | "local_whisperkit";
export type SpeakerProviderType = "local_speakerkit";
export type SemanticProviderType = "minimax_m3";
export type EmbeddingProviderType = "reserved";
export type ExportProviderType = "local_file";

export interface SettingsState {
  recordEnabled: boolean;
  language: string;
  chunkSeconds: number;
  idleTriggerSeconds: number;
  providerMode: "cloud" | "local";
  asrProviderType: AsrProviderType;
  speakerProviderType: SpeakerProviderType;
  todoProviderType: ProviderType;
  semanticProviderType: SemanticProviderType;
  embeddingProviderType: EmbeddingProviderType;
  exportProviderType: ExportProviderType;
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

export interface TodoItem {
  id: string;
  title: string;
  note: string;
  status: TodoStatus;
  createdAt: string;
  conversationSessionId: string;
  sourceText: string;
  owner: string;
  dueAt: string;
  priority: "low" | "medium" | "high";
  sourceSpanRefs: string[];
  candidateId: string;
}

export interface TodoCandidateItem {
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

export interface SessionItem {
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

export interface RuntimeStatus {
  runtimeLabel: string;
  currentSessionStatus: SessionStatus;
  lastSliceAt: string;
  lastExtractionAt: string;
  lastExtractionSummary: string;
}

export interface TranscriptAudio {
  id: string;
  fileName: string;
  durationMs: number;
  status: string;
  provider: string;
  modelName: string;
  offlineAvailable: boolean;
}

export interface SpeakerItem {
  id: string;
  label: string;
  displayName: string;
  color: string;
  segmentCount: number;
  corrected: boolean;
}

export interface TranscriptSegment {
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

export interface TranscriptJob {
  id: string;
  audioSegmentId: string;
  status: "queued" | "running" | "succeeded" | "failed" | "retrying";
  retryCount: number;
  maxRetryCount: number;
  errorMessage: string;
  provider: string;
  modelName: string;
}

export interface LocalModelStatus {
  provider: string;
  modelName: string;
  cacheDir: string;
  downloadStatus: "not_started" | "downloading" | "available" | "failed";
  downloadProgress: number;
  offlineAvailable: boolean;
  deviceRecommendation: string;
}

export interface TranscriptReview {
  audio: TranscriptAudio;
  segments: TranscriptSegment[];
  speakers: SpeakerItem[];
  jobs: TranscriptJob[];
  modelStatus: LocalModelStatus;
}

export interface TranscriptRevision {
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

export interface CorrectionPattern {
  id: string;
  phrase: string;
  replacement: string;
  patternType: string;
  scope: string;
  confidence: number;
  enabled: boolean;
}

export interface DeletedCorrectionPattern {
  deletedId: string;
}

export interface SemanticArtifact {
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

export interface ModelInvocation {
  id: string;
  provider: string;
  modelName: string;
  capability: string;
  status: "pending" | "running" | "succeeded" | "failed";
  requestSummary: string;
  responseSummary: string;
  errorMessage: string;
}

export interface RecordingType {
  value: string;
  label: string;
  templateId: string;
  confidence: number;
}

export interface SummaryArtifact {
  title: string;
  basis: string;
  bullets: string[];
  sourceSegmentIds: string[];
}

export interface MeetingMinutes {
  templateId: string;
  decisions: string[];
  risks: string[];
  openQuestions: string[];
  sourceSegmentIds: string[];
}

export interface TodoCandidate {
  title: string;
  detail: string;
  owner: string;
  priority: string;
  confidence: number;
  sourceSegmentIds: string[];
}

export interface SemanticWorkbench {
  sessionId: string;
  recordingType: RecordingType;
  revisions: TranscriptRevision[];
  correctionPatterns: CorrectionPattern[];
  summary: SummaryArtifact;
  meetingMinutes: MeetingMinutes;
  todoCandidates: TodoCandidate[];
  artifacts: SemanticArtifact[];
  modelInvocations: ModelInvocation[];
}
