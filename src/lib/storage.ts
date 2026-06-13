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
    const legacyTodoProviderType = String(
      (mergedSettings as SettingsState & { todoProviderType: string }).todoProviderType,
    );
    if (legacyTodoProviderType.trim().length === 0) {
      mergedSettings.todoProviderType = "semantic_m3";
    }
    if (legacyTodoProviderType === "embedded_local") {
      mergedSettings.todoProviderType = "legacy_local_llm";
    }
    if (
      mergedSettings.providerMode === "cloud" &&
      mergedSettings.asrProviderType === "cloud_volc" &&
      mergedSettings.todoProviderType === "cloud"
    ) {
      mergedSettings.providerMode = "local";
      mergedSettings.asrProviderType = "local_whisperkit";
      mergedSettings.todoProviderType = "semantic_m3";
    }
    return {
      settings: mergedSettings,
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
