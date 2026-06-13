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
  asrProviderType: "cloud" | "local";
  todoProviderType: "cloud" | "embedded_local";
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
  localTodoRuntimeStatus: "not_ready" | "starting" | "ready" | "failed";
  localTodoLastHealthCheckAt: string;
}

export interface TodoPayload {
  id: string;
  title: string;
  note: string;
  status: "pending" | "completed";
  createdAt: string;
  conversationSessionId: string;
  sourceText: string;
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

export interface LocalRuntimePayload {
  providerType: "cloud" | "embedded_local";
  modelVersion: string;
  runtimeStatus: "not_ready" | "starting" | "ready" | "failed";
  lastHealthCheckAt: string;
  fallbackEnabled: boolean;
  message: string;
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

export async function getLocalTodoRuntimeStatus(): Promise<LocalRuntimePayload | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<LocalRuntimePayload>("get_local_todo_runtime_status");
}
