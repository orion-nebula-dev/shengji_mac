import type { RuntimeStatus, SessionItem, SettingsState, TodoItem } from "../types";

export const defaultSettings: SettingsState = {
  recordEnabled: true,
  language: "zh-CN",
  chunkSeconds: 30,
  idleTriggerSeconds: 20,
  providerMode: "cloud",
  asrSubmitUrl: "https://api.example.com/asr/submit",
  asrQueryUrl: "https://api.example.com/asr/query",
  asrResourceId: "volc.seedasr.auc",
  asrModelName: "bigmodel",
  asrApiKeyMasked: "sk-asr-****",
  todoBaseUrl: "https://api.example.com/todo",
  todoModelName: "todo-model-v1",
  todoApiKeyMasked: "sk-todo-****",
};

export const defaultTodos: TodoItem[] = [
  {
    id: "todo_001",
    title: "给客户发送报价",
    note: "今天下午 4 点前发送最新报价，并确认税率口径。",
    status: "pending",
    createdAt: "2026-04-12 10:20",
    conversationSessionId: "session_001",
    sourceText: "下午给客户发送报价，然后确认税率口径。",
  },
  {
    id: "todo_002",
    title: "明早和小王确认排期",
    note: "优先确认录音模块联调时间和测试窗口。",
    status: "pending",
    createdAt: "2026-04-12 10:20",
    conversationSessionId: "session_001",
    sourceText: "明天上午和小王确认排期。",
  },
  {
    id: "todo_003",
    title: "补充转写失败重试策略",
    note: "新增失败任务重试和错误提示。",
    status: "completed",
    createdAt: "2026-04-11 18:40",
    conversationSessionId: "session_002",
    sourceText: "录音模块异常重试补一下。",
  },
];

export const defaultSessions: SessionItem[] = [
  {
    id: "session_001",
    mergedText:
      "下午给客户发送报价，然后确认税率口径。明天上午和小王确认排期。",
    startedAt: "2026-04-12 10:19",
    endedAt: "2026-04-12 10:20",
    triggerReason: "20 秒无有效录音",
    extractionStatus: "success",
    transcriptCount: 2,
    relatedTodoIds: ["todo_001", "todo_002"],
  },
  {
    id: "session_002",
    mergedText: "录音模块异常重试补一下，顺便看下日志链路。",
    startedAt: "2026-04-11 18:37",
    endedAt: "2026-04-11 18:40",
    triggerReason: "手动刷新",
    extractionStatus: "failed",
    transcriptCount: 1,
    relatedTodoIds: ["todo_003"],
  },
];

export const defaultRuntime: RuntimeStatus = {
  runtimeLabel: "录音中",
  currentSessionStatus: "idle_waiting",
  lastSliceAt: "2026-04-12 10:20:00",
  lastExtractionAt: "2026-04-12 10:20:23",
  lastExtractionSummary: "最近一次会话提取出 2 条 Todo",
};
