import { useEffect, useMemo, useState } from "react";
import {
  flushDesktopSession,
  getLocalTodoRuntimeStatus,
  loadBootstrapData,
  loadDesktopContext,
  processDesktopPendingJobs,
  saveDesktopSettings,
  simulateDesktopAudioSlice,
  startDesktopRecording,
  stopDesktopRecording,
  testDesktopModelConnection,
  toggleDesktopTodoStatus,
} from "./lib/desktop";
import { getDefaultState, loadState, saveState } from "./lib/storage";
import type { LocalRuntimeState, SessionItem, SettingsState, TodoItem } from "./types";

type TabKey = "workspace" | "settings" | "sessions";

const statusLabelMap = {
  pending: "未完成",
  completed: "已完成",
} as const;

function App() {
  const manualFlushCooldownMs = 10_000;
  const initialState = useMemo(() => loadState(), []);
  const fallbackState = useMemo(() => getDefaultState(), []);
  const [activeTab, setActiveTab] = useState<TabKey>("workspace");
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
  const [localRuntime, setLocalRuntime] = useState<LocalRuntimeState>({
    providerType: initialState.settings.todoProviderType,
    modelVersion: initialState.settings.localTodoModelVersion,
    runtimeStatus: initialState.settings.localTodoRuntimeStatus,
    lastHealthCheckAt: initialState.settings.localTodoLastHealthCheckAt,
    fallbackEnabled: initialState.settings.allowCloudFallback,
    message:
      initialState.settings.localTodoRuntimeStatus === "ready"
        ? "本地 Todo 运行时已就绪"
        : "本地 Todo 运行时未就绪",
  });
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

    getLocalTodoRuntimeStatus()
      .then((payload) => {
        if (!cancelled && payload) {
          setLocalRuntime(payload);
          setSettings((current) => ({
            ...current,
            todoProviderType: payload.providerType === "embedded_local" ? "embedded_local" : current.todoProviderType,
            localTodoModelVersion: payload.modelVersion,
            localTodoRuntimeStatus: payload.runtimeStatus,
            localTodoLastHealthCheckAt: payload.lastHealthCheckAt,
          }));
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLocalRuntime((current) => ({
            ...current,
            message: "当前浏览器原型模式不支持本地运行时状态查询",
          }));
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
        setLocalRuntime({
          providerType: payload.settings.todoProviderType,
          modelVersion: payload.settings.localTodoModelVersion,
          runtimeStatus: payload.settings.localTodoRuntimeStatus,
          lastHealthCheckAt: payload.settings.localTodoLastHealthCheckAt,
          fallbackEnabled: payload.settings.allowCloudFallback,
          message:
            payload.settings.localTodoRuntimeStatus === "ready"
              ? "本地 Todo 运行时已就绪"
              : "本地 Todo 运行时未就绪",
        });
        setTodos(payload.todos);
        setSessions(payload.sessions);
        setRuntime(payload.runtime);
        setSelectedTodoId(payload.todos[0]?.id ?? "");
      })
      .catch(() => {
        if (!cancelled) {
          setSettings(fallbackState.settings);
          setLocalRuntime({
            providerType: fallbackState.settings.todoProviderType,
            modelVersion: fallbackState.settings.localTodoModelVersion,
            runtimeStatus: fallbackState.settings.localTodoRuntimeStatus,
            lastHealthCheckAt: fallbackState.settings.localTodoLastHealthCheckAt,
            fallbackEnabled: fallbackState.settings.allowCloudFallback,
            message: "当前为浏览器原型，本地运行时状态使用默认值",
          });
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

  async function refreshLocalRuntime() {
    const payload = await getLocalTodoRuntimeStatus().catch(() => null);
    if (!payload) {
      return;
    }

    setLocalRuntime(payload);
    setSettings((current) => ({
      ...current,
      localTodoModelVersion: payload.modelVersion,
      localTodoRuntimeStatus: payload.runtimeStatus,
      localTodoLastHealthCheckAt: payload.lastHealthCheckAt,
    }));
  }

  async function saveSettings() {
    const persisted = await saveDesktopSettings(settings).catch(() => null);

    if (persisted) {
      setSettings(persisted);
    }

    await refreshLocalRuntime();

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
    if (provider === "todo") {
      await refreshLocalRuntime();
    }
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

  return (
    <div className="app-shell">
      <div className="ambient ambient-left" />
      <div className="ambient ambient-right" />
      <header className="topbar">
        <div>
          <p className="eyebrow">Smart Todo Companion</p>
          <h1>智能 Todo</h1>
        </div>
        <div className="topbar-actions">
          <span className="pill pill-live">{runtime.runtimeLabel}</span>
          <span className="pill">{runtime.currentSessionStatus}</span>
        </div>
      </header>

      <nav className="tabbar">
        {[
          ["workspace", "主工作台"],
          ["settings", "设置"],
          ["sessions", "会话文稿"],
        ].map(([key, label]) => (
          <button
            key={key}
            className={`tab ${activeTab === key ? "tab-active" : ""}`}
            onClick={() => setActiveTab(key as TabKey)}
            type="button"
          >
            {label}
          </button>
        ))}
      </nav>

      {saveBanner ? <div className="banner">{saveBanner}</div> : null}

      {activeTab === "workspace" ? (
        <main className="workspace-grid">
          <aside className="panel panel-sidebar">
            <section>
              <p className="section-kicker">视图</p>
              <div className="menu-list">
                <button className="menu-item menu-item-active" type="button">
                  全部 Todo
                </button>
                <button className="menu-item" type="button">
                  最近会话
                </button>
                <button className="menu-item" type="button">
                  失败任务
                </button>
              </div>
            </section>

            <section className="status-card">
              <p className="section-kicker">运行状态</p>
              <ul className="status-list">
                <li>
                  <span>当前状态</span>
                  <strong>{runtime.runtimeLabel}</strong>
                </li>
                <li>
                  <span>会话状态</span>
                  <strong>{runtime.currentSessionStatus}</strong>
                </li>
                <li>
                  <span>最近切片</span>
                  <strong>{runtime.lastSliceAt}</strong>
                </li>
                <li>
                  <span>最近提取</span>
                  <strong>{runtime.lastExtractionAt}</strong>
                </li>
              </ul>
              <div className="action-row">
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
                <button
                  className="secondary-button"
                  type="button"
                  onClick={() => handleRecordingAction("effective")}
                >
                  模拟有效切片
                </button>
              <button
                className="secondary-button"
                type="button"
                onClick={() => handleRecordingAction("silent")}
              >
                模拟静默切片
              </button>
              <button
                className="secondary-button"
                type="button"
                onClick={handleProcessPendingJobs}
              >
                处理待办任务
              </button>
            </div>
            </section>

            <section className="status-card">
              <p className="section-kicker">桌面桥接</p>
              <ul className="status-list">
                <li>
                  <span>运行容器</span>
                  <strong>{desktopContext?.runtime ?? "浏览器原型"}</strong>
                </li>
                <li>
                  <span>系统平台</span>
                  <strong>{desktopContext?.platform ?? "web"}</strong>
                </li>
                <li>
                  <span>录音接入</span>
                  <strong>{desktopContext?.recorderStatus ?? "未接入"}</strong>
                </li>
                <li>
                  <span>数据存储</span>
                  <strong>{desktopContext?.storageStatus ?? "localStorage"}</strong>
                </li>
                <li>
                  <span>本地提取</span>
                  <strong>{localRuntime.runtimeStatus}</strong>
                </li>
              </ul>
            </section>
          </aside>

          <section className="panel panel-list">
            <div className="list-toolbar">
              <input
                className="search-input"
                placeholder="搜索标题或备注"
                value={keyword}
                onChange={(event) => setKeyword(event.target.value)}
              />
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
                    <span
                      className={`badge ${
                        todo.status === "pending" ? "badge-pending" : "badge-completed"
                      }`}
                    >
                      {statusLabelMap[todo.status]}
                    </span>
                  </div>
                  <p>{todo.note}</p>
                  <div className="todo-meta">
                    <span>{todo.createdAt}</span>
                    <span>{todo.conversationSessionId}</span>
                  </div>
                </button>
              ))}
            </div>
          </section>

          <aside className="panel panel-detail">
            {selectedTodo ? (
              <>
                <div className="detail-header">
                  <div>
                    <p className="section-kicker">Todo 详情</p>
                    <h2>{selectedTodo.title}</h2>
                  </div>
                  <button
                    className="primary-button"
                    type="button"
                    onClick={() => toggleTodoStatus(selectedTodo.id)}
                  >
                    切换为{selectedTodo.status === "pending" ? "已完成" : "未完成"}
                  </button>
                </div>
                <div className="detail-block">
                  <label>状态</label>
                  <span
                    className={`badge ${
                      selectedTodo.status === "pending" ? "badge-pending" : "badge-completed"
                    }`}
                  >
                    {statusLabelMap[selectedTodo.status]}
                  </span>
                </div>
                <div className="detail-block">
                  <label>创建时间</label>
                  <p>{selectedTodo.createdAt}</p>
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
                  <label>最近提取摘要</label>
                  <p>{runtime.lastExtractionSummary}</p>
                </div>
                <div className="detail-block detail-runtime">
                  <label>桌面实现阶段</label>
                  <p>{desktopContext?.modelsStatus ?? "当前为前端原型阶段"}</p>
                </div>
                <div className="detail-block detail-runtime">
                  <label>本地 Todo 运行时</label>
                  <p>{localRuntime.message}</p>
                </div>
                <div className="detail-block detail-runtime">
                  <label>提取路径</label>
                  <p>
                    {selectedSession?.extractionProviderUsed ?? "未知"}
                    {" / "}
                    {selectedSession?.extractionFallbackUsed ? "发生过云端回退" : "未发生云端回退"}
                  </p>
                </div>
                <div className="detail-block detail-runtime">
                  <label>回退原因</label>
                  <p>{selectedSession?.extractionFallbackReason || "无"}</p>
                </div>
              </>
            ) : (
              <div className="empty-state">暂无 Todo 数据</div>
            )}
          </aside>
        </main>
      ) : null}

      {activeTab === "settings" ? (
        <main className="settings-layout">
          <section className="panel settings-panel">
            <div className="panel-head">
              <div>
                <p className="section-kicker">录音设置</p>
                <h2>基础控制</h2>
              </div>
              <button className="primary-button" onClick={saveSettings} type="button">
                保存设置
              </button>
            </div>

            <div className="settings-grid">
              <label className="field checkbox-field">
                <span>开启环境音录制</span>
                <input
                  type="checkbox"
                  checked={settings.recordEnabled}
                  onChange={(event) => handleSettingsChange("recordEnabled", event.target.checked)}
                />
              </label>

              <label className="field">
                <span>切片时长（秒）</span>
                <input
                  type="number"
                  min={1}
                  value={settings.chunkSeconds}
                  onChange={(event) =>
                    handleSettingsChange("chunkSeconds", Number(event.target.value))
                  }
                />
              </label>

              <label className="field">
                <span>无有效录音触发（秒）</span>
                <input
                  type="number"
                  min={1}
                  value={settings.idleTriggerSeconds}
                  onChange={(event) =>
                    handleSettingsChange("idleTriggerSeconds", Number(event.target.value))
                  }
                />
              </label>

              <label className="field">
                <span>语言</span>
                <input
                  type="text"
                  value={settings.language}
                  onChange={(event) => handleSettingsChange("language", event.target.value)}
                />
              </label>
            </div>
          </section>

          <section className="settings-columns">
            <section className="panel model-panel">
              <div className="panel-head">
                <div>
                  <p className="section-kicker">语音转写模型</p>
                  <h2>ASR Provider</h2>
                </div>
                <div className="model-actions">
                  <span className="pill">云端</span>
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() => handleModelTest("asr")}
                    disabled={testingProvider === "asr"}
                  >
                    {testingProvider === "asr" ? "测试中..." : "测试连接"}
                  </button>
                </div>
              </div>

              <div className="settings-grid">
                <label className="field">
                  <span>提交地址</span>
                  <input
                    type="url"
                    value={settings.asrSubmitUrl}
                    onChange={(event) => handleSettingsChange("asrSubmitUrl", event.target.value)}
                  />
                </label>
                <label className="field">
                  <span>查询地址</span>
                  <input
                    type="url"
                    value={settings.asrQueryUrl}
                    onChange={(event) => handleSettingsChange("asrQueryUrl", event.target.value)}
                  />
                </label>
                <label className="field">
                  <span>资源 ID</span>
                  <input
                    type="text"
                    value={settings.asrResourceId}
                    onChange={(event) => handleSettingsChange("asrResourceId", event.target.value)}
                  />
                </label>
                <label className="field">
                  <span>模型类型</span>
                  <input
                    type="text"
                    value={settings.asrModelName}
                    onChange={(event) => handleSettingsChange("asrModelName", event.target.value)}
                  />
                </label>
                <label className="field">
                  <span>API Key</span>
                  <input
                    type="password"
                    value={settings.asrApiKeyMasked}
                    onChange={(event) =>
                      handleSettingsChange("asrApiKeyMasked", event.target.value)
                    }
                  />
                </label>
              </div>
            </section>

            <section className="panel model-panel">
              <div className="panel-head">
                <div>
                  <p className="section-kicker">Todo 提取模型</p>
                  <h2>Extraction Provider</h2>
                </div>
                <div className="model-actions">
                  <span className="pill">
                    {settings.todoProviderType === "embedded_local" ? "内嵌本地" : "云端"}
                  </span>
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() => handleModelTest("todo")}
                    disabled={testingProvider === "todo"}
                  >
                    {testingProvider === "todo" ? "测试中..." : "测试连接"}
                  </button>
                </div>
              </div>

              <div className="settings-grid">
                <label className="field">
                  <span>提取模式</span>
                  <select
                    value={settings.todoProviderType}
                    onChange={(event) =>
                      handleSettingsChange(
                        "todoProviderType",
                        event.target.value as SettingsState["todoProviderType"],
                      )
                    }
                  >
                    <option value="cloud">云端模型</option>
                    <option value="embedded_local">内嵌本地</option>
                  </select>
                </label>
                <label className="field">
                  <span>调用地址</span>
                  <input
                    type="url"
                    value={settings.todoBaseUrl}
                    onChange={(event) => handleSettingsChange("todoBaseUrl", event.target.value)}
                  />
                </label>
                <label className="field">
                  <span>模型类型</span>
                  <input
                    type="text"
                    value={settings.todoModelName}
                    onChange={(event) => handleSettingsChange("todoModelName", event.target.value)}
                  />
                </label>
                <label className="field">
                  <span>API Key</span>
                  <input
                    type="password"
                    value={settings.todoApiKeyMasked}
                    onChange={(event) =>
                      handleSettingsChange("todoApiKeyMasked", event.target.value)
                    }
                  />
                </label>
                <label className="field">
                  <span>内嵌模型版本</span>
                  <input
                    type="text"
                    value={settings.localTodoModelVersion}
                    onChange={(event) =>
                      handleSettingsChange("localTodoModelVersion", event.target.value)
                    }
                  />
                </label>
                <label className="field checkbox-field">
                  <span>允许失败后回退云端</span>
                  <input
                    type="checkbox"
                    checked={settings.allowCloudFallback}
                    onChange={(event) =>
                      handleSettingsChange("allowCloudFallback", event.target.checked)
                    }
                  />
                </label>
                <label className="field">
                  <span>运行时状态</span>
                  <input type="text" value={settings.localTodoRuntimeStatus} readOnly />
                </label>
                <label className="field field-wide">
                  <span>最近健康检查</span>
                  <input type="text" value={settings.localTodoLastHealthCheckAt || "暂无"} readOnly />
                </label>
                <div className="runtime-hint">
                  <p className="section-kicker">本地运行时</p>
                  <p>{localRuntime.message}</p>
                  <p>当前实现先内置本地提取 Provider 骨架，云端配置仍保留为兜底路径。</p>
                </div>
              </div>
            </section>
          </section>
        </main>
      ) : null}

      {activeTab === "sessions" ? (
        <main className="sessions-layout">
          <section className="panel panel-session-list">
            <div className="panel-head">
              <div>
                <p className="section-kicker">最近会话</p>
                <h2>文稿与提取结果</h2>
              </div>
              <button
                className="secondary-button"
                type="button"
                onClick={handleFlushSession}
              >
                手动刷新当前会话
              </button>
            </div>

            <div className="session-list">
              {sessions.map((session) => (
                <article key={session.id} className="session-card">
                  <div className="todo-card-header">
                    <h3>{session.id}</h3>
                    <span
                      className={`badge ${
                        session.extractionStatus === "success"
                          ? "badge-completed"
                          : session.extractionStatus === "failed"
                            ? "badge-failed"
                            : "badge-waiting"
                      }`}
                    >
                      {session.extractionStatus}
                    </span>
                  </div>
                  <p>{session.mergedText}</p>
                  <div className="todo-meta">
                    <span>
                      {session.startedAt} - {session.endedAt}
                    </span>
                    <span>{session.triggerReason}</span>
                    <span>{session.extractionProviderUsed}</span>
                    <span>{session.extractionFallbackUsed ? "已回退云端" : "未回退"}</span>
                    <span>{session.extractionFallbackReason || "无回退原因"}</span>
                  </div>
                </article>
              ))}
            </div>
          </section>
        </main>
      ) : null}
    </div>
  );
}

export default App;
