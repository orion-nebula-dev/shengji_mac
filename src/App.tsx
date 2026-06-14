import { useEffect, useMemo, useState } from "react";
import {
  acceptDesktopTodoCandidate,
  addDesktopResearchToMindMap,
  convertDesktopResearchToTodo,
  deleteDesktopCorrectionPattern,
  dismissDesktopTodoCandidate,
  flushDesktopSession,
  exportDesktopMindMap,
  generateDesktopExportBundle,
  generateDesktopMindMap,
  generateDesktopTranslation,
  generateDesktopValueDiscovery,
  generateSemanticWorkbench,
  importDesktopLocalAudio,
  isTauriEnvironment,
  listDesktopTodoCandidates,
  loadBootstrapData,
  loadDesktopContext,
  loadSemanticWorkbench,
  loadTranscriptReview,
  markDesktopTranscriptSegment,
  processDesktopPendingJobs,
  renameDesktopSpeaker,
  rejectDesktopTranscriptRevision,
  retryDesktopSemanticArtifact,
  retryDesktopTranscriptJob,
  saveDesktopSettings,
  setDesktopCorrectionPatternEnabled,
  simulateDesktopAudioSlice,
  startDesktopRecording,
  startDesktopResearchFromSegment,
  stopDesktopRecording,
  syncDesktopTodoCandidates,
  testDesktopModelConnection,
  toggleDesktopTodoStatus,
  toggleDesktopMindMapNode,
  updateDesktopMindMapNode,
  updateDesktopTodoStatus,
} from "./lib/desktop";
import { defaultExportBundle, defaultSemanticWorkbench, defaultTodoCandidates, defaultTranscriptReview } from "./data/mock";
import { getDefaultState, loadState, saveState } from "./lib/storage";
import appIconUrl from "../src-tauri/icons/128x128.png";
import type {
  CorrectionPattern,
  DeepResearchDraft,
  ExportBundle,
  ExportItem,
  MindMapArtifact,
  MindMapExport,
  MomentArtifact,
  SemanticArtifact,
  SemanticWorkbench,
  SessionItem,
  SettingsState,
  TodoCandidateItem,
  TodoItem,
  TodoStatus,
  TranslationArtifact,
  TranscriptReview,
} from "./types";

type TabKey =
  | "overview"
  | "actions"
  | "transcript"
  | "semantic"
  | "research"
  | "mindmap"
  | "export"
  | "history"
  | "system"
  | "settings";

type OverviewPanelKey = "transcript" | "summary" | "todo" | "privacy";

const tabKeys: TabKey[] = [
  "overview",
  "actions",
  "transcript",
  "semantic",
  "research",
  "mindmap",
  "export",
  "history",
  "system",
  "settings",
];

function getHashTab(): TabKey | null {
  if (typeof window === "undefined") {
    return null;
  }

  const hashTab = window.location.hash.replace(/^#\/?/, "");
  return tabKeys.includes(hashTab as TabKey) ? (hashTab as TabKey) : null;
}

function getInitialTab(): TabKey {
  return getHashTab() ?? "overview";
}

const statusLabelMap = {
  open: "待处理",
  in_progress: "进行中",
  done: "已完成",
  dismissed: "已忽略",
} as const;

const priorityLabelMap = {
  low: "低",
  medium: "中",
  high: "高",
} as const;

const sessionStatusLabelMap = {
  collecting: "采集中",
  idle_waiting: "等待会话结束",
  ready_for_extraction: "待提取",
  extracted: "已提取",
  failed: "失败",
} as const;

const extractionStatusLabelMap = {
  success: "已完成",
  failed: "失败可重试",
  pending: "等待中",
} as const;

const exportFormatLabelMap: Record<string, string> = {
  markdown: "Markdown",
  srt: "SRT 字幕",
  json: "JSON",
  snapshot: "分享快照",
};

function getExportFormatLabel(format: string) {
  const [baseFormat, targetLanguage] = format.split("_");
  const label = exportFormatLabelMap[baseFormat] ?? format;
  return targetLanguage ? `${label} · ${targetLanguage}` : label;
}

const transcriptJobStatusLabelMap = {
  queued: "已排队",
  running: "转写中",
  succeeded: "已完成",
  failed: "失败可重试",
  retrying: "重试中",
} as const;

const appVersionLabel = "v1.1.1";
const overviewPanelLabelMap: Record<OverviewPanelKey, string> = {
  transcript: "转写",
  summary: "摘要",
  todo: "Todo",
  privacy: "隐私边界",
};

function formatDuration(ms: number) {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60)
    .toString()
    .padStart(2, "0");
  const seconds = (totalSeconds % 60).toString().padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function getFallbackReasonText(session?: SessionItem) {
  if (!session) {
    return "无";
  }

  if (!session.extractionFallbackUsed) {
    return "不适用";
  }

  return session.extractionFallbackReason || "未记录回退原因";
}

function parseMindMapArtifact(artifact: SemanticArtifact): MindMapArtifact | null {
  if (artifact.artifactType !== "mind_map" || artifact.status !== "succeeded") {
    return null;
  }

  try {
    return JSON.parse(artifact.payloadJson) as MindMapArtifact;
  } catch {
    return null;
  }
}

function parseMomentArtifact(artifact: SemanticArtifact): MomentArtifact[] {
  if (artifact.artifactType !== "moment" || artifact.status !== "succeeded") {
    return [];
  }

  try {
    return JSON.parse(artifact.payloadJson) as MomentArtifact[];
  } catch {
    return [];
  }
}

function parseResearchArtifact(artifact: SemanticArtifact): DeepResearchDraft | null {
  if (artifact.artifactType !== "deep_research" || artifact.status !== "succeeded") {
    return null;
  }

  try {
    return JSON.parse(artifact.payloadJson) as DeepResearchDraft;
  } catch {
    return null;
  }
}

function parseTranslationArtifact(artifact: SemanticArtifact): TranslationArtifact | null {
  if (artifact.artifactType !== "translation" || artifact.status !== "succeeded") {
    return null;
  }

  try {
    return JSON.parse(artifact.payloadJson) as TranslationArtifact;
  } catch {
    return null;
  }
}

function mindMapToMarkdown(mindMap: MindMapArtifact) {
  return [
    "# 语义脑图",
    "",
    `- 摘要：${mindMap.summary}`,
    `- 版本：${mindMap.version}`,
    `- 来源：${mindMap.sourceSpans.join("、") || "暂无来源"}`,
    "",
    ...mindMap.nodes.flatMap((node) => [
      `${node.id === mindMap.root ? "##" : "###"} ${node.label}`,
      node.note,
      `来源：${node.sourceSpanRefs.join("、") || "暂无来源"}`,
      "",
    ]),
  ].join("\n");
}

function createLocalTranslationArtifact(
  workbench: SemanticWorkbench,
  targetLanguage: string,
): SemanticArtifact {
  const sourceSpanRefs = workbench.revisions.map((revision) => revision.sourceSegmentId);
  const translation: TranslationArtifact = {
    targetLanguage,
    transcriptTranslations: workbench.revisions.map((revision) => ({
      sourceSegmentId: revision.sourceSegmentId,
      speakerLabel: revision.speakerLabel,
      startMs: revision.startMs,
      endMs: revision.endMs,
      originalText: revision.revisedText,
      translatedText: `[${targetLanguage}] ${revision.revisedText}`,
    })),
    summaryTranslation: {
      sourceArtifactType: "summary",
      originalTitle: workbench.summary.title,
      translatedTitle: `[${targetLanguage}] ${workbench.summary.title}`,
      originalBasis: workbench.summary.basis,
      translatedBasis: `[${targetLanguage}] ${workbench.summary.basis}`,
      translatedBullets: workbench.summary.bullets.map((bullet) => `[${targetLanguage}] ${bullet}`),
    },
    sourceSpanRefs,
  };
  return {
    id: `semantic_${workbench.sessionId}_translation_${targetLanguage.replace(/[^A-Za-z0-9_-]/g, "_")}`,
    sessionId: workbench.sessionId,
    artifactType: "translation",
    status: "succeeded",
    provider: "minimax_m3",
    modelName: "MiniMax-M3",
    schemaVersion: "v1.1",
    sourceSpanRefs,
    payloadJson: JSON.stringify(translation),
    errorMessage: "",
  };
}

function createLocalMindMapArtifact(
  workbench: SemanticWorkbench,
  mindMap: MindMapArtifact,
  edited: boolean,
): SemanticArtifact {
  const version = workbench.artifacts.filter((artifact) => artifact.artifactType === "mind_map").length + 1;
  return {
    id: `semantic_${workbench.sessionId}_mind_map_${edited ? "edited" : "generated"}_v${version}`,
    sessionId: workbench.sessionId,
    artifactType: "mind_map",
    status: "succeeded",
    provider: "minimax_m3",
    modelName: "MiniMax-M3",
    schemaVersion: "v0.8",
    sourceSpanRefs: mindMap.sourceSpans,
    payloadJson: JSON.stringify(mindMap),
    errorMessage: "",
  };
}

function createLocalValueArtifact(
  workbench: SemanticWorkbench,
  artifactType: "moment" | "deep_research",
  payload: MomentArtifact[] | DeepResearchDraft,
  sourceSpanRefs: string[],
): SemanticArtifact {
  return {
    id: `semantic_${workbench.sessionId}_${artifactType}_local_${Date.now()}`,
    sessionId: workbench.sessionId,
    artifactType,
    status: "succeeded",
    provider: "minimax_m3",
    modelName: "MiniMax-M3",
    schemaVersion: "v0.9",
    sourceSpanRefs,
    payloadJson: JSON.stringify(payload),
    errorMessage: "",
  };
}

function App() {
  const manualFlushCooldownMs = 10_000;
  const initialState = useMemo(() => loadState(), []);
  const fallbackState = useMemo(() => getDefaultState(), []);
  const [activeTab, setActiveTab] = useState<TabKey>(() => getInitialTab());
  const [overviewPanel, setOverviewPanel] = useState<OverviewPanelKey>("transcript");
  const [settings, setSettings] = useState<SettingsState>(initialState.settings);
  const [todos, setTodos] = useState<TodoItem[]>(initialState.todos);
  const [sessions, setSessions] = useState<SessionItem[]>(initialState.sessions);
  const [runtime, setRuntime] = useState(initialState.runtime);
  const [selectedTodoId, setSelectedTodoId] = useState(initialState.todos[0]?.id ?? "");
  const [filter, setFilter] = useState<"all" | TodoStatus>("all");
  const [todoCandidates, setTodoCandidates] =
    useState<TodoCandidateItem[]>(defaultTodoCandidates);
  const [keyword, setKeyword] = useState("");
  const [saveBanner, setSaveBanner] = useState("");
  const [testingProvider, setTestingProvider] = useState<"" | "asr" | "todo">("");
  const [lastManualFlushAt, setLastManualFlushAt] = useState(0);
  const [transcriptReview, setTranscriptReview] =
    useState<TranscriptReview>(defaultTranscriptReview);
  const [audioImportPath, setAudioImportPath] = useState("");
  const [selectedTranscriptSegmentId, setSelectedTranscriptSegmentId] = useState(
    defaultTranscriptReview.segments[0]?.id ?? "",
  );
  const [currentPlaybackMs, setCurrentPlaybackMs] = useState(0);
  const [speakerDrafts, setSpeakerDrafts] = useState<Record<string, string>>({});
  const [semanticWorkbench, setSemanticWorkbench] =
    useState<SemanticWorkbench>(defaultSemanticWorkbench);
  const [semanticLoading, setSemanticLoading] = useState(false);
  const [mindMapLoading, setMindMapLoading] = useState(false);
  const [selectedMindMapNodeId, setSelectedMindMapNodeId] = useState(
    defaultSemanticWorkbench.mindMap?.root ?? "",
  );
  const [mindMapDraft, setMindMapDraft] = useState({ label: "", note: "" });
  const [mindMapExport, setMindMapExport] = useState<MindMapExport | null>(null);
  const [exportBundle, setExportBundle] = useState<ExportBundle | null>(defaultExportBundle);
  const [exportLoading, setExportLoading] = useState(false);
  const [selectedExportFormat, setSelectedExportFormat] = useState("markdown");
  const [selectedTargetLanguage, setSelectedTargetLanguage] = useState("en-US");
  const [translationLoading, setTranslationLoading] = useState(false);
  const [sessionSearch, setSessionSearch] = useState("");
  const [valueDiscoveryLoading, setValueDiscoveryLoading] = useState(false);
  const [selectedResearchId, setSelectedResearchId] = useState(
    defaultSemanticWorkbench.deepResearch[0]?.id ?? "",
  );
  const [researchSegmentId, setResearchSegmentId] = useState(
    defaultSemanticWorkbench.revisions[0]?.sourceSegmentId ?? "",
  );
  const [researchQuestion, setResearchQuestion] = useState("这个片段是否值得继续研究？");
  const [desktopContext, setDesktopContext] = useState<{
    runtime: string;
    platform: string;
    recorderStatus: string;
    storageStatus: string;
    modelsStatus: string;
  } | null>(null);

  useEffect(() => {
    saveState({ settings, todos, sessions, runtime });
  }, [runtime, sessions, settings, todos]);

  useEffect(() => {
    const nextHash = `#${activeTab}`;

    if (window.location.hash !== nextHash) {
      window.history.replaceState(null, "", nextHash);
    }
  }, [activeTab]);

  useEffect(() => {
    const handleHashChange = () => {
      const hashTab = getHashTab();

      if (hashTab) {
        setActiveTab(hashTab);
      }
    };

    window.addEventListener("hashchange", handleHashChange);
    return () => window.removeEventListener("hashchange", handleHashChange);
  }, []);

  useEffect(() => {
    let cancelled = false;

    loadDesktopContext()
      .then((context) => {
        if (!cancelled) {
          setDesktopContext(context);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setDesktopContext(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const node = semanticWorkbench.mindMap?.nodes.find(
      (candidate) => candidate.id === selectedMindMapNodeId,
    );
    if (node) {
      setMindMapDraft({ label: node.label, note: node.note });
    }
  }, [selectedMindMapNodeId, semanticWorkbench.mindMap]);

  useEffect(() => {
    if (
      semanticWorkbench.deepResearch.length > 0 &&
      !semanticWorkbench.deepResearch.some((research) => research.id === selectedResearchId)
    ) {
      setSelectedResearchId(semanticWorkbench.deepResearch[0].id);
    }
    if (
      semanticWorkbench.revisions.length > 0 &&
      !semanticWorkbench.revisions.some((revision) => revision.sourceSegmentId === researchSegmentId)
    ) {
      setResearchSegmentId(semanticWorkbench.revisions[0].sourceSegmentId);
    }
  }, [researchSegmentId, selectedResearchId, semanticWorkbench.deepResearch, semanticWorkbench.revisions]);

  useEffect(() => {
    let cancelled = false;

    listDesktopTodoCandidates()
      .then((candidates) => {
        if (!cancelled && candidates) {
          setTodoCandidates(candidates);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setTodoCandidates(defaultTodoCandidates);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    loadSemanticWorkbench()
      .then((workbench) => {
        if (!workbench || cancelled) {
          return;
        }

        setSemanticWorkbench(workbench);
      })
      .catch(() => {
        if (!cancelled) {
          setSemanticWorkbench(defaultSemanticWorkbench);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    loadTranscriptReview()
      .then((review) => {
        if (!review || cancelled) {
          return;
        }

        setTranscriptReview(review);
        setSelectedTranscriptSegmentId(review.segments[0]?.id ?? "");
        setSpeakerDrafts(
          Object.fromEntries(review.speakers.map((speaker) => [speaker.id, speaker.label])),
        );
      })
      .catch(() => {
        if (!cancelled) {
          setTranscriptReview(defaultTranscriptReview);
          setSpeakerDrafts(
            Object.fromEntries(
              defaultTranscriptReview.speakers.map((speaker) => [speaker.id, speaker.label]),
            ),
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    loadBootstrapData()
      .then((payload) => {
        if (!payload || cancelled) {
          return;
        }

        setSettings(payload.settings);
        setTodos(payload.todos);
        setSessions(payload.sessions);
        setRuntime(payload.runtime);
        setSelectedTodoId(payload.todos[0]?.id ?? "");
      })
      .catch(() => {
        if (!cancelled) {
          setSettings(fallbackState.settings);
          setTodos(fallbackState.todos);
          setSessions(fallbackState.sessions);
          setRuntime(fallbackState.runtime);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [fallbackState.runtime, fallbackState.sessions, fallbackState.settings, fallbackState.todos]);

  const filteredTodos = todos.filter((todo) => {
    const matchesStatus = filter === "all" ? true : todo.status === filter;
    const matchesKeyword =
      keyword.trim().length === 0 ||
      [todo.title, todo.note, todo.owner, todo.sourceText].some((field) =>
        field.toLowerCase().includes(keyword.trim().toLowerCase()),
      );

    return matchesStatus && matchesKeyword;
  });

  const selectedTodo =
    todos.find((todo) => todo.id === selectedTodoId) ?? filteredTodos[0] ?? todos[0];
  const selectedSession = sessions.find(
    (session) => session.id === selectedTodo?.conversationSessionId,
  );

  function handleSettingsChange<K extends keyof SettingsState>(
    key: K,
    value: SettingsState[K],
  ) {
    setSettings((current) => ({
      ...current,
      [key]: value,
    }));
  }

  async function saveSettings() {
    const persisted = await saveDesktopSettings(settings).catch(() => null);

    if (!persisted) {
      setSaveBanner("设置未保存：桌面端持久化不可用，请在已安装应用中重试。");
      window.setTimeout(() => setSaveBanner(""), 3600);
      return;
    }

    setSettings(persisted);
    setSaveBanner("设置已保存，下一轮录音、转写与语义处理将使用新配置。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleModelTest(provider: "asr" | "todo") {
    setTestingProvider(provider);
    const result = await testDesktopModelConnection(provider, settings).catch(() => null);
    setTestingProvider("");

    if (!result) {
      setSaveBanner("当前浏览器原型模式不支持云模型连接测试。");
      window.setTimeout(() => setSaveBanner(""), 3200);
      return;
    }

    const label = provider === "asr" ? "ASR" : "Todo";
    const excerpt = result.responseExcerpt ? ` ${result.responseExcerpt}` : "";
    setSaveBanner(`${label} 测试结果：${result.message}${excerpt}`);
    window.setTimeout(() => setSaveBanner(""), 6000);
  }

  async function toggleTodoStatus(todoId: string) {
    const desktopTodo = await toggleDesktopTodoStatus(todoId).catch(() => null);

    if (desktopTodo) {
      setTodos((current) =>
        current.map((todo) => (todo.id === desktopTodo.id ? desktopTodo : todo)),
      );
      return;
    }

    setTodos((current) =>
      current.map((todo) =>
        todo.id === todoId
          ? {
              ...todo,
              status: todo.status === "done" ? "open" : "done",
            }
          : todo,
      ),
    );
  }

  async function handleTodoStatusChange(todoId: string, status: TodoStatus) {
    const updated = await updateDesktopTodoStatus(todoId, status).catch(() => null);
    if (isTauriEnvironment() && !updated) {
      setSaveBanner("更新 Todo 状态失败，请刷新后重试。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    setTodos((current) =>
      current.map((todo) =>
        todo.id === todoId
          ? {
              ...todo,
              status: updated?.status ?? status,
            }
          : todo,
      ),
    );
    setSaveBanner(`Todo 已更新为${statusLabelMap[updated?.status ?? status]}。`);
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleSyncTodoCandidates() {
    const candidates = await syncDesktopTodoCandidates().catch(() => null);
    if (isTauriEnvironment() && !candidates) {
      setSaveBanner("同步待办候选失败，请先生成语义纪要。");
      window.setTimeout(() => setSaveBanner(""), 3000);
      return;
    }

    setTodoCandidates(candidates ?? defaultTodoCandidates);
    setSaveBanner("已同步待办候选。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleAcceptTodoCandidate(candidate: TodoCandidateItem) {
    const accepted = await acceptDesktopTodoCandidate({
      candidateId: candidate.id,
      title: candidate.title,
      detail: candidate.detail,
      owner: candidate.owner,
      dueAt: candidate.dueAt,
      priority: candidate.priority,
    }).catch(() => null);
    if (isTauriEnvironment() && !accepted) {
      setSaveBanner("确认待办候选失败，请刷新后重试。");
      window.setTimeout(() => setSaveBanner(""), 3000);
      return;
    }

    const todo: TodoItem =
      accepted ?? {
        id: `todo_browser_${candidate.id}`,
        title: candidate.title,
        note: candidate.detail,
        status: "open",
        createdAt: new Date().toISOString().slice(0, 19).replace("T", " "),
        conversationSessionId: candidate.sessionId,
        sourceText: candidate.sourceText,
        owner: candidate.owner,
        dueAt: candidate.dueAt,
        priority: candidate.priority,
        sourceSpanRefs: candidate.sourceSpanRefs,
        candidateId: candidate.id,
      };
    setTodos((current) =>
      current.some((item) => item.id === todo.id) ? current : [todo, ...current],
    );
    setSelectedTodoId(todo.id);
    setTodoCandidates((current) =>
      current.map((item) =>
        item.id === candidate.id
          ? {
              ...item,
              status: "accepted",
              todoId: todo.id,
            }
          : item,
      ),
    );
    setSaveBanner("候选已进入正式 Todo。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleDismissTodoCandidate(candidateId: string) {
    const dismissed = await dismissDesktopTodoCandidate(candidateId).catch(() => null);
    if (isTauriEnvironment() && !dismissed) {
      setSaveBanner("忽略待办候选失败，请刷新后重试。");
      window.setTimeout(() => setSaveBanner(""), 3000);
      return;
    }

    setTodoCandidates((current) =>
      current.map((candidate) =>
        candidate.id === candidateId
          ? {
              ...candidate,
              status: dismissed?.status ?? "dismissed",
            }
          : candidate,
      ),
    );
    setSaveBanner("候选已忽略。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleFlushSession() {
    const now = Date.now();
    if (now - lastManualFlushAt < manualFlushCooldownMs) {
      const secondsLeft = Math.ceil((manualFlushCooldownMs - (now - lastManualFlushAt)) / 1000);
      setSaveBanner(`手动刷新过于频繁，请在 ${secondsLeft} 秒后再试。`);
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    const desktopSession = await flushDesktopSession().catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "手动刷新当前会话失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 2400);
      return null;
    });

    if (desktopSession) {
      setLastManualFlushAt(Date.now());
      const payload = await loadBootstrapData().catch(() => null);
      if (payload) {
        setTodos(payload.todos);
        setSessions(payload.sessions);
        setRuntime(payload.runtime);
        setSelectedTodoId((current) => current || payload.todos[0]?.id || "");
      } else {
        setSessions((current) => [desktopSession, ...current]);
      }
      setSaveBanner("已手动刷新当前会话，并尝试执行 Todo 提取。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    if (!desktopContext) {
      setSaveBanner("当前浏览器原型模式不支持手动刷新会话。");
      window.setTimeout(() => setSaveBanner(""), 2400);
    }
  }

  async function handleRecordingAction(action: "start" | "stop" | "effective" | "silent") {
    const result =
      action === "start"
        ? await startDesktopRecording().catch(() => null)
        : action === "stop"
          ? await stopDesktopRecording().catch(() => null)
          : action === "effective"
            ? await simulateDesktopAudioSlice(true).catch(() => null)
            : await simulateDesktopAudioSlice(false).catch(() => null);

    if (!result) {
      setSaveBanner("当前浏览器原型模式不支持录音骨架控制。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    setRuntime(result.runtime);
    const payload = await loadBootstrapData().catch(() => null);
    if (payload) {
      setSettings(payload.settings);
      setTodos(payload.todos);
      setSessions(payload.sessions);
      setRuntime(payload.runtime);
      setSelectedTodoId((current) => current || payload.todos[0]?.id || "");
    } else if (result.latestSession) {
      setSessions((current) => {
        const remaining = current.filter((item) => item.id !== result.latestSession?.id);
        return [result.latestSession as SessionItem, ...remaining];
      });
    }
    setSaveBanner(result.message);
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleProcessPendingJobs() {
    const result = await processDesktopPendingJobs().catch(() => null);

    if (!result) {
      setSaveBanner("当前浏览器原型模式不支持处理待办任务。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    setTodos(result.todos);
    setSessions(result.sessions);
    setRuntime(result.runtime);
    setSelectedTodoId((current) => current || result.todos[0]?.id || "");
    setSaveBanner(result.message);
    window.setTimeout(() => setSaveBanner(""), 4000);
  }

  async function handleImportAudio() {
    if (!audioImportPath.trim()) {
      setSaveBanner("请输入本地音频文件路径，用于离线转写评估。");
      window.setTimeout(() => setSaveBanner(""), 2600);
      return;
    }

    const review = await importDesktopLocalAudio(audioImportPath.trim()).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "导入本地音频失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 3600);
      return null;
    });

    if (!review) {
      const nextReview = {
        ...defaultTranscriptReview,
        audio: {
          ...defaultTranscriptReview.audio,
          fileName: audioImportPath.trim().split("/").filter(Boolean).pop() || "local-audio.wav",
        },
      };
      setTranscriptReview(nextReview);
      setSelectedTranscriptSegmentId(nextReview.segments[0]?.id ?? "");
      setSaveBanner("浏览器原型模式已载入本地评估样例。桌面端会读取真实路径。");
      window.setTimeout(() => setSaveBanner(""), 3600);
      return;
    }

    setTranscriptReview(review);
    setSelectedTranscriptSegmentId(review.segments[0]?.id ?? "");
    setSpeakerDrafts(Object.fromEntries(review.speakers.map((speaker) => [speaker.id, speaker.label])));
    setSaveBanner("已导入音频并生成本地转写评估时间轴。");
    window.setTimeout(() => setSaveBanner(""), 3600);
  }

  function jumpToTranscriptSegment(segmentId: string, startMs: number) {
    setSelectedTranscriptSegmentId(segmentId);
    setCurrentPlaybackMs(startMs);
  }

  async function handleRenameSpeaker(speakerId: string) {
    const label = speakerDrafts[speakerId]?.trim();
    if (!label) {
      setSaveBanner("说话人名称不能为空。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    const renamed = await renameDesktopSpeaker(speakerId, label).catch(() => null);
    setTranscriptReview((current) => ({
      ...current,
      speakers: current.speakers.map((speaker) =>
        speaker.id === speakerId
          ? { ...speaker, label, displayName: label, corrected: true }
          : speaker,
      ),
      segments: current.segments.map((segment) =>
        segment.speakerId === speakerId ? { ...segment, speakerLabel: label } : segment,
      ),
    }));

    if (renamed) {
      setSpeakerDrafts((current) => ({ ...current, [speakerId]: renamed.label }));
    }
    setSaveBanner("说话人名称已保存。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleMarkTranscriptSegment(segmentId: string) {
    const reason = "用户标注：该片段需要人工复核";
    const marked = await markDesktopTranscriptSegment(segmentId, "manual_review", reason).catch(
      () => null,
    );

    setTranscriptReview((current) => ({
      ...current,
      segments: current.segments.map((segment) =>
        segment.id === segmentId
          ? {
              ...segment,
              reviewStatus: "flagged",
              reviewReason: marked?.reviewReason || reason,
            }
          : segment,
      ),
    }));
    setSaveBanner("已标注错误片段。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleRetryTranscriptJob(jobId: string) {
    const retried = await retryDesktopTranscriptJob(jobId).catch(() => null);

    if (isTauriEnvironment() && !retried) {
      setSaveBanner("转写任务当前不可重试，请检查任务状态。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    setTranscriptReview((current) => ({
      ...current,
      jobs: current.jobs.map((job) =>
        job.id === jobId
          ? {
              ...job,
              status: "queued",
              retryCount: retried?.retryCount ?? job.retryCount + 1,
              errorMessage: "",
            }
          : job,
      ),
    }));
    setSaveBanner("失败转写任务已重新排队。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleGenerateSemanticWorkbench() {
    setSemanticLoading(true);
    const generated = await generateSemanticWorkbench().catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "生成语义纪要失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 3200);
      return null;
    });
    setSemanticLoading(false);

    if (!generated) {
      setSemanticWorkbench(defaultSemanticWorkbench);
      setSaveBanner("浏览器原型模式已载入语义纪要样例。桌面端会写入 semantic_artifacts。");
      window.setTimeout(() => setSaveBanner(""), 3600);
      return;
    }

    setSemanticWorkbench(generated);
    setSaveBanner("已基于修正文稿生成摘要、纪要和待办候选。");
    window.setTimeout(() => setSaveBanner(""), 3200);
  }

  async function handleToggleCorrectionPattern(pattern: CorrectionPattern) {
    const updated = await setDesktopCorrectionPatternEnabled(pattern.id, !pattern.enabled).catch(
      () => null,
    );
    if (isTauriEnvironment() && !updated) {
      setSaveBanner("更新修正记忆失败，请刷新后重试。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }
    const nextEnabled = updated?.enabled ?? !pattern.enabled;

    setSemanticWorkbench((current) => ({
      ...current,
      correctionPatterns: current.correctionPatterns.map((candidate) =>
        candidate.id === pattern.id
          ? {
              ...candidate,
              enabled: nextEnabled,
            }
          : candidate,
      ),
    }));
    setSaveBanner(nextEnabled ? "修正记忆已启用。" : "修正记忆已禁用。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleDeleteCorrectionPattern(patternId: string) {
    const deleted = await deleteDesktopCorrectionPattern(patternId).catch(() => null);
    if (isTauriEnvironment() && !deleted) {
      setSaveBanner("删除修正记忆失败，请刷新后重试。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }
    const deletedId = deleted?.deletedId ?? patternId;

    setSemanticWorkbench((current) => ({
      ...current,
      correctionPatterns: current.correctionPatterns.filter((pattern) => pattern.id !== deletedId),
    }));
    setSaveBanner("修正记忆已删除。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleRetrySemanticArtifact(artifact: SemanticArtifact) {
    const retried = await retryDesktopSemanticArtifact(artifact.id).catch(() => null);

    if (isTauriEnvironment() && !retried) {
      setSaveBanner("语义产物当前不可重试，请检查状态。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    setSemanticWorkbench((current) => ({
      ...current,
      artifacts: current.artifacts.map((candidate) =>
        candidate.id === artifact.id
          ? {
              ...candidate,
              status: retried?.status ?? "pending",
              errorMessage: retried?.errorMessage ?? "",
            }
          : candidate,
      ),
    }));
    setSaveBanner("失败语义产物已重新排队。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  async function handleRejectTranscriptRevision(revisionId: string) {
    const rejected = await rejectDesktopTranscriptRevision(revisionId).catch(() => null);
    if (isTauriEnvironment() && !rejected) {
      setSaveBanner("拒绝修正失败，请刷新后重试。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    setSemanticWorkbench((current) => ({
      ...current,
      revisions: current.revisions.map((revision) =>
        revision.id === revisionId
          ? {
              ...revision,
              status: rejected?.status ?? "rejected",
            }
          : revision,
      ),
    }));
    setSaveBanner("该条修正已拒绝，不再作为建议采用。");
    window.setTimeout(() => setSaveBanner(""), 2400);
  }

  function applyMindMapArtifact(artifact: SemanticArtifact) {
    const mindMap = parseMindMapArtifact(artifact);
    if (!mindMap) {
      setSaveBanner("脑图产物解析失败，请重新生成。");
      window.setTimeout(() => setSaveBanner(""), 2600);
      return;
    }

    setSemanticWorkbench((current) => ({
      ...current,
      mindMap,
      artifacts: [
        artifact,
        ...current.artifacts.filter((candidate) => candidate.id !== artifact.id),
      ],
    }));
    setSelectedMindMapNodeId((current) => current || mindMap.root);
  }

  async function handleGenerateMindMap() {
    setMindMapLoading(true);
    const artifact = await generateDesktopMindMap().catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "生成思维脑图失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 3200);
      return null;
    });
    setMindMapLoading(false);

    if (artifact) {
      applyMindMapArtifact(artifact);
      setSaveBanner("已生成思维脑图。");
      window.setTimeout(() => setSaveBanner(""), 2800);
      return;
    }

    if (!isTauriEnvironment() && semanticWorkbench.mindMap) {
      const nextMindMap = {
        ...semanticWorkbench.mindMap,
        edited: false,
        version: semanticWorkbench.mindMap.version + 1,
        parentArtifactId: "",
      };
      const localArtifact = createLocalMindMapArtifact(semanticWorkbench, nextMindMap, false);
      applyMindMapArtifact(localArtifact);
      setSaveBanner("浏览器原型模式已生成脑图样例。");
      window.setTimeout(() => setSaveBanner(""), 2800);
    }
  }

  async function handleToggleMindMapNode(nodeId: string, collapsed: boolean) {
    const artifact = semanticWorkbench.artifacts.find(
      (candidate) => candidate.artifactType === "mind_map" && candidate.status === "succeeded",
    );
    const updated = artifact
      ? await toggleDesktopMindMapNode({
          artifactId: artifact.id,
          nodeId,
          collapsed,
        }).catch(() => null)
      : null;

    if (updated) {
      applyMindMapArtifact(updated);
      setSaveBanner(collapsed ? "节点已折叠为新版本。" : "节点已展开为新版本。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    if (!isTauriEnvironment() && semanticWorkbench.mindMap) {
      const nextMindMap = {
        ...semanticWorkbench.mindMap,
        nodes: semanticWorkbench.mindMap.nodes.map((node) =>
          node.id === nodeId ? { ...node, collapsed } : node,
        ),
        edited: true,
        version: semanticWorkbench.mindMap.version + 1,
        parentArtifactId: artifact?.id ?? "",
      };
      applyMindMapArtifact(createLocalMindMapArtifact(semanticWorkbench, nextMindMap, true));
    }
  }

  async function handleSaveMindMapNode() {
    const artifact = semanticWorkbench.artifacts.find(
      (candidate) => candidate.artifactType === "mind_map" && candidate.status === "succeeded",
    );
    if (!semanticWorkbench.mindMap || !selectedMindMapNodeId || !mindMapDraft.label.trim()) {
      setSaveBanner("请选择节点并填写节点标题。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    const updated = artifact
      ? await updateDesktopMindMapNode({
          artifactId: artifact.id,
          nodeId: selectedMindMapNodeId,
          label: mindMapDraft.label.trim(),
          note: mindMapDraft.note.trim(),
        }).catch(() => null)
      : null;

    if (updated) {
      applyMindMapArtifact(updated);
      setSaveBanner("节点编辑已保存为新脑图版本。");
      window.setTimeout(() => setSaveBanner(""), 2600);
      return;
    }

    if (!isTauriEnvironment()) {
      const nextMindMap = {
        ...semanticWorkbench.mindMap,
        nodes: semanticWorkbench.mindMap.nodes.map((node) =>
          node.id === selectedMindMapNodeId
            ? { ...node, label: mindMapDraft.label.trim(), note: mindMapDraft.note.trim() }
            : node,
        ),
        edited: true,
        version: semanticWorkbench.mindMap.version + 1,
        parentArtifactId: artifact?.id ?? "",
      };
      applyMindMapArtifact(createLocalMindMapArtifact(semanticWorkbench, nextMindMap, true));
      setSaveBanner("浏览器原型模式已保存编辑版本。");
      window.setTimeout(() => setSaveBanner(""), 2600);
    }
  }

  async function handleExportMindMap(format: "markdown" | "json") {
    const artifact = semanticWorkbench.artifacts.find(
      (candidate) => candidate.artifactType === "mind_map" && candidate.status === "succeeded",
    );
    const exported = artifact
      ? await exportDesktopMindMap(artifact.id, format).catch(() => null)
      : null;

    if (exported) {
      setMindMapExport(exported);
      setSaveBanner(`已生成 ${exported.fileName} 导出内容。`);
      window.setTimeout(() => setSaveBanner(""), 2600);
      return;
    }

    if (!isTauriEnvironment() && semanticWorkbench.mindMap) {
      setMindMapExport({
        format,
        fileName: `demo-mind-map.${format === "markdown" ? "md" : "json"}`,
        content:
          format === "markdown"
            ? mindMapToMarkdown(semanticWorkbench.mindMap)
            : JSON.stringify(semanticWorkbench.mindMap, null, 2),
      });
      setSaveBanner("浏览器原型模式已生成导出预览。");
      window.setTimeout(() => setSaveBanner(""), 2600);
    }
  }

  async function handleGenerateExportBundle() {
    setExportLoading(true);
    const generated = await generateDesktopExportBundle({
      formats: ["markdown", "srt", "json", "snapshot"],
      targetLanguages: [],
    }).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "生成导出包失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 3200);
      return null;
    });
    setExportLoading(false);

    if (generated) {
      setExportBundle(generated);
      setSelectedExportFormat(generated.items[0]?.format ?? "markdown");
      setSaveBanner("已生成 Markdown、SRT、JSON 和本地分享快照。");
      window.setTimeout(() => setSaveBanner(""), 3200);
      return;
    }

    if (!isTauriEnvironment()) {
      setExportBundle(defaultExportBundle);
      setSelectedExportFormat(defaultExportBundle.items[0]?.format ?? "markdown");
      setSaveBanner("浏览器原型模式已载入导出包样例。");
      window.setTimeout(() => setSaveBanner(""), 3200);
    }
  }

  async function handleGenerateTranslation() {
    const targetLanguage = selectedTargetLanguage.trim();
    if (!targetLanguage) {
      setSaveBanner("请选择目标语言。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }
    setTranslationLoading(true);
    const generated = await generateDesktopTranslation({ targetLanguage }).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "生成翻译失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 3200);
      return null;
    });
    setTranslationLoading(false);

    const artifact = generated ?? (!isTauriEnvironment() ? createLocalTranslationArtifact(semanticWorkbench, targetLanguage) : null);
    if (!artifact) {
      return;
    }
    const translation = parseTranslationArtifact(artifact);
    setSemanticWorkbench((current) => ({
      ...current,
      translations: translation
        ? [
            translation,
            ...current.translations.filter(
              (candidate) => candidate.targetLanguage !== translation.targetLanguage,
            ),
          ]
        : current.translations,
      artifacts: [
        artifact,
        ...current.artifacts.filter((candidate) => candidate.id !== artifact.id),
      ],
    }));
    setSaveBanner(`已生成 ${targetLanguage} 转写与摘要翻译。`);
    window.setTimeout(() => setSaveBanner(""), 3000);
  }

  async function handleGenerateMultilingualExportBundle() {
    const targetLanguage = selectedTargetLanguage.trim();
    if (!targetLanguage) {
      setSaveBanner("请选择目标语言。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }
    const hasTranslation = semanticWorkbench.translations.some(
      (translation) => translation.targetLanguage === targetLanguage,
    );
    if (!hasTranslation) {
      setSaveBanner("请先生成该目标语言的翻译产物。");
      window.setTimeout(() => setSaveBanner(""), 2600);
      return;
    }

    setExportLoading(true);
    const generated = await generateDesktopExportBundle({
      formats: ["markdown", "srt", "json", "snapshot"],
      targetLanguages: [targetLanguage],
    }).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "生成多语言导出包失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 3200);
      return null;
    });
    setExportLoading(false);

    if (generated) {
      setExportBundle(generated);
      setSelectedExportFormat(generated.items[0]?.format ?? `markdown_${targetLanguage}`);
      setSaveBanner(`已生成 ${targetLanguage} 多语言导出包。`);
      window.setTimeout(() => setSaveBanner(""), 3000);
      return;
    }

    if (!isTauriEnvironment()) {
      const translation = semanticWorkbench.translations.find(
        (candidate) => candidate.targetLanguage === targetLanguage,
      );
      if (!translation) {
        return;
      }
      const localBundle: ExportBundle = {
        ...defaultExportBundle,
        id: `export_bundle_demo_${targetLanguage}`,
        items: [
          {
            id: `export_demo_markdown_${targetLanguage}`,
            format: `markdown_${targetLanguage}`,
            fileName: `声记多语言导出-${targetLanguage}.md`,
            mimeType: "text/markdown; charset=utf-8",
            content: [
              "# ShengJi Multilingual Export",
              "",
              "## Summary Translation",
              translation.summaryTranslation.translatedBasis,
              "",
              "## Transcript Translation",
              ...translation.transcriptTranslations.map(
                (segment) =>
                  `- Source segment \`${segment.sourceSegmentId}\` · ${segment.speakerLabel}: ${segment.translatedText}`,
              ),
            ].join("\n"),
            status: "succeeded",
            sourceSpanRefs: translation.sourceSpanRefs,
            errorMessage: "",
          },
          {
            id: `export_demo_json_${targetLanguage}`,
            format: `json_${targetLanguage}`,
            fileName: `声记多语言结构化导出-${targetLanguage}.json`,
            mimeType: "application/json; charset=utf-8",
            content: JSON.stringify(
              {
                sessionId: semanticWorkbench.sessionId,
                targetLanguage,
                translations: translation,
              },
              null,
              2,
            ),
            status: "succeeded",
            sourceSpanRefs: translation.sourceSpanRefs,
            errorMessage: "",
          },
        ],
        snapshot: {
          id: `export_demo_snapshot_${targetLanguage}`,
          fileName: `声记多语言分享快照-${targetLanguage}.html`,
          title: `声记 Multilingual 分享快照 · ${targetLanguage}`,
          html: "<!doctype html><html lang=\"en\"><title>ShengJi Multilingual Snapshot</title><body>Multilingual local export</body></html>",
          sourceSpanRefs: translation.sourceSpanRefs,
          privacySummary: defaultExportBundle.privacySummary,
        },
      };
      setExportBundle(localBundle);
      setSelectedExportFormat(localBundle.items[0]?.format ?? `markdown_${targetLanguage}`);
      setSaveBanner(`浏览器原型模式已生成 ${targetLanguage} 多语言导出包。`);
      window.setTimeout(() => setSaveBanner(""), 3200);
    }
  }

  function findResearchArtifact(researchId: string) {
    return semanticWorkbench.artifacts.find((artifact) => {
      const research = parseResearchArtifact(artifact);
      return research?.id === researchId;
    });
  }

  async function handleGenerateValueDiscovery() {
    setValueDiscoveryLoading(true);
    const artifact = await generateDesktopValueDiscovery().catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "生成价值发现失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 3200);
      return null;
    });
    setValueDiscoveryLoading(false);

    if (artifact) {
      const refreshed = await loadSemanticWorkbench().catch(() => null);
      if (refreshed) {
        setSemanticWorkbench(refreshed);
        setSelectedResearchId(refreshed.deepResearch[0]?.id ?? "");
      } else {
        const moments = parseMomentArtifact(artifact);
        setSemanticWorkbench((current) => ({
          ...current,
          moments: moments.length > 0 ? moments : current.moments,
          artifacts: [
            artifact,
            ...current.artifacts.filter((candidate) => candidate.id !== artifact.id),
          ],
        }));
      }
      setSaveBanner("已生成 Moment 与研究草稿。");
      window.setTimeout(() => setSaveBanner(""), 3000);
      return;
    }

    if (!isTauriEnvironment()) {
      setSemanticWorkbench(defaultSemanticWorkbench);
      setSelectedResearchId(defaultSemanticWorkbench.deepResearch[0]?.id ?? "");
      setSaveBanner("浏览器原型模式已载入价值发现样例。");
      window.setTimeout(() => setSaveBanner(""), 3000);
    }
  }

  async function handleStartResearchFromSegment() {
    const selectedRevision = semanticWorkbench.revisions.find(
      (revision) => revision.sourceSegmentId === researchSegmentId,
    );
    if (!selectedRevision) {
      setSaveBanner("请选择一个可研究的来源片段。");
      window.setTimeout(() => setSaveBanner(""), 2400);
      return;
    }

    const artifact = await startDesktopResearchFromSegment({
      segmentId: selectedRevision.sourceSegmentId,
      question: researchQuestion,
    }).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "发起研究失败。";
      setSaveBanner(message);
      window.setTimeout(() => setSaveBanner(""), 3200);
      return null;
    });

    if (artifact) {
      const research = parseResearchArtifact(artifact);
      const refreshed = await loadSemanticWorkbench().catch(() => null);
      if (refreshed) {
        setSemanticWorkbench(refreshed);
      } else if (research) {
        setSemanticWorkbench((current) => ({
          ...current,
          deepResearch: [
            research,
            ...current.deepResearch.filter((candidate) => candidate.id !== research.id),
          ],
          artifacts: [
            artifact,
            ...current.artifacts.filter((candidate) => candidate.id !== artifact.id),
          ],
        }));
      }
      setSelectedResearchId(research?.id ?? "");
      setSaveBanner("已从来源片段生成研究草稿。");
      window.setTimeout(() => setSaveBanner(""), 2800);
      return;
    }

    if (!isTauriEnvironment()) {
      const research: DeepResearchDraft = {
        id: `research_${selectedRevision.sourceSegmentId}`,
        question: researchQuestion.trim() || "这个片段是否值得继续研究？",
        background: `${selectedRevision.speakerLabel} ${formatDuration(selectedRevision.startMs)} - ${formatDuration(selectedRevision.endMs)}：${selectedRevision.revisedText}`,
        hypotheses: ["该片段可能包含后续验收风险。", "补充来源证据后可转为行动项。"],
        searchDirections: ["检查同会话低置信度片段。", "对照语义产物来源覆盖率。"],
        nextSteps: ["复核该片段并补充结论。", "将结论转为 Todo 或脑图节点。"],
        sourceSpanRefs: [selectedRevision.sourceSegmentId],
        convertedTodoId: "",
        mindMapNodeId: "",
      };
      const artifact = createLocalValueArtifact(
        semanticWorkbench,
        "deep_research",
        research,
        research.sourceSpanRefs,
      );
      setSemanticWorkbench((current) => ({
        ...current,
        deepResearch: [
          research,
          ...current.deepResearch.filter((candidate) => candidate.id !== research.id),
        ],
        artifacts: [artifact, ...current.artifacts],
      }));
      setSelectedResearchId(research.id);
      setSaveBanner("浏览器原型模式已生成片段研究草稿。");
      window.setTimeout(() => setSaveBanner(""), 2800);
    }
  }

  async function handleConvertResearchToTodo(research: DeepResearchDraft) {
    const artifact = findResearchArtifact(research.id);
    const converted = artifact
      ? await convertDesktopResearchToTodo({
          artifactId: artifact.id,
          researchId: research.id,
        }).catch(() => null)
      : null;

    if (converted) {
      setTodos((current) => [
        converted,
        ...current.filter((todo) => todo.id !== converted.id),
      ]);
      setSelectedTodoId(converted.id);
      const refreshed = await loadSemanticWorkbench().catch(() => null);
      if (refreshed) {
        setSemanticWorkbench(refreshed);
      }
      setSaveBanner("研究结论已转为正式 Todo。");
      window.setTimeout(() => setSaveBanner(""), 2800);
      return;
    }

    if (!isTauriEnvironment()) {
      const todoId = `todo_v09_${Date.now()}`;
      const localTodo: TodoItem = {
        id: todoId,
        title: `研究：${research.question}`,
        note: research.nextSteps.join("；"),
        status: "open",
        createdAt: "刚刚",
        conversationSessionId: semanticWorkbench.sessionId,
        sourceText: research.background,
        owner: "",
        dueAt: "",
        priority: "medium",
        sourceSpanRefs: research.sourceSpanRefs,
        candidateId: research.id,
      };
      setTodos((current) => [localTodo, ...current]);
      setSelectedTodoId(todoId);
      setSemanticWorkbench((current) => ({
        ...current,
        deepResearch: current.deepResearch.map((candidate) =>
          candidate.id === research.id ? { ...candidate, convertedTodoId: todoId } : candidate,
        ),
      }));
      setSaveBanner("浏览器原型模式已把研究结论转为 Todo。");
      window.setTimeout(() => setSaveBanner(""), 2800);
    }
  }

  async function handleAddResearchToMindMap(research: DeepResearchDraft) {
    const artifact = findResearchArtifact(research.id);
    const updated = artifact
      ? await addDesktopResearchToMindMap({
          artifactId: artifact.id,
          researchId: research.id,
        }).catch(() => null)
      : null;

    if (updated) {
      applyMindMapArtifact(updated);
      const refreshed = await loadSemanticWorkbench().catch(() => null);
      if (refreshed) {
        setSemanticWorkbench(refreshed);
      }
      setSaveBanner("研究结论已追加为脑图节点。");
      window.setTimeout(() => setSaveBanner(""), 2800);
      return;
    }

    if (!isTauriEnvironment() && semanticWorkbench.mindMap) {
      const nodeId = `research_${research.id}`;
      const nextMindMap: MindMapArtifact = {
        ...semanticWorkbench.mindMap,
        nodes: [
          ...semanticWorkbench.mindMap.nodes,
          {
            id: nodeId,
            label: `研究：${research.question}`,
            kind: "research",
            note: research.nextSteps.join("；"),
            sourceSpanRefs: research.sourceSpanRefs,
            collapsed: false,
          },
        ],
        edges: [
          ...semanticWorkbench.mindMap.edges,
          { id: `edge_root_${nodeId}`, from: semanticWorkbench.mindMap.root, to: nodeId, label: "研究" },
        ],
        sourceSpans: Array.from(new Set([...semanticWorkbench.mindMap.sourceSpans, ...research.sourceSpanRefs])),
        edited: true,
        version: semanticWorkbench.mindMap.version + 1,
        parentArtifactId: semanticWorkbench.artifacts.find((item) => item.artifactType === "mind_map")?.id ?? "",
      };
      const localArtifact = createLocalMindMapArtifact(semanticWorkbench, nextMindMap, true);
      applyMindMapArtifact(localArtifact);
      setSemanticWorkbench((current) => ({
        ...current,
        deepResearch: current.deepResearch.map((candidate) =>
          candidate.id === research.id ? { ...candidate, mindMapNodeId: nodeId } : candidate,
        ),
      }));
      setSelectedMindMapNodeId(nodeId);
      setSaveBanner("浏览器原型模式已追加研究脑图节点。");
      window.setTimeout(() => setSaveBanner(""), 2800);
    }
  }

  const currentMindMap = semanticWorkbench.mindMap;
  const selectedMindMapNode = currentMindMap?.nodes.find(
    (node) => node.id === selectedMindMapNodeId,
  );
  const selectedResearch =
    semanticWorkbench.deepResearch.find((research) => research.id === selectedResearchId) ??
    semanticWorkbench.deepResearch[0];

  const pendingTodoCount = todos.filter(
    (todo) => todo.status === "open" || todo.status === "in_progress",
  ).length;
  const completedTodoCount = todos.filter((todo) => todo.status === "done").length;
  const proposedCandidateCount = todoCandidates.filter(
    (candidate) => candidate.status === "proposed",
  ).length;
  const failedSessionCount = sessions.filter((session) => session.extractionStatus === "failed").length;
  const latestSession = sessions[0];
  const filteredSessions = sessions.filter((session) => {
    const query = sessionSearch.trim().toLowerCase();
    if (!query) {
      return true;
    }
    return [session.id, session.mergedText, session.extractionProviderUsed, session.triggerReason]
      .some((field) => field.toLowerCase().includes(query));
  });
  const selectedExportItem: ExportItem | undefined =
    exportBundle?.items.find((item) => item.format === selectedExportFormat) ??
    exportBundle?.items[0];
  const exportReadyCount =
    exportBundle?.items.filter((item) => item.status === "succeeded").length ?? 0;
  const activeTranslation =
    semanticWorkbench.translations.find(
      (translation) => translation.targetLanguage === selectedTargetLanguage,
    ) ?? semanticWorkbench.translations[0];
  const navItems: Array<{ key: TabKey; label: string; description: string }> = [
    { key: "overview", label: "今日工作台", description: "录音与概览" },
    { key: "actions", label: "行动中心", description: "Todo 执行" },
    { key: "transcript", label: "转写评估", description: "音频与说话人" },
    { key: "semantic", label: "语义纪要", description: "修正与候选" },
    { key: "research", label: "价值发现", description: "Moment 与研究" },
    { key: "mindmap", label: "思维脑图", description: "结构与导出" },
    { key: "export", label: "导出中心", description: "快照与归档" },
    { key: "history", label: "会话日志", description: "文稿与来源" },
    { key: "system", label: "系统状态", description: "排障与运行时" },
    { key: "settings", label: "设置", description: "模型与录音" },
  ];
  const activeNavItem = navItems.find((item) => item.key === activeTab) ?? navItems[0];
  const overviewPanelItems: Array<{ key: OverviewPanelKey; label: string; description: string }> = [
    { key: "transcript", label: "转写", description: "时间轴与片段" },
    { key: "summary", label: "摘要", description: "纪要与产物" },
    { key: "todo", label: "Todo", description: "候选与执行" },
    { key: "privacy", label: "隐私边界", description: "本地与云端" },
  ];
  const selectedTranscriptSegment =
    transcriptReview.segments.find((segment) => segment.id === selectedTranscriptSegmentId) ??
    transcriptReview.segments[0];
  const selectedRevision =
    semanticWorkbench.revisions.find(
      (revision) => revision.sourceSegmentId === selectedTranscriptSegment?.id,
    ) ?? semanticWorkbench.revisions[0];
  const latestArtifact = semanticWorkbench.artifacts[0];
  const latestModelInvocation = semanticWorkbench.modelInvocations[0];
  const titlebarLocation =
    activeTab === "overview"
      ? `${activeNavItem.label} / ${overviewPanelLabelMap[overviewPanel]}`
      : activeNavItem.label;
  const titlebarContext =
    activeTab === "overview" ? transcriptReview.audio.fileName : activeNavItem.description;
  const isRecordingActive = runtime.currentSessionStatus === "collecting";
  const recordingControlLabel = isRecordingActive ? "暂停录音" : "开始录音";
  const recordingControlAction: "start" | "stop" = isRecordingActive ? "stop" : "start";

  return (
    <div className="desktop-shell">
      <div className="app-frame">
        <header className="app-toolbar">
          <div className="window-title">
            <div className="titlebar-location">
              <strong>{titlebarLocation}</strong>
              <span>{titlebarContext}</span>
            </div>
          </div>
          <div className="titlebar-actions">
            <span className={`status-chip toolbar-recording-chip ${isRecordingActive ? "chip-recording" : ""}`}>
              {runtime.runtimeLabel}
            </span>
            <button
              className="toolbar-button toolbar-button-primary"
              type="button"
              title={recordingControlLabel}
              onClick={() => handleRecordingAction(recordingControlAction)}
            >
              {recordingControlLabel}
            </button>
            <button className="toolbar-button" type="button" title="搜索会话" onClick={() => setActiveTab("history")}>
              搜索
            </button>
            <button className="toolbar-button" type="button" title="打开转写评估" onClick={() => setActiveTab("transcript")}>
              评估
            </button>
            <button className="toolbar-button" type="button" title="打开设置" onClick={() => setActiveTab("settings")}>
              设置
            </button>
          </div>
        </header>

        <div className="window-body">
          <aside className="sidebar">
            <div className="brand-block">
              <div className="brand-mark-row">
                <img className="brand-mark" src={appIconUrl} alt="" aria-hidden="true" />
                <div>
                  <p className="section-kicker">ShengJi</p>
                  <h1>声记</h1>
                </div>
              </div>
              <button
                className="primary-button sidebar-primary-action"
                type="button"
                onClick={() => handleRecordingAction(recordingControlAction)}
              >
                {isRecordingActive ? "暂停录音" : "新建记录"}
              </button>
              <span className={`status-chip ${isRecordingActive ? "chip-recording" : ""}`}>
                {runtime.runtimeLabel}
              </span>
            </div>

            <nav className="sidebar-nav">
              {navItems.map((item) => (
                <button
                  key={item.key}
                  className={`nav-item ${activeTab === item.key ? "nav-item-active" : ""}`}
                  onClick={() => setActiveTab(item.key)}
                  type="button"
                >
                  <span>{item.label}</span>
                  <small>{item.description}</small>
                </button>
              ))}
            </nav>
          </aside>

          <section className="content-area">
            {saveBanner ? <div className="system-banner">{saveBanner}</div> : null}

            {activeTab === "overview" ? (
              <main className="record-workbench">
                <header className="record-header">
                  <div>
                    <p className="section-kicker">Record Workbench</p>
                    <h2>{transcriptReview.audio.fileName}</h2>
                    <div className="record-meta">
                      <span>今天 10:30</span>
                      <span>{formatDuration(transcriptReview.audio.durationMs)}</span>
                      <span>{semanticWorkbench.recordingType.label}</span>
                    </div>
                  </div>
                  <div className="heading-actions">
                    <span className={`status-chip recording-header-chip ${isRecordingActive ? "chip-recording" : ""}`}>
                      {runtime.runtimeLabel}
                    </span>
                    <button className="secondary-button" type="button" onClick={() => setActiveTab("export")}>
                      分享
                    </button>
                    <button className="secondary-button" type="button" onClick={() => setActiveTab("export")}>
                      导出
                    </button>
                    <button className="secondary-button" type="button" onClick={() => setActiveTab("history")}>
                      搜索
                    </button>
                    <button
                      className="primary-button"
                      type="button"
                      onClick={() => handleRecordingAction(recordingControlAction)}
                    >
                      {recordingControlLabel}
                    </button>
                  </div>
                </header>

                <nav className="record-tabs" aria-label="今日工作台分段">
                  {overviewPanelItems.map((panel) => (
                    <button
                      key={panel.key}
                      className={overviewPanel === panel.key ? "record-tab-active" : ""}
                      type="button"
                      onClick={() => setOverviewPanel(panel.key)}
                    >
                      <span>{panel.label}</span>
                      <small>{panel.description}</small>
                    </button>
                  ))}
                </nav>

                <section className={`record-layout record-layout-${overviewPanel}`}>
                  {overviewPanel === "transcript" ? (
                    <>
                      <section className="transcript-card overview-main-panel">
                        <div className="audio-strip">
                          <button className="round-play-button" type="button" onClick={() => handleRecordingAction("start")}>
                            播放
                          </button>
                          <div className="waveform" aria-hidden="true">
                            {Array.from({ length: 46 }).map((_, index) => (
                              <span key={index} style={{ height: `${10 + ((index * 7) % 24)}px` }} />
                            ))}
                          </div>
                          <select name="overviewPlaybackSpeed" value="1x" aria-label="播放速度" onChange={() => undefined}>
                            <option value="1x">1x</option>
                          </select>
                        </div>

                        <div className="transcript-feed">
                          {transcriptReview.segments.slice(0, 6).map((segment) => (
                            <article
                              key={segment.id}
                              className={`transcript-feed-row ${
                                selectedTranscriptSegmentId === segment.id ? "transcript-feed-row-active" : ""
                              }`}
                            >
                              <button
                                className="feed-time"
                                type="button"
                                onClick={() => jumpToTranscriptSegment(segment.id, segment.startMs)}
                              >
                                {formatDuration(segment.startMs)}
                              </button>
                              <div className="speaker-avatar">{segment.speakerLabel.slice(-1)}</div>
                              <button
                                className="transcript-feed-copy"
                                type="button"
                                onClick={() => jumpToTranscriptSegment(segment.id, segment.startMs)}
                              >
                                <strong>{segment.speakerLabel}</strong>
                                <p>{segment.text}</p>
                              </button>
                            </article>
                          ))}
                          {transcriptReview.segments.length === 0 ? (
                            <div className="empty-state">暂无转写片段，请先导入或开始录音。</div>
                          ) : null}
                        </div>
                      </section>

                      <aside className="overview-detail-panel">
                        <section className="panel-lite overview-timeline-detail">
                          <div className="panel-head">
                            <div>
                              <p className="section-kicker">Selected Segment</p>
                              <h3>
                                {selectedTranscriptSegment
                                  ? `${formatDuration(selectedTranscriptSegment.startMs)} - ${formatDuration(selectedTranscriptSegment.endMs)}`
                                  : "暂无片段"}
                              </h3>
                            </div>
                            {selectedTranscriptSegment ? (
                              <span
                                className={`badge ${
                                  selectedTranscriptSegment.reviewStatus === "flagged"
                                    ? "badge-failed"
                                    : "badge-completed"
                                }`}
                              >
                                {selectedTranscriptSegment.reviewStatus === "flagged" ? "需复核" : "正常"}
                              </span>
                            ) : null}
                          </div>
                          {selectedTranscriptSegment ? (
                            <>
                              <p>{selectedTranscriptSegment.text}</p>
                              <ul className="compact-list">
                                <li><span>说话人</span><strong>{selectedTranscriptSegment.speakerLabel}</strong></li>
                                <li><span>置信度</span><strong>{(selectedTranscriptSegment.confidence * 100).toFixed(0)}%</strong></li>
                                <li><span>Provider</span><strong>{selectedTranscriptSegment.provider}</strong></li>
                                <li><span>来源 ID</span><strong>{selectedTranscriptSegment.id}</strong></li>
                              </ul>
                              <div className="row-actions">
                                <button
                                  className="secondary-button"
                                  type="button"
                                  onClick={() => handleMarkTranscriptSegment(selectedTranscriptSegment.id)}
                                >
                                  标注复核
                                </button>
                                <button className="secondary-button" type="button" onClick={() => setActiveTab("transcript")}>
                                  打开评估
                                </button>
                              </div>
                            </>
                          ) : (
                            <div className="empty-state">请选择一个转写片段。</div>
                          )}
                        </section>

                        <section className="panel-lite overview-timeline-detail">
                          <div className="panel-head">
                            <div>
                              <p className="section-kicker">Revision</p>
                              <h3>修正入口</h3>
                            </div>
                            <button className="secondary-button" type="button" onClick={() => setOverviewPanel("summary")}>
                              查看摘要
                            </button>
                          </div>
                          {selectedRevision ? (
                            <>
                              <p>{selectedRevision.reasonSummary}</p>
                              <div className="revision-compare overview-revision-compare">
                                <div>
                                  <span>原文</span>
                                  <p>{selectedRevision.originalText}</p>
                                </div>
                                <div>
                                  <span>修正文稿</span>
                                  <p>{selectedRevision.revisedText}</p>
                                </div>
                              </div>
                            </>
                          ) : (
                            <div className="empty-state">暂无修正文稿。</div>
                          )}
                        </section>
                      </aside>
                    </>
                  ) : null}

                  {overviewPanel === "summary" ? (
                    <>
                      <section className="panel-lite overview-main-panel">
                        <div className="panel-head">
                          <div>
                            <p className="section-kicker">Summary</p>
                            <h3>{semanticWorkbench.summary.title}</h3>
                          </div>
                          <button
                            className="primary-button"
                            type="button"
                            onClick={handleGenerateSemanticWorkbench}
                            disabled={semanticLoading}
                          >
                            {semanticLoading ? "生成中" : "重新生成"}
                          </button>
                        </div>
                        <p className="runtime-message">{semanticWorkbench.summary.basis}</p>
                        <ul className="semantic-list">
                          {semanticWorkbench.summary.bullets.map((item) => (
                            <li key={item}>{item}</li>
                          ))}
                        </ul>
                        <div className="overview-section-grid">
                          <section>
                            <strong>决策</strong>
                            {semanticWorkbench.meetingMinutes.decisions.map((item) => (
                              <p key={item}>{item}</p>
                            ))}
                          </section>
                          <section>
                            <strong>风险</strong>
                            {semanticWorkbench.meetingMinutes.risks.map((item) => (
                              <p key={item}>{item}</p>
                            ))}
                          </section>
                          <section>
                            <strong>待确认问题</strong>
                            {semanticWorkbench.meetingMinutes.openQuestions.map((item) => (
                              <p key={item}>{item}</p>
                            ))}
                          </section>
                        </div>
                      </section>

                      <aside className="overview-detail-panel">
                        <section className="panel-lite">
                          <div className="panel-head">
                            <div>
                              <p className="section-kicker">Artifacts</p>
                              <h3>产物状态</h3>
                            </div>
                            <button className="secondary-button" type="button" onClick={() => setActiveTab("semantic")}>
                              打开纪要
                            </button>
                          </div>
                          <div className="artifact-list">
                            {semanticWorkbench.artifacts.slice(0, 5).map((artifact) => (
                              <article key={artifact.id} className="artifact-row">
                                <div>
                                  <strong>{artifact.artifactType}</strong>
                                  <span>{artifact.modelName} · {artifact.schemaVersion}</span>
                                </div>
                                <span
                                  className={`badge ${
                                    artifact.status === "failed"
                                      ? "badge-failed"
                                      : artifact.status === "succeeded"
                                        ? "badge-completed"
                                        : "badge-waiting"
                                  }`}
                                >
                                  {artifact.status}
                                </span>
                              </article>
                            ))}
                          </div>
                        </section>

                        <section className="panel-lite">
                          <div className="panel-head">
                            <div>
                              <p className="section-kicker">Latest Call</p>
                              <h3>{latestModelInvocation?.modelName ?? "暂无调用"}</h3>
                            </div>
                            {latestArtifact ? <span className="status-chip">{latestArtifact.provider}</span> : null}
                          </div>
                          {latestModelInvocation ? (
                            <>
                              <p className="runtime-message">{latestModelInvocation.requestSummary}</p>
                              <p className="runtime-message">
                                {latestModelInvocation.responseSummary || latestModelInvocation.errorMessage}
                              </p>
                            </>
                          ) : (
                            <div className="empty-state">暂无模型调用记录。</div>
                          )}
                        </section>
                      </aside>
                    </>
                  ) : null}

                  {overviewPanel === "todo" ? (
                    <>
                      <section className="panel-lite overview-main-panel">
                        <div className="panel-head">
                          <div>
                            <p className="section-kicker">Todo Candidates</p>
                            <h3>{proposedCandidateCount} 条待确认候选</h3>
                          </div>
                          <button className="secondary-button" type="button" onClick={handleSyncTodoCandidates}>
                            同步候选
                          </button>
                        </div>
                        <div className="overview-todo-list">
                          {todoCandidates.map((candidate) => (
                            <article key={candidate.id} className="candidate-row">
                              <div>
                                <div className="todo-card-header">
                                  <h3>{candidate.title}</h3>
                                  <span
                                    className={`badge ${
                                      candidate.status === "proposed"
                                        ? "badge-pending"
                                        : candidate.status === "accepted"
                                          ? "badge-completed"
                                          : "badge-waiting"
                                    }`}
                                  >
                                    {candidate.status === "proposed"
                                      ? "待确认"
                                      : candidate.status === "accepted"
                                        ? "已接受"
                                        : "已忽略"}
                                  </span>
                                </div>
                                <p>{candidate.detail}</p>
                                <div className="todo-meta">
                                  <span>{candidate.owner || "未分配"}</span>
                                  <span>{candidate.dueAt || "无截止时间"}</span>
                                  <span>优先级 {priorityLabelMap[candidate.priority]}</span>
                                  <span>来源 {candidate.sourceSpanRefs.join("、") || "暂无来源"}</span>
                                </div>
                              </div>
                              {candidate.status === "proposed" ? (
                                <div className="row-actions">
                                  <button className="primary-button" type="button" onClick={() => handleAcceptTodoCandidate(candidate)}>
                                    接受
                                  </button>
                                  <button className="secondary-button" type="button" onClick={() => handleDismissTodoCandidate(candidate.id)}>
                                    忽略
                                  </button>
                                </div>
                              ) : null}
                            </article>
                          ))}
                          {todoCandidates.length === 0 ? (
                            <div className="empty-state">暂无待确认候选，请先生成语义纪要。</div>
                          ) : null}
                        </div>
                      </section>

                      <aside className="overview-detail-panel">
                        <section className="panel-lite">
                          <div className="panel-head">
                            <div>
                              <p className="section-kicker">Current Todo</p>
                              <h3>{selectedTodo?.title ?? "暂无 Todo"}</h3>
                            </div>
                            <button className="secondary-button" type="button" onClick={() => setActiveTab("actions")}>
                              打开行动中心
                            </button>
                          </div>
                          {selectedTodo ? (
                            <>
                              <p className="runtime-message">{selectedTodo.note}</p>
                              <ul className="compact-list">
                                <li><span>状态</span><strong>{statusLabelMap[selectedTodo.status]}</strong></li>
                                <li><span>负责人</span><strong>{selectedTodo.owner || "未分配"}</strong></li>
                                <li><span>截止时间</span><strong>{selectedTodo.dueAt || "未设置"}</strong></li>
                                <li><span>优先级</span><strong>{priorityLabelMap[selectedTodo.priority]}</strong></li>
                              </ul>
                              <div className="row-actions">
                                <button className="secondary-button" type="button" onClick={() => handleTodoStatusChange(selectedTodo.id, "in_progress")}>
                                  进行中
                                </button>
                                <button className="primary-button" type="button" onClick={() => toggleTodoStatus(selectedTodo.id)}>
                                  {selectedTodo.status === "done" ? "重新打开" : "完成"}
                                </button>
                              </div>
                            </>
                          ) : (
                            <div className="empty-state">暂无 Todo 数据。</div>
                          )}
                        </section>

                        <section className="panel-lite">
                          <div className="panel-head">
                            <div>
                              <p className="section-kicker">Execution</p>
                              <h3>执行摘要</h3>
                            </div>
                          </div>
                          <div className="overview-mini-grid">
                            <div><span>待处理</span><strong>{pendingTodoCount}</strong></div>
                            <div><span>已完成</span><strong>{completedTodoCount}</strong></div>
                            <div><span>候选</span><strong>{proposedCandidateCount}</strong></div>
                            <div><span>失败</span><strong>{failedSessionCount}</strong></div>
                          </div>
                        </section>
                      </aside>
                    </>
                  ) : null}

                  {overviewPanel === "privacy" ? (
                    <>
                      <section className="panel-lite overview-main-panel">
                        <div className="panel-head">
                          <div>
                            <p className="section-kicker">Privacy Boundary</p>
                            <h3>本地优先，云端只处理转写后文本</h3>
                          </div>
                          <span className="status-chip chip-live">边界清晰</span>
                        </div>
                        <div className="overview-privacy-grid">
                          <section>
                            <strong>本地</strong>
                            <p>录音、音频路径、转写评估、说话人标签和导出文件优先留在本机。</p>
                          </section>
                          <section>
                            <strong>云端</strong>
                            <p>MiniMax M3 只接收修正后的文本上下文，用于摘要、Todo、脑图和研究。</p>
                          </section>
                          <section>
                            <strong>导出</strong>
                            <p>Markdown、SRT、JSON 和分享快照由本地 SQLite 产物生成，不上传完整路径或 API Key。</p>
                          </section>
                        </div>
                        <div className="overview-section-grid">
                          <section>
                            <strong>录音开关</strong>
                            <p>{settings.recordEnabled ? "环境音录制已开启。" : "环境音录制已关闭。"}</p>
                          </section>
                          <section>
                            <strong>ASR 兜底</strong>
                            <p>{settings.allowCloudFallback ? "允许本地 ASR 不可用时使用云端兜底。" : "关闭云端兜底。"}</p>
                          </section>
                          <section>
                            <strong>Embedding</strong>
                            <p>保持预留状态，不参与当前生产链路。</p>
                          </section>
                        </div>
                      </section>

                      <aside className="overview-detail-panel">
                        <section className="panel-lite">
                          <div className="panel-head">
                            <div>
                              <p className="section-kicker">Model & Key</p>
                              <h3>语义服务</h3>
                            </div>
                            <button className="secondary-button" type="button" onClick={() => setActiveTab("settings")}>
                              打开设置
                            </button>
                          </div>
                          <ul className="compact-list">
                            <li><span>Provider</span><strong>{settings.semanticProviderType}</strong></li>
                            <li><span>模型</span><strong>{settings.semanticModelName}</strong></li>
                            <li><span>Key</span><strong>{settings.semanticApiKeyMasked ? "已配置" : "未配置"}</strong></li>
                            <li><span>入口</span><strong>semantic_artifacts</strong></li>
                          </ul>
                          <div className="row-actions">
                            <button className="secondary-button" type="button" onClick={() => handleModelTest("todo")} disabled={testingProvider === "todo"}>
                              {testingProvider === "todo" ? "测试中" : "测试 M3"}
                            </button>
                          </div>
                        </section>

                        <section className="panel-lite">
                          <div className="panel-head">
                            <div>
                              <p className="section-kicker">Local Model</p>
                              <h3>{transcriptReview.modelStatus.modelName}</h3>
                            </div>
                            <span className="status-chip">{transcriptReview.modelStatus.downloadProgress}%</span>
                          </div>
                          <div className="model-progress-track">
                            <span style={{ width: `${transcriptReview.modelStatus.downloadProgress}%` }} />
                          </div>
                          <p className="runtime-message">{transcriptReview.modelStatus.deviceRecommendation}</p>
                        </section>
                      </aside>
                    </>
                  ) : null}
                </section>

                <footer className="audio-dock">
                  <span>{formatDuration(currentPlaybackMs)}</span>
                  <div className="audio-dock-wave" aria-hidden="true">
                    {Array.from({ length: 64 }).map((_, index) => (
                      <span key={index} style={{ height: `${4 + ((index * 5) % 18)}px` }} />
                    ))}
                  </div>
                  <span>{formatDuration(transcriptReview.audio.durationMs)}</span>
                  <button className="audio-dock-button" type="button" onClick={() => handleRecordingAction(recordingControlAction)}>
                    {recordingControlLabel}
                  </button>
                </footer>
              </main>
            ) : null}

            {activeTab === "actions" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">Action Center</p>
                    <h2>Todo 与候选确认</h2>
                    <p className="page-subtitle">从语义产物确认行动项，按状态、负责人和来源片段追踪执行。</p>
                  </div>
                  <div className="heading-actions">
                    <button className="secondary-button" type="button" onClick={handleSyncTodoCandidates}>
                      同步候选
                    </button>
                    <input
                      className="search-input"
                      aria-label="搜索 Todo"
                      name="todoSearch"
                      placeholder="搜索标题、负责人或来源"
                      value={keyword}
                      onChange={(event) => setKeyword(event.target.value)}
                    />
                  </div>
                </div>

                <section className="actions-layout">
                  <section className="panel-lite todo-list-panel">
                  <div className="panel-head">
                    <div>
                      <p className="section-kicker">Queue</p>
                      <h2>候选与行动项</h2>
                    </div>
                    <span className="status-chip">{filteredTodos.length} 个行动项</span>
                  </div>

                  <section className="candidate-panel">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">Candidates</p>
                        <h3>待确认候选</h3>
                      </div>
                      <span className="status-chip">{proposedCandidateCount} 条待处理</span>
                    </div>
                    <div className="candidate-list">
                      {todoCandidates.map((candidate) => (
                        <article key={candidate.id} className="candidate-row">
                          <div>
                            <div className="todo-card-header">
                              <h3>{candidate.title}</h3>
                              <span className={`badge ${candidate.status === "proposed" ? "badge-pending" : candidate.status === "accepted" ? "badge-completed" : "badge-waiting"}`}>
                                {candidate.status === "proposed"
                                  ? "待确认"
                                  : candidate.status === "accepted"
                                    ? "已接受"
                                    : "已忽略"}
                              </span>
                            </div>
                            <p>{candidate.detail}</p>
                            <div className="todo-meta">
                              <span>{candidate.owner || "未分配"}</span>
                              <span>{candidate.dueAt || "无截止时间"}</span>
                              <span>优先级 {priorityLabelMap[candidate.priority]}</span>
                              <span>置信度 {(candidate.confidence * 100).toFixed(0)}%</span>
                              <span>来源 {candidate.sourceSpanRefs.join("、") || "暂无来源"}</span>
                            </div>
                          </div>
                          {candidate.status === "proposed" ? (
                            <div className="row-actions">
                              <button
                                className="primary-button"
                                type="button"
                                onClick={() => handleAcceptTodoCandidate(candidate)}
                              >
                                接受
                              </button>
                              <button
                                className="secondary-button"
                                type="button"
                                onClick={() => handleDismissTodoCandidate(candidate.id)}
                              >
                                忽略
                              </button>
                            </div>
                          ) : null}
                        </article>
                      ))}
                      {todoCandidates.length === 0 ? (
                        <div className="empty-state">暂无待确认候选，请先生成语义纪要</div>
                      ) : null}
                    </div>
                  </section>
                  <div className="filter-row">
                    {[
                      ["all", "全部"],
                      ["open", "待处理"],
                      ["in_progress", "进行中"],
                      ["done", "已完成"],
                      ["dismissed", "已忽略"],
                    ].map(([key, label]) => (
                      <button
                        key={key}
                        className={`filter-chip ${filter === key ? "filter-chip-active" : ""}`}
                        onClick={() => setFilter(key as "all" | TodoStatus)}
                        type="button"
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                  <div className="todo-list">
                    {filteredTodos.map((todo) => (
                      <button
                        key={todo.id}
                        className={`todo-card ${selectedTodo?.id === todo.id ? "todo-card-active" : ""}`}
                        onClick={() => setSelectedTodoId(todo.id)}
                        type="button"
                      >
                        <div className="todo-card-header">
                          <h3>{todo.title}</h3>
                          <span className={`badge ${todo.status === "done" ? "badge-completed" : todo.status === "dismissed" ? "badge-waiting" : "badge-pending"}`}>
                            {statusLabelMap[todo.status]}
                          </span>
                        </div>
                        <p>{todo.note}</p>
                        <div className="todo-meta">
                          <span>{todo.createdAt}</span>
                          <span>{todo.owner || "未分配"}</span>
                          <span>优先级 {priorityLabelMap[todo.priority]}</span>
                          <span>来源 {todo.conversationSessionId}</span>
                        </div>
                      </button>
                    ))}
                  </div>
                </section>

                <aside className="panel-lite detail-panel">
                  {selectedTodo ? (
                    <>
                      <div className="panel-head vertical-head">
                        <div>
                          <p className="section-kicker">Todo Detail</p>
                          <h2>{selectedTodo.title}</h2>
                        </div>
                        <div className="row-actions">
                          <button className="secondary-button" type="button" onClick={() => handleTodoStatusChange(selectedTodo.id, "in_progress")}>
                            进行中
                          </button>
                          <button className="primary-button" type="button" onClick={() => toggleTodoStatus(selectedTodo.id)}>
                            {selectedTodo.status === "done" ? "重新打开" : "完成"}
                          </button>
                        </div>
                      </div>
                      <div className="detail-block">
                        <label>状态</label>
                        <span className={`badge ${selectedTodo.status === "done" ? "badge-completed" : selectedTodo.status === "dismissed" ? "badge-waiting" : "badge-pending"}`}>
                          {statusLabelMap[selectedTodo.status]}
                        </span>
                      </div>
                      <div className="detail-block">
                        <label>负责人 / 截止 / 优先级</label>
                        <p>
                          {(selectedTodo.owner || "未分配")}
                          {" / "}
                          {(selectedTodo.dueAt || "未设置截止时间")}
                          {" / "}
                          {priorityLabelMap[selectedTodo.priority]}
                        </p>
                      </div>
                      <div className="detail-block">
                        <label>备注</label>
                        <p>{selectedTodo.note}</p>
                      </div>
                      <div className="detail-block">
                        <label>来源片段</label>
                        <p>{selectedTodo.sourceSpanRefs.join("、") || "暂无来源片段"}</p>
                      </div>
                      <div className="detail-block">
                        <label>来源文稿</label>
                        <p>{selectedTodo.sourceText}</p>
                      </div>
                      <div className="row-actions">
                        <button className="secondary-button" type="button" onClick={() => handleTodoStatusChange(selectedTodo.id, "open")}>
                          重新打开
                        </button>
                        <button className="secondary-button" type="button" onClick={() => handleTodoStatusChange(selectedTodo.id, "dismissed")}>
                          忽略
                        </button>
                      </div>
                      <div className="detail-block detail-runtime">
                        <label>提取路径</label>
                        <p>
                          {selectedSession?.extractionProviderUsed ?? "未知"}
                          {" / "}
                          {selectedSession?.extractionFallbackUsed ? "发生过云端兜底" : "未发生云端兜底"}
                        </p>
                      </div>
                      <div className="detail-block detail-runtime">
                        <label>回退原因</label>
                        <p>{getFallbackReasonText(selectedSession)}</p>
                      </div>
                    </>
                  ) : (
                    <div className="empty-state">暂无 Todo 数据</div>
                  )}
                </aside>
                </section>
              </main>
            ) : null}

            {activeTab === "transcript" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">Transcript Review</p>
                    <h2>转写评估与说话人</h2>
                  </div>
                  <span
                    className={`status-chip ${
                      transcriptReview.audio.offlineAvailable ? "chip-live" : "chip-danger"
                    }`}
                  >
                    {transcriptReview.audio.offlineAvailable ? "离线可用" : "需联网"}
                  </span>
                </div>

                <section className="transcript-import-bar panel-lite">
                  <label className="field field-wide">
                    <span>本地音频路径</span>
                    <input
                      type="text"
                      value={audioImportPath}
                      onChange={(event) => setAudioImportPath(event.target.value)}
                      placeholder="/Users/wwh/Audio/meeting.wav"
                    />
                  </label>
                  <button className="primary-button" type="button" onClick={handleImportAudio}>
                    导入音频
                  </button>
                </section>

                <section className="transcript-layout">
                  <section className="panel-lite transcript-main-panel">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">Timeline</p>
                        <h3>{transcriptReview.audio.fileName}</h3>
                      </div>
                      <div className="playback-meter">
                        <strong>{formatDuration(currentPlaybackMs)}</strong>
                        <span>/ {formatDuration(transcriptReview.audio.durationMs)}</span>
                      </div>
                    </div>

                    <div className="audio-state-grid">
                      <div>
                        <span>状态</span>
                        <strong>{transcriptJobStatusLabelMap[transcriptReview.audio.status as keyof typeof transcriptJobStatusLabelMap] ?? transcriptReview.audio.status}</strong>
                      </div>
                      <div>
                        <span>Provider</span>
                        <strong>{transcriptReview.audio.provider}</strong>
                      </div>
                      <div>
                        <span>模型</span>
                        <strong>{transcriptReview.audio.modelName}</strong>
                      </div>
                    </div>

                    <div className="transcript-timeline">
                      {transcriptReview.segments.map((segment) => (
                        <article
                          key={segment.id}
                          className={`transcript-segment ${
                            selectedTranscriptSegmentId === segment.id
                              ? "transcript-segment-active"
                              : ""
                          }`}
                        >
                          <button
                            className="segment-jump"
                            type="button"
                            onClick={() =>
                              jumpToTranscriptSegment(segment.id, segment.startMs)
                            }
                          >
                            <span className="segment-time">
                              {formatDuration(segment.startMs)} - {formatDuration(segment.endMs)}
                            </span>
                            <span className="speaker-pill">{segment.speakerLabel}</span>
                          </button>
                          <p>{segment.text}</p>
                          <div className="segment-meta">
                            <span>置信度 {(segment.confidence * 100).toFixed(0)}%</span>
                            <span>{segment.provider}</span>
                            {segment.reviewStatus === "flagged" ? (
                              <span className="badge badge-failed">需复核</span>
                            ) : (
                              <span className="badge badge-completed">正常</span>
                            )}
                          </div>
                          {segment.reviewReason ? (
                            <p className="review-note">{segment.reviewReason}</p>
                          ) : null}
                          <button
                            className="text-button"
                            type="button"
                            onClick={() => handleMarkTranscriptSegment(segment.id)}
                          >
                            标注错误
                          </button>
                        </article>
                      ))}
                      {transcriptReview.segments.length === 0 ? (
                        <div className="empty-state">暂无转写时间轴，请先导入本地音频</div>
                      ) : null}
                    </div>
                  </section>

                  <aside className="transcript-side-stack">
                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Speakers</p>
                          <h3>说话人标签</h3>
                        </div>
                      </div>
                      <div className="speaker-list">
                        {transcriptReview.speakers.map((speaker) => (
                          <article key={speaker.id} className="speaker-row">
                            <span
                              className="speaker-dot"
                              style={{ backgroundColor: speaker.color }}
                            />
                            <label className="field">
                              <span>{speaker.segmentCount} 个片段</span>
                              <input
                                type="text"
                                value={speakerDrafts[speaker.id] ?? speaker.label}
                                onChange={(event) =>
                                  setSpeakerDrafts((current) => ({
                                    ...current,
                                    [speaker.id]: event.target.value,
                                  }))
                                }
                              />
                            </label>
                            <button
                              className="secondary-button"
                              type="button"
                              onClick={() => handleRenameSpeaker(speaker.id)}
                            >
                              保存
                            </button>
                          </article>
                        ))}
                        {transcriptReview.speakers.length === 0 ? (
                          <div className="empty-state">暂无说话人</div>
                        ) : null}
                      </div>
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Local Model</p>
                          <h3>本地模型状态</h3>
                        </div>
                        <span className="status-chip">{transcriptReview.modelStatus.downloadProgress}%</span>
                      </div>
                      <div className="model-progress-track">
                        <span style={{ width: `${transcriptReview.modelStatus.downloadProgress}%` }} />
                      </div>
                      <ul className="compact-list">
                        <li>
                          <span>Provider</span>
                          <strong>{transcriptReview.modelStatus.provider}</strong>
                        </li>
                        <li>
                          <span>模型</span>
                          <strong>{transcriptReview.modelStatus.modelName}</strong>
                        </li>
                        <li>
                          <span>缓存</span>
                          <strong>{transcriptReview.modelStatus.cacheDir}</strong>
                        </li>
                        <li>
                          <span>状态</span>
                          <strong>{transcriptReview.modelStatus.downloadStatus}</strong>
                        </li>
                      </ul>
                      <p className="runtime-message">
                        {transcriptReview.modelStatus.deviceRecommendation}
                      </p>
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Jobs</p>
                          <h3>转写任务</h3>
                        </div>
                      </div>
                      <div className="job-list">
                        {transcriptReview.jobs.map((job) => (
                          <article key={job.id} className="job-row">
                            <div>
                              <strong>{job.modelName}</strong>
                              <span>{job.provider}</span>
                            </div>
                            <span
                              className={`badge ${
                                job.status === "failed"
                                  ? "badge-failed"
                                  : job.status === "succeeded"
                                    ? "badge-completed"
                                    : "badge-waiting"
                              }`}
                            >
                              {transcriptJobStatusLabelMap[job.status]}
                            </span>
                            {job.errorMessage ? <p>{job.errorMessage}</p> : null}
                            {job.status === "failed" ? (
                              <button
                                className="secondary-button"
                                type="button"
                                onClick={() => handleRetryTranscriptJob(job.id)}
                              >
                                重试
                              </button>
                            ) : null}
                          </article>
                        ))}
                        {transcriptReview.jobs.length === 0 ? (
                          <div className="empty-state">暂无转写任务</div>
                        ) : null}
                      </div>
                    </section>
                  </aside>
                </section>
              </main>
            ) : null}

            {activeTab === "semantic" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">Semantic Workbench</p>
                    <h2>转写修正与类型化纪要</h2>
                  </div>
                  <div className="heading-actions">
                    <span className="status-chip chip-live">
                      {semanticWorkbench.recordingType.label}
                    </span>
                    <button
                      className="primary-button"
                      type="button"
                      onClick={handleGenerateSemanticWorkbench}
                      disabled={semanticLoading}
                    >
                      {semanticLoading ? "生成中" : "生成纪要"}
                    </button>
                  </div>
                </div>

                <section className="semantic-layout">
                  <section className="page-stack">
                    <section className="panel-lite semantic-hero-panel">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Transcript Revision</p>
                          <h3>原文 / 修正文稿对照</h3>
                        </div>
                        <span className="status-chip">
                          {semanticWorkbench.recordingType.templateId}
                        </span>
                      </div>
                      <div className="revision-list">
                        {semanticWorkbench.revisions.map((revision) => (
                          <article
                            key={revision.id}
                            className={`revision-row ${
                              revision.status === "rejected" ? "revision-row-rejected" : ""
                            }`}
                          >
                            <div className="revision-meta">
                              <span>{formatDuration(revision.startMs)} - {formatDuration(revision.endMs)}</span>
                              <span>{revision.speakerLabel}</span>
                              <span
                                className={`badge ${
                                  revision.changeLevel === "meaning_affecting"
                                    ? "badge-failed"
                                    : revision.changeLevel === "none"
                                      ? "badge-completed"
                                      : "badge-waiting"
                                }`}
                              >
                                {revision.changeLevel === "meaning_affecting"
                                  ? "影响语义"
                                  : revision.changeLevel === "wording"
                                    ? "措辞"
                                    : revision.changeLevel === "punctuation"
                                      ? "标点"
                                      : "无修正"}
                              </span>
                              {revision.status === "rejected" ? (
                                <span className="badge badge-waiting">已拒绝</span>
                              ) : null}
                            </div>
                            <div className="revision-compare">
                              <div>
                                <span>原文</span>
                                <p>{revision.originalText}</p>
                              </div>
                              <div>
                                <span>修正文稿</span>
                                <p>{revision.revisedText}</p>
                              </div>
                            </div>
                            <p className="review-note">
                              来源 {revision.sourceSegmentId} · {revision.reasonSummary}
                            </p>
                            {revision.status !== "rejected" && revision.changeLevel !== "none" ? (
                              <div className="row-actions">
                                <button
                                  className="secondary-button"
                                  type="button"
                                  onClick={() => handleRejectTranscriptRevision(revision.id)}
                                >
                                  拒绝修正
                                </button>
                              </div>
                            ) : null}
                          </article>
                        ))}
                        {semanticWorkbench.revisions.length === 0 ? (
                          <div className="empty-state">暂无修正文稿，请先生成纪要</div>
                        ) : null}
                      </div>
                    </section>

                    <section className="semantic-artifact-grid">
                      <article className="panel-lite">
                        <div className="panel-head">
                          <div>
                            <p className="section-kicker">Summary</p>
                            <h3>{semanticWorkbench.summary.title}</h3>
                          </div>
                        </div>
                        <p className="runtime-message">{semanticWorkbench.summary.basis}</p>
                        <ul className="semantic-list">
                          {semanticWorkbench.summary.bullets.map((item) => (
                            <li key={item}>{item}</li>
                          ))}
                        </ul>
                      </article>

                      <article className="panel-lite">
                        <div className="panel-head">
                          <div>
                            <p className="section-kicker">Minutes</p>
                            <h3>类型化纪要</h3>
                          </div>
                        </div>
                        <div className="minutes-block">
                          <strong>决策</strong>
                          {semanticWorkbench.meetingMinutes.decisions.map((item) => (
                            <p key={item}>{item}</p>
                          ))}
                        </div>
                        <div className="minutes-block">
                          <strong>风险</strong>
                          {semanticWorkbench.meetingMinutes.risks.map((item) => (
                            <p key={item}>{item}</p>
                          ))}
                        </div>
                      </article>
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Todo Candidates</p>
                          <h3>待办候选</h3>
                        </div>
                        <span className="status-chip">候选可进入正式 Todo</span>
                      </div>
                      <div className="todo-candidate-list">
                        {semanticWorkbench.todoCandidates.map((todo) => (
                          <article key={`${todo.title}-${todo.detail}`} className="todo-candidate-row">
                            <div>
                              <strong>{todo.title}</strong>
                              <p>{todo.detail}</p>
                              <span>来源：{todo.sourceSegmentIds.join("、") || "暂无来源"}</span>
                            </div>
                            <span className="badge badge-waiting">
                              {(todo.confidence * 100).toFixed(0)}%
                            </span>
                          </article>
                        ))}
                        {semanticWorkbench.todoCandidates.length === 0 ? (
                          <div className="empty-state">暂无待办候选</div>
                        ) : null}
                      </div>
                    </section>
                  </section>

                  <aside className="semantic-side-stack">
                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Correction Memory</p>
                          <h3>本地修正记忆</h3>
                        </div>
                      </div>
                      <div className="correction-list">
                        {semanticWorkbench.correctionPatterns.map((pattern) => (
                          <article key={pattern.id} className="correction-row">
                            <div>
                              <strong>{pattern.phrase} → {pattern.replacement}</strong>
                              <span>{pattern.patternType} · {(pattern.confidence * 100).toFixed(0)}%</span>
                            </div>
                            <div className="row-actions">
                              <button
                                className="secondary-button"
                                type="button"
                                onClick={() => handleToggleCorrectionPattern(pattern)}
                              >
                                {pattern.enabled ? "禁用" : "启用"}
                              </button>
                              <button
                                className="text-button"
                                type="button"
                                onClick={() => handleDeleteCorrectionPattern(pattern.id)}
                              >
                                删除
                              </button>
                            </div>
                          </article>
                        ))}
                        {semanticWorkbench.correctionPatterns.length === 0 ? (
                          <div className="empty-state">暂无修正记忆</div>
                        ) : null}
                      </div>
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Artifacts</p>
                          <h3>语义产物状态</h3>
                        </div>
                      </div>
                      <div className="artifact-list">
                        {semanticWorkbench.artifacts.map((artifact) => (
                          <article key={artifact.id} className="artifact-row">
                            <div>
                              <strong>{artifact.artifactType}</strong>
                              <span>{artifact.provider} · {artifact.schemaVersion}</span>
                              {artifact.errorMessage ? <p>{artifact.errorMessage}</p> : null}
                            </div>
                            <div className="row-actions">
                              <span
                                className={`badge ${
                                  artifact.status === "failed"
                                    ? "badge-failed"
                                    : artifact.status === "succeeded"
                                      ? "badge-completed"
                                      : "badge-waiting"
                                }`}
                              >
                                {artifact.status}
                              </span>
                              {artifact.status === "failed" ? (
                                <button
                                  className="secondary-button"
                                  type="button"
                                  onClick={() => handleRetrySemanticArtifact(artifact)}
                                >
                                  重试
                                </button>
                              ) : null}
                            </div>
                          </article>
                        ))}
                      </div>
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Model Calls</p>
                          <h3>模型调用记录</h3>
                        </div>
                      </div>
                      <div className="model-call-list">
                        {semanticWorkbench.modelInvocations.map((invocation) => (
                          <article key={invocation.id} className="model-call-row">
                            <strong>{invocation.modelName}</strong>
                            <span>{invocation.requestSummary}</span>
                            <span>{invocation.responseSummary || invocation.errorMessage}</span>
                          </article>
                        ))}
                      </div>
                    </section>
                  </aside>
                </section>
              </main>
            ) : null}

            {activeTab === "research" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">Value Discovery</p>
                    <h2>Moment 与深度研究</h2>
                  </div>
                  <div className="heading-actions">
                    <span className="status-chip">
                      {semanticWorkbench.moments.length} 个 Moment
                    </span>
                    <button
                      className="primary-button"
                      type="button"
                      onClick={handleGenerateValueDiscovery}
                      disabled={valueDiscoveryLoading}
                    >
                      {valueDiscoveryLoading ? "生成中" : "生成价值发现"}
                    </button>
                  </div>
                </div>

                <section className="research-layout">
                  <section className="panel-lite research-moment-panel">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">Moments</p>
                        <h3>关键片段</h3>
                      </div>
                      <span className="badge badge-waiting">3-10 条</span>
                    </div>
                    <div className="moment-list">
                      {semanticWorkbench.moments.map((moment) => (
                        <article key={moment.id} className={`moment-row moment-row-${moment.momentType}`}>
                          <div className="moment-row-head">
                            <span className="status-chip">{moment.title}</span>
                            <strong>{formatDuration(moment.startMs)} - {formatDuration(moment.endMs)}</strong>
                          </div>
                          <p>{moment.summary}</p>
                          <div className="moment-meta">
                            <span>{(moment.importance * 100).toFixed(0)}%</span>
                            <span>{moment.actionHint}</span>
                          </div>
                          <div className="source-ref-list">
                            {moment.sourceSpanRefs.map((source) => (
                              <button
                                key={source}
                                className="source-ref"
                                type="button"
                                onClick={() => {
                                  setResearchSegmentId(source);
                                  setActiveTab("transcript");
                                }}
                                title="跳转到转写评估页查看来源"
                              >
                                {source}
                              </button>
                            ))}
                          </div>
                        </article>
                      ))}
                      {semanticWorkbench.moments.length === 0 ? (
                        <div className="empty-state">暂无 Moment，请先生成价值发现。</div>
                      ) : null}
                    </div>
                  </section>

                  <section className="page-stack">
                    <section className="panel-lite research-launch-panel">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Research From Segment</p>
                          <h3>从片段发起研究</h3>
                        </div>
                      </div>
                      <div className="research-launch-grid">
                        <label className="field field-wide">
                          <span>来源片段</span>
                          <select
                            value={researchSegmentId}
                            onChange={(event) => setResearchSegmentId(event.target.value)}
                          >
                            {semanticWorkbench.revisions.map((revision) => (
                              <option key={revision.id} value={revision.sourceSegmentId}>
                                {formatDuration(revision.startMs)} {revision.speakerLabel} · {revision.sourceSegmentId}
                              </option>
                            ))}
                          </select>
                        </label>
                        <label className="field field-wide">
                          <span>研究问题</span>
                          <input
                            type="text"
                            value={researchQuestion}
                            onChange={(event) => setResearchQuestion(event.target.value)}
                            placeholder="输入一个要继续验证的问题"
                          />
                        </label>
                        <button
                          className="secondary-button"
                          type="button"
                          onClick={handleStartResearchFromSegment}
                        >
                          生成片段研究
                        </button>
                      </div>
                    </section>

                    <section className="panel-lite research-detail-panel">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Deep Research</p>
                          <h3>{selectedResearch?.question ?? "暂无研究草稿"}</h3>
                        </div>
                        {selectedResearch ? (
                          <span className="status-chip">
                            {selectedResearch.convertedTodoId ? "已转 Todo" : "草稿"}
                          </span>
                        ) : null}
                      </div>

                      {selectedResearch ? (
                        <div className="research-detail">
                          <p className="runtime-message">{selectedResearch.background}</p>
                          <div className="research-columns">
                            <div>
                              <strong>待验证假设</strong>
                              {selectedResearch.hypotheses.map((item) => (
                                <p key={item}>{item}</p>
                              ))}
                            </div>
                            <div>
                              <strong>检索方向</strong>
                              {selectedResearch.searchDirections.map((item) => (
                                <p key={item}>{item}</p>
                              ))}
                            </div>
                            <div>
                              <strong>可执行下一步</strong>
                              {selectedResearch.nextSteps.map((item) => (
                                <p key={item}>{item}</p>
                              ))}
                            </div>
                          </div>
                          <div className="source-ref-list">
                            {selectedResearch.sourceSpanRefs.map((source) => (
                              <button
                                key={source}
                                className="source-ref"
                                type="button"
                                onClick={() => setActiveTab("transcript")}
                                title="跳转到转写评估页查看来源"
                              >
                                {source}
                              </button>
                            ))}
                          </div>
                          <div className="row-actions">
                            <button
                              className="primary-button"
                              type="button"
                              onClick={() => handleConvertResearchToTodo(selectedResearch)}
                            >
                              转为 Todo
                            </button>
                            <button
                              className="secondary-button"
                              type="button"
                              onClick={() => handleAddResearchToMindMap(selectedResearch)}
                            >
                              加入脑图
                            </button>
                          </div>
                        </div>
                      ) : (
                        <div className="empty-state">暂无研究草稿，请先生成价值发现。</div>
                      )}
                    </section>
                  </section>

                  <aside className="research-side-stack">
                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Drafts</p>
                          <h3>研究草稿</h3>
                        </div>
                      </div>
                      <div className="research-draft-list">
                        {semanticWorkbench.deepResearch.map((research) => (
                          <button
                            key={research.id}
                            className={`research-draft-row ${
                              selectedResearch?.id === research.id ? "research-draft-row-active" : ""
                            }`}
                            type="button"
                            onClick={() => setSelectedResearchId(research.id)}
                          >
                            <strong>{research.question}</strong>
                            <span>
                              {research.sourceSpanRefs.length} 个来源 · {research.mindMapNodeId ? "已入脑图" : "未入脑图"}
                            </span>
                          </button>
                        ))}
                        {semanticWorkbench.deepResearch.length === 0 ? (
                          <div className="empty-state">暂无草稿</div>
                        ) : null}
                      </div>
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Trace</p>
                          <h3>来源片段状态</h3>
                        </div>
                      </div>
                      <ul className="compact-list">
                        <li>
                          <span>修正文稿</span>
                          <strong>{semanticWorkbench.revisions.length}</strong>
                        </li>
                        <li>
                          <span>研究草稿</span>
                          <strong>{semanticWorkbench.deepResearch.length}</strong>
                        </li>
                        <li>
                          <span>脑图</span>
                          <strong>{semanticWorkbench.mindMap ? `v${semanticWorkbench.mindMap.version}` : "无"}</strong>
                        </li>
                      </ul>
                    </section>
                  </aside>
                </section>
              </main>
            ) : null}

            {activeTab === "mindmap" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">MindMap</p>
                    <h2>思维脑图</h2>
                  </div>
                  <div className="heading-actions">
                    <span className="status-chip">
                      {currentMindMap?.edited ? `已编辑 v${currentMindMap.version}` : `生成版 v${currentMindMap?.version ?? 0}`}
                    </span>
                    <button
                      className="primary-button"
                      type="button"
                      onClick={handleGenerateMindMap}
                      disabled={mindMapLoading}
                    >
                      {mindMapLoading ? "生成中" : "生成 / 重新生成"}
                    </button>
                  </div>
                </div>

                <section className="mindmap-layout">
                  <section className="panel-lite mindmap-canvas-panel">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">Canvas</p>
                        <h3>{currentMindMap?.nodes.find((node) => node.id === currentMindMap.root)?.label ?? "暂无脑图"}</h3>
                      </div>
                      <span className="badge badge-waiting">
                        {currentMindMap?.sourceSpans.length ?? 0} 个来源
                      </span>
                    </div>
                    {currentMindMap ? (
                        <div className="mindmap-edge-strip" aria-label="脑图边关系">
                          {currentMindMap.edges.map((edge) => (
                            <span key={edge.id}>
                              {edge.from} → {edge.to} · {edge.label}
                            </span>
                          ))}
                        </div>
                    ) : null}
                    {currentMindMap ? (
                      <div className="mindmap-canvas" aria-label="思维脑图画布">
                        {currentMindMap.nodes.map((node) => {
                          const isRoot = node.id === currentMindMap.root;
                          const isSelected = node.id === selectedMindMapNodeId;
                          return (
                            <button
                              key={node.id}
                              className={`mindmap-node mindmap-node-${node.kind} ${
                                isRoot ? "mindmap-node-root" : ""
                              } ${isSelected ? "mindmap-node-selected" : ""}`}
                              type="button"
                              onClick={() => setSelectedMindMapNodeId(node.id)}
                              title={node.sourceSpanRefs.join("、") || "暂无来源"}
                            >
                              <span>{node.label}</span>
                              <small>{node.kind} · {node.collapsed ? "已折叠" : "展开"}</small>
                            </button>
                          );
                        })}
                      </div>
                    ) : (
                      <div className="empty-state">暂无脑图，请先生成思维脑图。</div>
                    )}
                  </section>

                  <aside className="mindmap-side-stack">
                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Node Editor</p>
                          <h3>节点编辑</h3>
                        </div>
                        {selectedMindMapNode ? (
                          <button
                            className="secondary-button"
                            type="button"
                            onClick={() =>
                              handleToggleMindMapNode(
                                selectedMindMapNode.id,
                                !selectedMindMapNode.collapsed,
                              )
                            }
                          >
                            {selectedMindMapNode.collapsed ? "展开" : "折叠"}
                          </button>
                        ) : null}
                      </div>
                      {selectedMindMapNode ? (
                        <div className="mindmap-editor">
                          <label className="field field-wide">
                            <span>节点标题</span>
                            <input
                              type="text"
                              value={mindMapDraft.label}
                              onChange={(event) =>
                                setMindMapDraft((current) => ({
                                  ...current,
                                  label: event.target.value,
                                }))
                              }
                            />
                          </label>
                          <label className="field field-wide">
                            <span>节点说明</span>
                            <textarea
                              value={mindMapDraft.note}
                              onChange={(event) =>
                                setMindMapDraft((current) => ({
                                  ...current,
                                  note: event.target.value,
                                }))
                              }
                            />
                          </label>
                          <div className="source-ref-list">
                            {selectedMindMapNode.sourceSpanRefs.map((source) => (
                              <button
                                key={source}
                                className="source-ref"
                                type="button"
                                onClick={() => setActiveTab("transcript")}
                                title="跳转到转写评估页查看来源"
                              >
                                {source}
                              </button>
                            ))}
                          </div>
                          <button
                            className="primary-button"
                            type="button"
                            onClick={handleSaveMindMapNode}
                          >
                            保存为新版本
                          </button>
                        </div>
                      ) : (
                        <div className="empty-state">请选择一个节点。</div>
                      )}
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Export</p>
                          <h3>导出</h3>
                        </div>
                        <div className="row-actions">
                          <button className="secondary-button" type="button" onClick={() => handleExportMindMap("markdown")}>
                            Markdown
                          </button>
                          <button className="secondary-button" type="button" onClick={() => handleExportMindMap("json")}>
                            JSON
                          </button>
                        </div>
                      </div>
                      {mindMapExport ? (
                        <div className="export-preview">
                          <strong>{mindMapExport.fileName}</strong>
                          <pre>{mindMapExport.content}</pre>
                        </div>
                      ) : (
                        <div className="empty-state">导出后会在这里显示可复用内容。</div>
                      )}
                    </section>
                  </aside>
                </section>
              </main>
            ) : null}

            {activeTab === "export" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">Export Center</p>
                    <h2>导出中心</h2>
                  </div>
                  <div className="heading-actions">
                    <span className="status-chip">
                      {exportBundle ? `${exportReadyCount} 个格式就绪` : "等待生成"}
                    </span>
                    <button
                      className="primary-button"
                      type="button"
                      onClick={handleGenerateExportBundle}
                      disabled={exportLoading}
                    >
                      {exportLoading ? "生成中" : "生成导出包"}
                    </button>
                  </div>
                </div>

                <section className="panel-lite translation-control-panel">
                  <div>
                    <p className="section-kicker">Translation</p>
                    <h3>翻译与多语言导出</h3>
                    <p className="runtime-message">
                      翻译结果作为 `translation` 语义产物保存，来源回链到转写片段；摘要翻译不会覆盖原始摘要。
                    </p>
                  </div>
                  <div className="translation-actions">
                    <label className="field">
                      <span>目标语言</span>
                      <select
                        value={selectedTargetLanguage}
                        onChange={(event) => setSelectedTargetLanguage(event.target.value)}
                      >
                        <option value="en-US">English / en-US</option>
                        <option value="ja-JP">日本語 / ja-JP</option>
                        <option value="ko-KR">한국어 / ko-KR</option>
                      </select>
                    </label>
                    <button
                      className="secondary-button"
                      type="button"
                      onClick={handleGenerateTranslation}
                      disabled={translationLoading}
                    >
                      {translationLoading ? "翻译中" : "生成翻译"}
                    </button>
                    <button
                      className="primary-button"
                      type="button"
                      onClick={handleGenerateMultilingualExportBundle}
                      disabled={exportLoading}
                    >
                      多语言导出
                    </button>
                  </div>
                </section>

                <section className="export-layout">
                  <section className="panel-lite export-main-panel">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">Local Bundle</p>
                        <h3>{exportBundle?.sessionId ?? latestSession?.id ?? "暂无会话"}</h3>
                      </div>
                      <span className="badge badge-completed">
                        {exportBundle?.provider ?? "local_file"}
                      </span>
                    </div>

                    <div className="export-format-tabs">
                      {(exportBundle?.items ?? []).map((item) => (
                        <button
                          key={item.id}
                          className={`export-format-tab ${
                            selectedExportItem?.id === item.id ? "export-format-tab-active" : ""
                          }`}
                          type="button"
                          onClick={() => setSelectedExportFormat(item.format)}
                        >
                          <span>{getExportFormatLabel(item.format)}</span>
                          <small>{item.status === "succeeded" ? item.fileName : item.errorMessage}</small>
                        </button>
                      ))}
                    </div>

                    {selectedExportItem ? (
                      <div className="export-preview export-preview-large">
                        <div className="export-file-meta">
                          <strong>{selectedExportItem.fileName}</strong>
                          <span>{selectedExportItem.mimeType}</span>
                          <span>{selectedExportItem.sourceSpanRefs.length} 个来源片段</span>
                        </div>
                        <pre>{selectedExportItem.content}</pre>
                      </div>
                    ) : (
                      <div className="empty-state">点击生成导出包后显示 Markdown、SRT、JSON 和快照预览。</div>
                    )}
                  </section>

                  <aside className="export-side-stack">
                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Snapshot</p>
                          <h3>本地分享快照</h3>
                        </div>
                        <span className="badge badge-waiting">
                          {exportBundle?.snapshot ? "已生成" : "未生成"}
                        </span>
                      </div>
                      <p className="runtime-message">
                        {exportBundle?.privacySummary ?? "导出内容只在本机生成，不上传音频、完整路径或密钥。"}
                      </p>
                      <ul className="compact-list">
                        <li><span>快照文件</span><strong>{exportBundle?.snapshot?.fileName ?? "待生成"}</strong></li>
                        <li><span>快照标题</span><strong>{exportBundle?.snapshot?.title ?? "声记分享快照"}</strong></li>
                        <li><span>来源覆盖</span><strong>{exportBundle?.snapshot?.sourceSpanRefs.length ?? 0} 个片段</strong></li>
                      </ul>
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">AI Artifacts</p>
                          <h3>状态、来源与重试</h3>
                        </div>
                      </div>
                      <div className="artifact-export-list">
                        {semanticWorkbench.artifacts.map((artifact) => (
                          <article key={artifact.id} className="artifact-export-row">
                            <div>
                              <strong>{artifact.artifactType}</strong>
                              <span>{artifact.provider} · {artifact.modelName} · {artifact.sourceSpanRefs.length} 来源</span>
                              {artifact.errorMessage ? <p>{artifact.errorMessage}</p> : null}
                            </div>
                            {artifact.status === "failed" ? (
                              <button
                                className="secondary-button"
                                type="button"
                                onClick={() => handleRetrySemanticArtifact(artifact)}
                              >
                                重试
                              </button>
                            ) : (
                              <span
                                className={`badge ${
                                  artifact.status === "succeeded" ? "badge-completed" : "badge-waiting"
                                }`}
                              >
                                {artifact.status}
                              </span>
                            )}
                          </article>
                        ))}
                      </div>
                    </section>

                    <section className="panel-lite">
                      <div className="panel-head">
                        <div>
                          <p className="section-kicker">Trace</p>
                          <h3>翻译来源追溯</h3>
                        </div>
                        <span className="badge badge-waiting">
                          {activeTranslation?.targetLanguage ?? "未生成"}
                        </span>
                      </div>
                      {activeTranslation ? (
                        <div className="translation-trace-list">
                          <article>
                            <strong>{activeTranslation.summaryTranslation.translatedTitle}</strong>
                            <p>{activeTranslation.summaryTranslation.translatedBasis}</p>
                            <span>来源：{activeTranslation.summaryTranslation.sourceArtifactType}</span>
                          </article>
                          {activeTranslation.transcriptTranslations.slice(0, 4).map((segment) => (
                            <article key={segment.sourceSegmentId}>
                              <strong>{segment.sourceSegmentId} · {segment.speakerLabel}</strong>
                              <p>{segment.translatedText}</p>
                              <span>{formatDuration(segment.startMs)} - {formatDuration(segment.endMs)}</span>
                            </article>
                          ))}
                        </div>
                      ) : (
                        <div className="empty-state">先生成目标语言翻译，随后可追溯到原文片段。</div>
                      )}
                    </section>
                  </aside>
                </section>
              </main>
            ) : null}

            {activeTab === "history" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">History / Logs</p>
                    <h2>会话文稿与提取日志</h2>
                  </div>
                  <button className="secondary-button" type="button" onClick={handleFlushSession}>
                    手动刷新当前会话
                  </button>
                </div>
                <section className="panel-lite archive-search-panel">
                  <label className="field field-wide">
                    <span>搜索会话归档</span>
                    <input
                      type="search"
                      value={sessionSearch}
                      onChange={(event) => setSessionSearch(event.target.value)}
                      placeholder="按会话 ID、文稿、Provider 或触发原因搜索"
                    />
                  </label>
                  <div className="archive-metrics">
                    <span>{filteredSessions.length} / {sessions.length} 个会话</span>
                    <span>{failedSessionCount} 个失败项</span>
                    <span>{sessions.filter((session) => session.extractionFallbackUsed).length} 个回退项</span>
                  </div>
                </section>
                <section className="session-list">
                  {filteredSessions.map((session) => (
                    <article key={session.id} className="session-card">
                      <div className="todo-card-header">
                        <div>
                          <h3>{session.id}</h3>
                          <p>{session.startedAt} - {session.endedAt}</p>
                        </div>
                        <span
                          className={`badge ${
                            session.extractionStatus === "success"
                              ? "badge-completed"
                              : session.extractionStatus === "failed"
                                ? "badge-failed"
                                : "badge-waiting"
                          }`}
                        >
                          {extractionStatusLabelMap[session.extractionStatus]}
                        </span>
                      </div>
                      <p>{session.mergedText}</p>
                      <div className="log-grid">
                        <span>触发：{session.triggerReason}</span>
                        <span>Provider：{session.extractionProviderUsed}</span>
                        <span>{session.extractionFallbackUsed ? "已云端兜底" : "未兜底"}</span>
                        <span>原因：{getFallbackReasonText(session)}</span>
                      </div>
                    </article>
                  ))}
                  {filteredSessions.length === 0 ? (
                    <div className="empty-state">没有匹配的归档会话。</div>
                  ) : null}
                </section>
              </main>
            ) : null}

            {activeTab === "system" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">System States</p>
                    <h2>运行状态与排障</h2>
                  </div>
                  <span className={`status-chip ${failedSessionCount > 0 ? "chip-danger" : "chip-live"}`}>
                    {failedSessionCount > 0 ? `${failedSessionCount} 个失败项` : "无阻断问题"}
                  </span>
                </div>
                <section className="system-grid">
                  <article className="panel-lite">
                    <p className="section-kicker">桌面桥接</p>
                    <ul className="compact-list">
                      <li><span>运行容器</span><strong>{desktopContext?.runtime ?? "浏览器原型"}</strong></li>
                      <li><span>系统平台</span><strong>{desktopContext?.platform ?? "web"}</strong></li>
                      <li><span>录音接入</span><strong>{desktopContext?.recorderStatus ?? "未接入"}</strong></li>
                      <li><span>数据存储</span><strong>{desktopContext?.storageStatus ?? "localStorage"}</strong></li>
                    </ul>
                  </article>
                  <article className="panel-lite">
                    <p className="section-kicker">Todo 语义入口</p>
                    <ul className="compact-list">
                      <li><span>Provider</span><strong>MiniMax M3</strong></li>
                      <li><span>产物类型</span><strong>todo_extraction</strong></li>
                      <li><span>状态</span><strong>语义边界已登记</strong></li>
                    </ul>
                    <p className="runtime-message">{desktopContext?.modelsStatus ?? "MiniMax M3 语义入口已固定"}</p>
                  </article>
                  <article className="panel-lite system-wide">
                    <p className="section-kicker">失败与回退</p>
                    <div className="session-list compact-session-list">
                      {sessions.filter((session) => session.extractionStatus === "failed" || session.extractionFallbackUsed).map((session) => (
                        <article key={session.id} className="session-card">
                          <div className="todo-card-header">
                            <h3>{session.id}</h3>
                            <span className="badge badge-failed">
                              {session.extractionFallbackUsed ? "发生回退" : "提取失败"}
                            </span>
                          </div>
                          <p>{getFallbackReasonText(session)}</p>
                        </article>
                      ))}
                      {failedSessionCount === 0 && !sessions.some((session) => session.extractionFallbackUsed) ? (
                        <div className="empty-state">暂无失败或回退记录</div>
                      ) : null}
                    </div>
                  </article>
                </section>
              </main>
            ) : null}

            {activeTab === "settings" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">Settings · {appVersionLabel}</p>
                    <h2>工作台设置</h2>
                    <p className="page-subtitle">配置语音、语义、知识与隐私边界，保持本地优先且生产可用。</p>
                  </div>
                  <div className="heading-actions">
                    <span className="status-chip chip-live">{appVersionLabel} 生产可用</span>
                    <button className="primary-button" onClick={saveSettings} type="button">
                      保存设置
                    </button>
                  </div>
                </div>

                <section className="settings-grid-wide">
                  <section className="panel-lite settings-provider-card">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">录音设置</p>
                        <h3>基础控制</h3>
                      </div>
                    </div>
                    <div className="settings-grid">
                      <label className="field checkbox-field">
                        <span>开启环境音录制</span>
                        <input type="checkbox" checked={settings.recordEnabled} onChange={(event) => handleSettingsChange("recordEnabled", event.target.checked)} />
                      </label>
                      <label className="field">
                        <span>切片时长（秒）</span>
                        <input type="number" min={1} value={settings.chunkSeconds} onChange={(event) => handleSettingsChange("chunkSeconds", Number(event.target.value))} />
                      </label>
                      <label className="field">
                        <span>无有效录音触发（秒）</span>
                        <input type="number" min={1} value={settings.idleTriggerSeconds} onChange={(event) => handleSettingsChange("idleTriggerSeconds", Number(event.target.value))} />
                      </label>
                      <label className="field">
                        <span>语言</span>
                        <input type="text" value={settings.language} onChange={(event) => handleSettingsChange("language", event.target.value)} />
                      </label>
                    </div>
                  </section>

                  <section className="panel-lite settings-provider-card">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">语音转写模型</p>
                        <h3>本地优先 ASR</h3>
                      </div>
                      <span className="status-chip chip-live">本地运行</span>
                      <button className="secondary-button" type="button" onClick={() => handleModelTest("asr")} disabled={testingProvider === "asr"}>
                        {testingProvider === "asr" ? "测试中..." : "测试连接"}
                      </button>
                    </div>
                    <div className="settings-grid">
                      <label className="field">
                        <span>转写模式</span>
                        <select value={settings.asrProviderType} onChange={(event) => handleSettingsChange("asrProviderType", event.target.value as SettingsState["asrProviderType"])}>
                          <option value="local_whisperkit">本地 WhisperKit / Argmax</option>
                          <option value="cloud_volc">火山云端 ASR</option>
                        </select>
                      </label>
                      <label className="field"><span>提交地址</span><input type="url" value={settings.asrSubmitUrl} onChange={(event) => handleSettingsChange("asrSubmitUrl", event.target.value)} /></label>
                      <label className="field"><span>查询地址</span><input type="url" value={settings.asrQueryUrl} onChange={(event) => handleSettingsChange("asrQueryUrl", event.target.value)} /></label>
                      <label className="field"><span>资源 ID</span><input type="text" value={settings.asrResourceId} onChange={(event) => handleSettingsChange("asrResourceId", event.target.value)} /></label>
                      <label className="field"><span>模型类型</span><input type="text" value={settings.asrModelName} onChange={(event) => handleSettingsChange("asrModelName", event.target.value)} /></label>
                      <label className="field field-wide"><span>API Key</span><input type="password" value={settings.asrApiKeyMasked} onChange={(event) => handleSettingsChange("asrApiKeyMasked", event.target.value)} /></label>
                      <label className="field checkbox-field"><span>本地 ASR 不可用时允许云端兜底</span><input type="checkbox" checked={settings.allowCloudFallback} onChange={(event) => handleSettingsChange("allowCloudFallback", event.target.checked)} /></label>
                    </div>
                    <div className="runtime-hint">
                      <p className="section-kicker">本地优先策略</p>
                      <p>
                        当前本地 WhisperKit / Argmax 已纳入生产闭环。关闭兜底时不会上传音频。
                      </p>
                    </div>
                  </section>

                  <section className="panel-lite settings-provider-card settings-cloud-card">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">语义理解与隐私边界</p>
                        <h3>MiniMax M3 工作台基座</h3>
                      </div>
                      <span className="status-chip chip-live">云端服务</span>
                    </div>
                    <div className="settings-grid settings-grid-three">
                      <label className="field">
                        <span>说话人 Provider</span>
                        <select value={settings.speakerProviderType} onChange={(event) => handleSettingsChange("speakerProviderType", event.target.value as SettingsState["speakerProviderType"])}>
                          <option value="local_speakerkit">本地 SpeakerKit</option>
                        </select>
                      </label>
                      <label className="field">
                        <span>语义 Provider</span>
                        <select value={settings.semanticProviderType} onChange={(event) => handleSettingsChange("semanticProviderType", event.target.value as SettingsState["semanticProviderType"])}>
                          <option value="minimax_m3">MiniMax M3</option>
                        </select>
                      </label>
                      <label className="field">
                        <span>导出 Provider</span>
                        <select value={settings.exportProviderType} onChange={(event) => handleSettingsChange("exportProviderType", event.target.value as SettingsState["exportProviderType"])}>
                          <option value="local_file">本地文件导出</option>
                        </select>
                      </label>
                      <label className="field"><span>M3 调用地址</span><input type="url" value={settings.semanticBaseUrl} onChange={(event) => handleSettingsChange("semanticBaseUrl", event.target.value)} /></label>
                      <label className="field"><span>M3 模型</span><input type="text" value={settings.semanticModelName} onChange={(event) => handleSettingsChange("semanticModelName", event.target.value)} /></label>
                      <label className="field"><span>M3 API Key</span><input type="password" value={settings.semanticApiKeyMasked} onChange={(event) => handleSettingsChange("semanticApiKeyMasked", event.target.value)} /></label>
                      <label className="field">
                        <span>Embedding Provider</span>
                        <select value={settings.embeddingProviderType} onChange={(event) => handleSettingsChange("embeddingProviderType", event.target.value as SettingsState["embeddingProviderType"])}>
                          <option value="reserved">预留，不启用</option>
                        </select>
                      </label>
                    </div>
                    <div className="privacy-boundary-grid">
                      <div>
                        <strong>本地</strong>
                        <p>音频转写与说话人分离默认留在本机，导出也只使用本地 SQLite 产物。</p>
                      </div>
                      <div>
                        <strong>云端</strong>
                        <p>MiniMax M3 只接收转写后的文本上下文，用于摘要、Todo、脑图和研究。</p>
                      </div>
                      <div>
                        <strong>预留</strong>
                        <p>Embedding 保持预留；本地导出已在生产闭环启用。</p>
                      </div>
                    </div>
                    <div className="provider-status-grid">
                      <div>
                        <span>MiniMax M3 成本</span>
                        <strong>按云端 token 计费</strong>
                        <p>摘要、纪要、Todo、脑图、研究共享同一语义入口。</p>
                      </div>
                      <div>
                        <span>本地导出成本</span>
                        <strong>无外部调用</strong>
                        <p>Markdown、SRT、JSON 和快照只使用本地 SQLite 产物。</p>
                      </div>
                      <div>
                        <span>密钥状态</span>
                        <strong>{settings.semanticApiKeyMasked ? "已配置 M3 Key" : "未配置 M3 Key"}</strong>
                        <p>{settings.asrApiKeyMasked ? "ASR Key 已脱敏保存。" : "ASR Key 未配置；本地 ASR 可继续使用。"}</p>
                      </div>
                      <div>
                        <span>隐私说明</span>
                        <strong>导出不上传</strong>
                        <p>日志和导出记录不展示完整音频路径、API Key 或完整隐私文本。</p>
                      </div>
                    </div>
                  </section>

                  <section className="panel-lite settings-provider-card settings-wide">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">Todo 语义产物</p>
                        <h3>MiniMax M3 候选入口</h3>
                      </div>
                      <button className="secondary-button" type="button" onClick={() => handleModelTest("todo")} disabled={testingProvider === "todo"}>
                        {testingProvider === "todo" ? "测试中..." : "测试连接"}
                      </button>
                    </div>
                    <div className="settings-grid settings-grid-three">
                      <label className="field">
                        <span>提取模式</span>
                        <select value={settings.todoProviderType} onChange={(event) => handleSettingsChange("todoProviderType", event.target.value as SettingsState["todoProviderType"])}>
                          <option value="semantic_m3">MiniMax M3 语义边界</option>
                        </select>
                      </label>
                      <label className="field"><span>语义模型</span><input type="text" value={settings.semanticModelName} readOnly /></label>
                      <label className="field field-wide"><span>产物落库</span><input type="text" value="semantic_artifacts(type='todo_extraction')" readOnly /></label>
                    </div>
                    <div className="runtime-hint">
                      <p className="section-kicker">语义入口</p>
                      <p>当前生产版已将 Todo 候选、纪要、脑图和导出闭环统一纳入 MiniMax M3 语义产物。</p>
                      <p>Todo 候选统一通过 MiniMax M3 语义链路承载，产物落库到 semantic_artifacts。</p>
                    </div>
                  </section>

                  <section className="panel-lite settings-version-card">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">版本信息</p>
                        <h3>Shengji Mac {appVersionLabel}</h3>
                      </div>
                      <button className="secondary-button" type="button" onClick={() => setActiveTab("system")}>
                        检查状态
                      </button>
                    </div>
                    <p>架构模式：本地优先 · 云端增强。主流程已覆盖录音、转写、修正、摘要、Todo、脑图、研究、翻译和导出。</p>
                  </section>
                </section>
              </main>
            ) : null}
          </section>
        </div>
        <footer className="window-statusbar" aria-label="运行摘要">
          <div className="statusbar-group">
            <span className={`statusbar-item ${isRecordingActive ? "statusbar-recording" : ""}`}>
              <span className="status-dot" />
              录音：{runtime.runtimeLabel}
            </span>
            <button className="statusbar-button" type="button" onClick={() => handleRecordingAction(recordingControlAction)}>
              {recordingControlLabel}
            </button>
            <span className="statusbar-item">
              会话：{sessionStatusLabelMap[runtime.currentSessionStatus]}
            </span>
            <button className="statusbar-button" type="button" onClick={() => setActiveTab("semantic")}>
              语义：MiniMax M3
            </button>
          </div>
          <div className="statusbar-group statusbar-group-right">
            <button
              className={`statusbar-button ${failedSessionCount > 0 ? "statusbar-danger" : ""}`}
              type="button"
              onClick={() => setActiveTab("system")}
            >
              失败 {failedSessionCount}
            </button>
            <button className="statusbar-button" type="button" onClick={() => setActiveTab("actions")}>
              候选 {proposedCandidateCount}
            </button>
            <span className="statusbar-item">
              本地模型：{transcriptReview.modelStatus.offlineAvailable ? "可用" : "未就绪"}
            </span>
            <button className="statusbar-button statusbar-version" type="button" onClick={() => setActiveTab("settings")}>
              {appVersionLabel}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}

export default App;
