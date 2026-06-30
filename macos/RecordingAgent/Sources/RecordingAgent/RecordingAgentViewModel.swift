import AppKit
import Foundation
import RecordingAgentCore

@MainActor
final class RecordingAgentViewModel: ObservableObject {
    @Published var state: RecordingAgentState = .idle
    @Published var confirmedSegments: [TranscriptSegment] = []
    @Published var history: [Recording] = []
    @Published var selectedDetail: RecordingDetail?
    @Published var errorMessage: String?
    @Published var selectedModel: ASRModelName = .default
    @Published var selectedLanguage: LanguageMode = .auto
    @Published var modelAssets: [ASRModelAsset] = []
    @Published var modelPreparationProgress: ModelPreparationProgress?
    @Published var pendingModelPreparation: ASRModelName?
    @Published var runtimeSnapshot: RecordingRuntimeSnapshot = .inactive
    @Published var databasePath: String = ""
    @Published var modelPath: String = ""
    @Published var exportStatusMessage: String?
    @Published var isExportStatusError = false
    @Published private(set) var selectedRecordingID: String?
    @Published var isPreparingModel = false
    @Published private(set) var hasActiveRecording = false
    @Published private(set) var isRecordingCommandRunning = false

    var canStartRecording: Bool {
        container != nil &&
            !isRecordingCommandRunning &&
            !isPreparingModel &&
            !hasActiveRecording &&
            state.allowsRecordingStart &&
            selectedModelAsset?.isReadyForRecording == true
    }

    var canStopRecording: Bool {
        container != nil && !isRecordingCommandRunning && hasActiveRecording && state.allowsRecordingStop
    }

    var areRecordingControlsDisabled: Bool {
        !canStartRecording && !canStopRecording
    }

    var areRecordingInputsDisabled: Bool {
        isRecordingCommandRunning || isPreparingModel || hasActiveRecording || !state.allowsRecordingStart
    }

    var selectedModelAsset: ASRModelAsset? {
        modelAssets.first { $0.modelName == selectedModel }
    }

    var canPrepareSelectedModel: Bool {
        canPrepareModel(selectedModel)
    }

    private let container: AppContainer?
    private var liveRefreshTask: Task<Void, Never>?
    private var prepareModelTask: Task<Void, Never>?
    private var openDetailTask: Task<Void, Never>?

    init() {
        do {
            self.container = try AppContainer.live()
            self.databasePath = container?.databaseURL.path ?? ""
            self.modelPath = container?.modelBaseDirectory.path ?? ""
        } catch {
            self.container = nil
            self.errorMessage = "初始化失败：\(error.localizedDescription)"
        }
    }

    func bootstrap() {
        Task {
            guard let container else { return }
            do {
                try await container.store.migrate()
                modelAssets = try await container.modelManager.listModels()
                history = try await container.store.fetchRecordings()
                await refreshFromAgent()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func start() {
        guard canStartRecording else { return }
        isRecordingCommandRunning = true
        errorMessage = nil
        Task {
            defer { isRecordingCommandRunning = false }
            guard let container else { return }
            do {
                try await container.agent.startRecording(model: selectedModel, language: selectedLanguage)
                await refreshFromAgent()
                beginLiveRefresh()
            } catch {
                await refreshFromAgent()
                errorMessage = userMessage(for: error)
                history = (try? await container.store.fetchRecordings()) ?? history
            }
            await refreshFromAgent()
        }
    }

    func requestPrepareSelectedModel() {
        guard canPrepareSelectedModel else { return }
        pendingModelPreparation = selectedModel
    }

    func cancelPendingModelPreparation() {
        pendingModelPreparation = nil
    }

    func confirmPrepareSelectedModel() {
        guard let targetModel = pendingModelPreparation else { return }
        pendingModelPreparation = nil
        selectedModel = targetModel
        prepareModel(targetModel)
    }

    func cancelPrepareSelectedModel() {
        prepareModelTask?.cancel()
    }

    func preparationProgress(for asset: ASRModelAsset) -> ModelPreparationProgress? {
        guard modelPreparationProgress?.modelName == asset.modelName else { return nil }
        return modelPreparationProgress
    }

    private func prepareModel(_ targetModel: ASRModelName) {
        guard canPrepareModel(targetModel) else { return }
        prepareModelTask = Task {
            guard let container else { return }
            isPreparingModel = true
            errorMessage = nil
            modelPreparationProgress = ModelPreparationProgress(
                modelName: targetModel,
                status: .downloading,
                completedUnitCount: 0,
                totalUnitCount: 1,
                message: "准备下载"
            )
            updateModelAsset(targetModel, status: .downloading, progress: 0)
            defer {
                isPreparingModel = false
                prepareModelTask = nil
            }
            do {
                _ = try await container.modelManager.prepare(model: targetModel, trigger: .settings) { [weak self] progress in
                    await MainActor.run {
                        guard let self else { return }
                        self.modelPreparationProgress = progress
                        self.updateModelAsset(targetModel, status: progress.status, progress: progress.percent)
                    }
                }
                modelAssets = try await container.modelManager.listModels()
                modelPreparationProgress = nil
                errorMessage = nil
            } catch is CancellationError {
                errorMessage = "已取消模型准备"
                modelPreparationProgress = nil
                modelAssets = (try? await container.modelManager.listModels()) ?? modelAssets
                updateModelAsset(targetModel, status: .notDownloaded, progress: 0)
            } catch {
                let message = userMessage(for: error)
                errorMessage = message
                modelPreparationProgress = nil
                modelAssets = (try? await container.modelManager.listModels()) ?? modelAssets
                updateModelAsset(targetModel, status: .failed, errorMessage: message)
            }
        }
    }

    func stop() {
        guard canStopRecording else { return }
        isRecordingCommandRunning = true
        Task {
            defer { isRecordingCommandRunning = false }
            guard let container else { return }
            do {
                try await container.agent.stopRecording()
                liveRefreshTask?.cancel()
                liveRefreshTask = nil
                await refreshFromAgent()
                history = try await container.store.fetchRecordings()
                if let first = history.first {
                    selectedDetail = try await container.store.fetchRecordingDetail(id: first.id)
                }
            } catch {
                liveRefreshTask?.cancel()
                liveRefreshTask = nil
                await refreshFromAgent()
                errorMessage = userMessage(for: error)
            }
            await refreshFromAgent()
        }
    }

    func openDetail(_ recording: Recording) {
        openDetailTask?.cancel()
        selectedRecordingID = recording.id
        exportStatusMessage = nil
        isExportStatusError = false
        openDetailTask = Task {
            guard let container else { return }
            let detail = try? await container.store.fetchRecordingDetail(id: recording.id)
            guard !Task.isCancelled, selectedRecordingID == recording.id else { return }
            selectedDetail = detail
        }
    }

    func copyTranscript() {
        let text = selectedDetail?.segments
            .filter { $0.status == .success }
            .map(\.text)
            .joined(separator: "\n") ?? ""
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    func copyMarkdown() {
        guard let selectedDetail else {
            setExportStatus("请选择记录后复制 Markdown", isError: true)
            return
        }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(
            MarkdownTranscriptExporter.format(
                recording: selectedDetail.recording,
                draft: currentTranscriptDraft(for: selectedDetail)
            ),
            forType: .string
        )
        setExportStatus("Markdown 已复制", isError: false)
    }

    func exportMarkdown() {
        guard let selectedDetail else {
            setExportStatus("请选择记录后导出 Markdown", isError: true)
            return
        }
        let markdown = MarkdownTranscriptExporter.format(
            recording: selectedDetail.recording,
            draft: currentTranscriptDraft(for: selectedDetail)
        )
        let panel = NSSavePanel()
        panel.canCreateDirectories = true
        panel.isExtensionHidden = false
        panel.nameFieldStringValue = MarkdownTranscriptExporter.suggestedFileName(title: selectedDetail.recording.title)
        panel.begin { [weak self] response in
            guard response == .OK, let url = panel.url else {
                Task { @MainActor in
                    self?.exportStatusMessage = nil
                    self?.isExportStatusError = false
                }
                return
            }
            Task {
                do {
                    try await Task.detached(priority: .userInitiated) {
                        try markdown.write(to: url, atomically: true, encoding: .utf8)
                    }.value
                    await MainActor.run {
                        self?.setExportStatus("Markdown 已导出：\(url.lastPathComponent)", isError: false)
                    }
                } catch {
                    await MainActor.run {
                        self?.setExportStatus("导出 Markdown 失败：\(error.localizedDescription)", isError: true)
                    }
                }
            }
        }
    }

    private func setExportStatus(_ message: String, isError: Bool) {
        exportStatusMessage = message
        isExportStatusError = isError
        if isError {
            errorMessage = message
        } else {
            errorMessage = nil
        }
    }

    private func currentTranscriptDraft(for detail: RecordingDetail) -> TranscriptDraft {
        .realtimeRaw(segments: detail.segments)
    }

    func refreshModels() {
        Task {
            guard let container else { return }
            modelAssets = (try? await container.modelManager.listModels()) ?? modelAssets
        }
    }

    private func refreshFromAgent() async {
        guard let container else { return }
        state = await container.agent.state
        hasActiveRecording = await container.agent.isRecordingActive
        confirmedSegments = await container.agent.confirmedSegments
        runtimeSnapshot = await container.agent.runtimeSnapshot()
    }

    private func beginLiveRefresh() {
        liveRefreshTask?.cancel()
        liveRefreshTask = Task { [weak self] in
            while !Task.isCancelled {
                await self?.refreshFromAgent()
                try? await Task.sleep(nanoseconds: 300_000_000)
            }
        }
    }

    private func updateModelAsset(_ model: ASRModelName, status: ASRModelAssetStatus, progress: Int? = nil, errorMessage: String = "") {
        guard let index = modelAssets.firstIndex(where: { $0.modelName == model }) else { return }
        var asset = modelAssets[index]
        asset.status = status
        if let progress {
            asset.downloadProgress = min(max(progress, 0), 100)
        }
        asset.errorMessage = errorMessage
        asset.updatedAt = Date()
        modelAssets[index] = asset
    }

    private func canPrepareModel(_ model: ASRModelName) -> Bool {
        container != nil &&
            !isPreparingModel &&
            !hasActiveRecording &&
            !isRecordingCommandRunning &&
            (modelAssets.first { $0.modelName == model }?.allowsPreparationStart ?? true)
    }

    private func userMessage(for error: Error) -> String {
        if let agentError = error as? RecordingAgentError {
            return "\(agentError.code.rawValue)：\(agentError.message)"
        }
        if let storeError = error as? SQLiteRecordingStoreError {
            return storeError.description
        }
        return error.localizedDescription
    }
}
