import { defaultRuntime, defaultSessions, defaultSettings, defaultTodos } from "../data/mock";
import type { RuntimeStatus, SessionItem, SettingsState, TodoItem, TodoStatus } from "../types";

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
    if (
      legacyAsrProviderType === "local" ||
      legacyAsrProviderType === "cloud" ||
      legacyAsrProviderType !== "local_whisperkit"
    ) {
      mergedSettings.asrProviderType = "local_whisperkit";
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
    const normalizedTodos = (parsed.todos ?? defaultState.todos).map((todo) => {
      const legacyStatus = String((todo as TodoItem & { status: string }).status);
      const status: TodoStatus =
        legacyStatus === "completed" || legacyStatus === "done"
          ? "done"
          : legacyStatus === "in_progress"
            ? "in_progress"
            : legacyStatus === "dismissed"
              ? "dismissed"
              : "open";
      return {
        ...todo,
        status,
        owner: todo.owner ?? "",
        dueAt: todo.dueAt ?? "",
        priority: todo.priority ?? "medium",
        sourceSpanRefs: todo.sourceSpanRefs ?? [],
        candidateId: todo.candidateId ?? "",
      };
    });
    const normalizedRuntime: RuntimeStatus = {
      ...defaultState.runtime,
      ...(parsed.runtime ?? {}),
    };
    if (
      normalizedRuntime.currentSessionStatus === "idle_waiting" &&
      normalizedRuntime.runtimeLabel === "录音中"
    ) {
      normalizedRuntime.currentSessionStatus = "collecting";
    }
    return {
      settings: sanitizedSettings,
      todos: normalizedTodos,
      sessions: parsed.sessions ?? defaultState.sessions,
      runtime: normalizedRuntime,
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
