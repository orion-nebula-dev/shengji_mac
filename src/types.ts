export type TodoStatus = "pending" | "completed";
export type SessionStatus =
  | "collecting"
  | "idle_waiting"
  | "ready_for_extraction"
  | "extracted"
  | "failed";

export interface SettingsState {
  recordEnabled: boolean;
  language: string;
  chunkSeconds: number;
  idleTriggerSeconds: number;
  providerMode: "cloud" | "local";
  asrSubmitUrl: string;
  asrQueryUrl: string;
  asrResourceId: string;
  asrModelName: string;
  asrApiKeyMasked: string;
  todoBaseUrl: string;
  todoModelName: string;
  todoApiKeyMasked: string;
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
