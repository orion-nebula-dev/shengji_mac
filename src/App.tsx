import { useEffect, useMemo, useState } from "react";
import {
  flushDesktopSession,
  importDesktopLocalAudio,
  loadBootstrapData,
  loadDesktopContext,
  loadTranscriptReview,
  markDesktopTranscriptSegment,
  processDesktopPendingJobs,
  renameDesktopSpeaker,
  retryDesktopTranscriptJob,
  saveDesktopSettings,
  simulateDesktopAudioSlice,
  startDesktopRecording,
  stopDesktopRecording,
  testDesktopModelConnection,
  toggleDesktopTodoStatus,
} from "./lib/desktop";
import { defaultTranscriptReview } from "./data/mock";
import { getDefaultState, loadState, saveState } from "./lib/storage";
import type { SessionItem, SettingsState, TodoItem, TranscriptReview } from "./types";

type TabKey = "overview" | "actions" | "transcript" | "history" | "system" | "settings";

const statusLabelMap = {
  pending: "未完成",
  completed: "已完成",
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

const transcriptJobStatusLabelMap = {
  queued: "已排队",
  running: "转写中",
  succeeded: "已完成",
  failed: "失败可重试",
  retrying: "重试中",
} as const;

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

function App() {
  const manualFlushCooldownMs = 10_000;
  const initialState = useMemo(() => loadState(), []);
  const fallbackState = useMemo(() => getDefaultState(), []);
  const [activeTab, setActiveTab] = useState<TabKey>("overview");
  const [settings, setSettings] = useState<SettingsState>(initialState.settings);
  const [todos, setTodos] = useState<TodoItem[]>(initialState.todos);
  const [sessions, setSessions] = useState<SessionItem[]>(initialState.sessions);
  const [runtime, setRuntime] = useState(initialState.runtime);
  const [selectedTodoId, setSelectedTodoId] = useState(initialState.todos[0]?.id ?? "");
  const [filter, setFilter] = useState<"all" | "pending" | "completed">("all");
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
      [todo.title, todo.note].some((field) =>
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

    if (persisted) {
      setSettings(persisted);
    }

    setSaveBanner("设置已保存，下一轮切片与提取将使用新配置。");
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
              status: todo.status === "pending" ? "completed" : "pending",
            }
          : todo,
      ),
    );
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
      setSaveBanner("请输入本地音频文件路径，用于 v0.5 离线评估。");
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

  const pendingTodoCount = todos.filter((todo) => todo.status === "pending").length;
  const completedTodoCount = todos.filter((todo) => todo.status === "completed").length;
  const failedSessionCount = sessions.filter((session) => session.extractionStatus === "failed").length;
  const latestSession = sessions[0];
  const navItems: Array<{ key: TabKey; label: string; description: string }> = [
    { key: "overview", label: "今日工作台", description: "录音与概览" },
    { key: "actions", label: "行动中心", description: "Todo 执行" },
    { key: "transcript", label: "转写评估", description: "音频与说话人" },
    { key: "history", label: "会话日志", description: "文稿与来源" },
    { key: "system", label: "系统状态", description: "排障与运行时" },
    { key: "settings", label: "设置", description: "模型与录音" },
  ];

  return (
    <div className="desktop-shell">
      <div className="window-frame">
        <header className="window-titlebar">
          <div className="traffic-lights" aria-hidden="true">
            <span className="traffic-dot traffic-close" />
            <span className="traffic-dot traffic-minimize" />
            <span className="traffic-dot traffic-maximize" />
          </div>
          <div className="window-title">
            <strong>声记</strong>
            <span>语音知识与行动工作台</span>
          </div>
          <div className="titlebar-actions">
            <button className="icon-button" type="button" onClick={() => setActiveTab("history")}>
              搜索
            </button>
            <button className="icon-button" type="button" onClick={() => setActiveTab("settings")}>
              设置
            </button>
          </div>
        </header>

        <div className="window-body">
          <aside className="sidebar">
            <div className="brand-block">
              <p className="section-kicker">ShengJi</p>
              <h1>今日工作流</h1>
              <span className={`status-chip ${settings.recordEnabled ? "chip-live" : ""}`}>
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

            <section className="sidebar-card">
              <p className="section-kicker">运行摘要</p>
              <ul className="compact-list">
                <li>
                  <span>会话状态</span>
                  <strong>{sessionStatusLabelMap[runtime.currentSessionStatus]}</strong>
                </li>
                <li>
                  <span>语义入口</span>
                  <strong>MiniMax M3</strong>
                </li>
                <li>
                  <span>失败任务</span>
                  <strong>{failedSessionCount}</strong>
                </li>
              </ul>
            </section>
          </aside>

          <section className="content-area">
            {saveBanner ? <div className="system-banner">{saveBanner}</div> : null}

            {activeTab === "overview" ? (
              <main className="page-stack">
                <div className="page-heading">
                  <div>
                    <p className="section-kicker">Now / Today</p>
                    <h2>录音状态与今日概览</h2>
                  </div>
                  <div className="heading-actions">
                    <button
                      className="primary-button"
                      type="button"
                      onClick={() => handleRecordingAction("start")}
                    >
                      启动录音
                    </button>
                    <button
                      className="secondary-button"
                      type="button"
                      onClick={() => handleRecordingAction("stop")}
                    >
                      停止录音
                    </button>
                  </div>
                </div>

                <section className="metrics-grid">
                  <article className="metric-card">
                    <span>待办</span>
                    <strong>{pendingTodoCount}</strong>
                    <p>等待处理的行动项</p>
                  </article>
                  <article className="metric-card">
                    <span>已完成</span>
                    <strong>{completedTodoCount}</strong>
                    <p>已标记完成的 Todo</p>
                  </article>
                  <article className="metric-card">
                    <span>最近切片</span>
                    <strong>{runtime.lastSliceAt}</strong>
                    <p>录音切片与会话聚合入口</p>
                  </article>
                  <article className="metric-card">
                    <span>最近提取</span>
                    <strong>{runtime.lastExtractionAt}</strong>
                    <p>{runtime.lastExtractionSummary}</p>
                  </article>
                </section>

                <section className="overview-grid">
                  <article className="panel-lite recording-panel">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">Recording</p>
                        <h3>录音控制</h3>
                      </div>
                      <span className="status-chip chip-live">
                        {sessionStatusLabelMap[runtime.currentSessionStatus]}
                      </span>
                    </div>
                    <div className="control-grid">
                      <button className="secondary-button" type="button" onClick={() => handleRecordingAction("effective")}>
                        模拟有效切片
                      </button>
                      <button className="secondary-button" type="button" onClick={() => handleRecordingAction("silent")}>
                        模拟静默切片
                      </button>
                      <button className="secondary-button" type="button" onClick={handleProcessPendingJobs}>
                        处理待办任务
                      </button>
                      <button className="secondary-button" type="button" onClick={handleFlushSession}>
                        手动刷新会话
                      </button>
                    </div>
                  </article>

                  <article className="panel-lite">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">Latest Session</p>
                        <h3>上一段会话</h3>
                      </div>
                      <button className="text-button" type="button" onClick={() => setActiveTab("history")}>
                        查看日志
                      </button>
                    </div>
                    {latestSession ? (
                      <div className="session-preview">
                        <p>{latestSession.mergedText}</p>
                        <div className="todo-meta">
                          <span>{latestSession.startedAt}</span>
                          <span>{extractionStatusLabelMap[latestSession.extractionStatus]}</span>
                          <span>{latestSession.extractionProviderUsed}</span>
                        </div>
                      </div>
                    ) : (
                      <div className="empty-state">暂无会话文稿</div>
                    )}
                  </article>
                </section>
              </main>
            ) : null}

            {activeTab === "actions" ? (
              <main className="actions-layout">
                <section className="panel-lite todo-list-panel">
                  <div className="panel-head">
                    <div>
                      <p className="section-kicker">Actions</p>
                      <h2>Todo 执行中心</h2>
                    </div>
                    <input
                      className="search-input"
                      placeholder="搜索标题或备注"
                      value={keyword}
                      onChange={(event) => setKeyword(event.target.value)}
                    />
                  </div>
                  <div className="filter-row">
                    {[
                      ["all", "全部"],
                      ["pending", "未完成"],
                      ["completed", "已完成"],
                    ].map(([key, label]) => (
                      <button
                        key={key}
                        className={`filter-chip ${filter === key ? "filter-chip-active" : ""}`}
                        onClick={() => setFilter(key as "all" | "pending" | "completed")}
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
                          <span className={`badge ${todo.status === "pending" ? "badge-pending" : "badge-completed"}`}>
                            {statusLabelMap[todo.status]}
                          </span>
                        </div>
                        <p>{todo.note}</p>
                        <div className="todo-meta">
                          <span>{todo.createdAt}</span>
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
                        <button className="primary-button" type="button" onClick={() => toggleTodoStatus(selectedTodo.id)}>
                          切换为{selectedTodo.status === "pending" ? "已完成" : "未完成"}
                        </button>
                      </div>
                      <div className="detail-block">
                        <label>状态</label>
                        <span className={`badge ${selectedTodo.status === "pending" ? "badge-pending" : "badge-completed"}`}>
                          {statusLabelMap[selectedTodo.status]}
                        </span>
                      </div>
                      <div className="detail-block">
                        <label>备注</label>
                        <p>{selectedTodo.note}</p>
                      </div>
                      <div className="detail-block">
                        <label>来源文稿</label>
                        <p>{selectedTodo.sourceText}</p>
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
                <section className="session-list">
                  {sessions.map((session) => (
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
                    <p className="section-kicker">Settings</p>
                    <h2>录音、模型与隐私设置</h2>
                  </div>
                  <button className="primary-button" onClick={saveSettings} type="button">
                    保存设置
                  </button>
                </div>

                <section className="settings-grid-wide">
                  <section className="panel-lite">
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

                  <section className="panel-lite">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">语音转写模型</p>
                        <h3>本地优先 ASR</h3>
                      </div>
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
                        当前本地 WhisperKit / Argmax 接口已进入 v0.4 边界设计，正式转写执行在 v0.5 接入。关闭兜底时不会上传音频。
                      </p>
                    </div>
                  </section>

                  <section className="panel-lite settings-wide">
                    <div className="panel-head">
                      <div>
                        <p className="section-kicker">语义理解与隐私边界</p>
                        <h3>MiniMax M3 工作台基座</h3>
                      </div>
                      <span className="status-chip">v0.4 架构边界</span>
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
                        <p>音频转写与说话人分离默认留在本机，v0.5 才接入实际 Argmax local server。</p>
                      </div>
                      <div>
                        <strong>云端</strong>
                        <p>MiniMax M3 只接收转写后的文本上下文，用于摘要、Todo、脑图和研究。</p>
                      </div>
                      <div>
                        <strong>预留</strong>
                        <p>Embedding 与导出已登记 provider 边界，但 v0.4 不启用向量检索默认路径。</p>
                      </div>
                    </div>
                  </section>

                  <section className="panel-lite settings-wide">
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
                      <p>v0.4 默认只登记 Todo 语义产物边界，实际 Todo 候选确认在 v0.7 接入。</p>
                      <p>Todo 候选统一通过 MiniMax M3 语义链路承载，产物落库到 semantic_artifacts。</p>
                    </div>
                  </section>
                </section>
              </main>
            ) : null}
          </section>
        </div>
      </div>
    </div>
  );
}

export default App;
