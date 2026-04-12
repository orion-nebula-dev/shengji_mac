export type TodoStatus = "pending" | "completed";
export type SessionStatus =
  | "collecting"
  | "idle_waiting"
  | "ready_for_extraction"
  | "extracted"
  | "failed";
export type ProviderType = "cloud" | "embedded_local";
export type LocalRuntimeStatus = "not_ready" | "starting" | "ready" | "failed";

export interface SettingsState {
  recordEnabled: boolean;
  language: string;
  chunkSeconds: number;
  idleTriggerSeconds: number;
  providerMode: "cloud" | "local";
  asrProviderType: "cloud";
  todoProviderType: ProviderType;
  asrSubmitUrl: string;
  asrQueryUrl: string;
  asrResourceId: string;
  asrModelName: string;
  asrApiKeyMasked: string;
  todoBaseUrl: string;
  todoModelName: string;
  todoApiKeyMasked: string;
  localTodoModelVersion: string;
  allowCloudFallback: boolean;
  localTodoRuntimeStatus: LocalRuntimeStatus;
  localTodoLastHealthCheckAt: string;
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

export interface LocalRuntimeState {
  providerType: ProviderType;
  modelVersion: string;
  runtimeStatus: LocalRuntimeStatus;
  lastHealthCheckAt: string;
  fallbackEnabled: boolean;
  message: string;
}
