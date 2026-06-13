import { defaultRuntime, defaultSessions, defaultSettings, defaultTodos } from "../data/mock";
import type { RuntimeStatus, SessionItem, SettingsState, TodoItem } from "../types";

const STORAGE_KEY = "smart-todo-desktop-state";

export interface PersistedState {
  settings: SettingsState;
  todos: TodoItem[];
  sessions: SessionItem[];
  runtime: RuntimeStatus;
}

const defaultState: PersistedState = {
  settings: defaultSettings,
  todos: defaultTodos,
  sessions: defaultSessions,
  runtime: defaultRuntime,
};

export function loadState(): PersistedState {
  const raw = window.localStorage.getItem(STORAGE_KEY);

  if (!raw) {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(defaultState));
    return defaultState;
  }

  try {
    const parsed = JSON.parse(raw) as Partial<PersistedState>;
    const mergedSettings = {
      ...defaultState.settings,
      ...(parsed.settings ?? {}),
    };
    const legacyAsrProviderType = String(
      (mergedSettings as SettingsState & { asrProviderType: string }).asrProviderType,
    );
    if (legacyAsrProviderType === "local" || legacyAsrProviderType.trim().length === 0) {
      mergedSettings.asrProviderType = "local_whisperkit";
    }
    if (legacyAsrProviderType === "cloud") {
      mergedSettings.asrProviderType = "cloud_volc";
    }
    mergedSettings.todoProviderType = "semantic_m3";
    const sanitizedSettings: SettingsState = {
      recordEnabled: mergedSettings.recordEnabled,
      language: mergedSettings.language,
      chunkSeconds: mergedSettings.chunkSeconds,
      idleTriggerSeconds: mergedSettings.idleTriggerSeconds,
      providerMode: mergedSettings.providerMode,
      asrProviderType: mergedSettings.asrProviderType,
      speakerProviderType: mergedSettings.speakerProviderType,
      todoProviderType: "semantic_m3",
      semanticProviderType: mergedSettings.semanticProviderType,
      embeddingProviderType: mergedSettings.embeddingProviderType,
      exportProviderType: mergedSettings.exportProviderType,
      asrSubmitUrl: mergedSettings.asrSubmitUrl,
      asrQueryUrl: mergedSettings.asrQueryUrl,
      asrResourceId: mergedSettings.asrResourceId,
      asrModelName: mergedSettings.asrModelName,
      asrApiKeyMasked: mergedSettings.asrApiKeyMasked,
      semanticBaseUrl: mergedSettings.semanticBaseUrl,
      semanticModelName: mergedSettings.semanticModelName,
      semanticApiKeyMasked: mergedSettings.semanticApiKeyMasked,
      allowCloudFallback: mergedSettings.allowCloudFallback,
    };
    return {
      settings: sanitizedSettings,
      todos: parsed.todos ?? defaultState.todos,
      sessions: parsed.sessions ?? defaultState.sessions,
      runtime: {
        ...defaultState.runtime,
        ...(parsed.runtime ?? {}),
      },
    };
  } catch {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(defaultState));
    return defaultState;
  }
}

export function saveState(nextState: PersistedState) {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(nextState));
}

export function getDefaultState(): PersistedState {
  return defaultState;
}
