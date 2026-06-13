export type TodoStatus = "pending" | "completed";
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
