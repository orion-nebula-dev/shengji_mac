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
    return {
      settings: {
        ...defaultState.settings,
        ...(parsed.settings ?? {}),
      },
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
