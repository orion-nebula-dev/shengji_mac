import AVFoundation
import SwiftUI
import RecordingAgentCore

struct RootView: View {
    @EnvironmentObject private var viewModel: RecordingAgentViewModel
    @State private var selection: SidebarItem? = .home

    var body: some View {
        NavigationSplitView {
            List(selection: $selection) {
                Label("首页", systemImage: "record.circle").tag(SidebarItem.home)
                Label("历史", systemImage: "clock").tag(SidebarItem.history)
                Label("设置", systemImage: "gearshape").tag(SidebarItem.settings)
            }
            .navigationTitle("声记")
        } detail: {
            switch selection {
            case .home:
                HomeView()
            case .history:
                HistoryView()
            case .settings:
                SettingsView()
            case .none:
                HomeView()
            }
        }
        .alert(
            "准备本地模型",
            isPresented: Binding(
                get: { viewModel.pendingModelPreparation != nil },
                set: { isPresented in
                    if !isPresented {
                        viewModel.cancelPendingModelPreparation()
                    }
                }
            )
        ) {
            Button("取消", role: .cancel) {
                viewModel.cancelPendingModelPreparation()
            }
            Button("准备") {
                viewModel.confirmPrepareSelectedModel()
            }
        } message: {
            Text("将下载并编译 \(viewModel.pendingModelPreparation?.rawValue ?? "") 本地转写模型。模型文件较大，过程中可以取消。")
        }
    }
}

enum SidebarItem: Hashable {
    case home
    case history
    case settings
}

struct HomeView: View {
    @EnvironmentObject private var viewModel: RecordingAgentViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                VStack(alignment: .leading, spacing: 6) {
                    Text("声记")
                        .font(.largeTitle.weight(.semibold))
                        .accessibilityIdentifier("home-title")
                    Text(statusText(viewModel.state))
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("home-status")
                }
                Spacer()
                Button {
                    viewModel.canStopRecording ? viewModel.stop() : viewModel.start()
                } label: {
                    Label(viewModel.hasActiveRecording ? "结束记录" : "开始记录", systemImage: viewModel.hasActiveRecording ? "stop.fill" : "record.circle")
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(viewModel.areRecordingControlsDisabled)
                .accessibilityIdentifier("home-recording-toggle-button")
            }

            if let error = viewModel.errorMessage {
                Text(error)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("home-error-message")
            }

            HStack {
                Picker("模型", selection: $viewModel.selectedModel) {
                    ForEach(ASRModelName.allCases, id: \.self) { model in
                        Text(model.rawValue).tag(model)
                    }
                }
                .disabled(viewModel.areRecordingInputsDisabled)
                .accessibilityIdentifier("home-model-picker")
                Picker("语言", selection: $viewModel.selectedLanguage) {
                    ForEach(LanguageMode.allCases, id: \.self) { language in
                        Text(language.displayName).tag(language)
                    }
                }
                .disabled(viewModel.areRecordingInputsDisabled)
                .accessibilityIdentifier("home-language-picker")
                Spacer()
            }

            if let asset = viewModel.selectedModelAsset {
                ModelPreparationRow(
                    asset: asset,
                    progress: viewModel.preparationProgress(for: asset),
                    isPreparing: viewModel.isPreparingModel,
                    canPrepare: viewModel.canPrepareSelectedModel,
                    onPrepare: viewModel.requestPrepareSelectedModel,
                    onCancel: viewModel.cancelPrepareSelectedModel
                )
            } else {
                Text("正在读取模型状态")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            if viewModel.hasActiveRecording {
                RecordingRuntimeRow(snapshot: viewModel.runtimeSnapshot)
            }

            Divider()

            Text("已确认片段")
                .font(.headline)
                .accessibilityIdentifier("home-confirmed-segments-title")
            if viewModel.confirmedSegments.isEmpty {
                EmptyStateView(systemImage: "waveform", title: "暂无确认转写", message: "开始记录后，每个 15 秒切片完成本地转写才会显示在这里。")
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        ForEach(viewModel.confirmedSegments, id: \.id) { segment in
                            TranscriptSegmentRow(segment: segment)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .padding(24)
        .accessibilityIdentifier("home-view")
    }
}

struct RecordingRuntimeRow: View {
    let snapshot: RecordingRuntimeSnapshot

    var body: some View {
        HStack(alignment: .center, spacing: 18) {
            Label {
                Text(snapshot.elapsedText)
                    .monospacedDigit()
            } icon: {
                Image(systemName: "timer")
            }
            .font(.headline)

            Divider()
                .frame(height: 24)

            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Label("输入音量", systemImage: "waveform")
                    Text(snapshot.inputLevel.displayText)
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                }
                ProgressView(value: Double(snapshot.inputLevel.percent), total: 100)
                    .frame(width: 220)
            }
        }
        .padding(.vertical, 4)
        .accessibilityIdentifier("home-runtime-row")
    }
}

struct HistoryView: View {
    @EnvironmentObject private var viewModel: RecordingAgentViewModel

    var body: some View {
        HSplitView {
            List(viewModel.history, id: \.id) { recording in
                Button {
                    viewModel.openDetail(recording)
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(recording.title)
                            .font(.headline)
                        Text("\(recording.status.rawValue) · \(recording.languageMode.displayName) · \(formatDuration(recording.durationMilliseconds)) · \(recording.transcriptSegmentCount) 段 / 失败 \(recording.failedSegmentCount)")
                            .foregroundStyle(.secondary)
                            .font(.caption)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.plain)
            }
            .frame(minWidth: 320)

            DetailView()
                .frame(minWidth: 520)
        }
        .padding(16)
    }
}

struct DetailView: View {
    @EnvironmentObject private var viewModel: RecordingAgentViewModel

    var body: some View {
        if let detail = viewModel.selectedDetail {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    HStack {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(detail.recording.title)
                                .font(.title2.weight(.semibold))
                            Text("\(detail.recording.status.rawValue) · \(detail.recording.languageMode.displayName) · \(formatDuration(detail.recording.durationMilliseconds))")
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button {
                            viewModel.copyTranscript()
                        } label: {
                            Label("复制全文", systemImage: "doc.on.doc")
                        }
                        .accessibilityIdentifier("detail-copy-transcript-button")
                        Button {
                            viewModel.copyMarkdown()
                        } label: {
                            Label("复制 Markdown", systemImage: "doc.on.clipboard")
                        }
                        .accessibilityIdentifier("detail-copy-markdown-button")
                        Button {
                            viewModel.exportMarkdown()
                        } label: {
                            Label("导出 Markdown", systemImage: "square.and.arrow.up")
                        }
                        .accessibilityIdentifier("detail-export-markdown-button")
                    }
                    if let exportStatus = viewModel.exportStatusMessage {
                        Text(exportStatus)
                            .font(.caption)
                            .foregroundStyle(viewModel.isExportStatusError ? .red : .secondary)
                            .textSelection(.enabled)
                            .accessibilityIdentifier("detail-export-status")
                    }

                    if !detail.recording.audioFilePath.isEmpty {
                        AudioPlaybackView(path: detail.recording.audioFilePath)
                    }

                    Text("转写片段")
                        .font(.headline)
                    ForEach(detail.segments, id: \.id) { segment in
                        TranscriptSegmentRow(segment: segment)
                    }

                    DisclosureGroup("Agent 事实") {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(detail.events, id: \.id) { event in
                                Text("\(event.eventType) · \(event.status) · \(event.message)")
                            }
                            ForEach(detail.steps, id: \.id) { step in
                                Text("\(step.stepType)[\(step.stepIndex)] · \(step.status.rawValue) · \(step.refType):\(step.refID)")
                            }
                            ForEach(detail.artifacts, id: \.id) { artifact in
                                Text("\(artifact.artifactType.rawValue) · \(artifact.uri)")
                            }
                        }
                        .font(.caption)
                        .textSelection(.enabled)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
            }
        } else {
            EmptyStateView(systemImage: "doc.text.magnifyingglass", title: "请选择记录", message: "")
        }
    }
}

struct EmptyStateView: View {
    let systemImage: String
    let title: String
    let message: String

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: systemImage)
                .font(.system(size: 36))
                .foregroundStyle(.secondary)
            Text(title)
                .font(.headline)
            if !message.isEmpty {
                Text(message)
                    .foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .multilineTextAlignment(.center)
    }
}

struct SettingsView: View {
    @EnvironmentObject private var viewModel: RecordingAgentViewModel

    var body: some View {
        Form {
            Picker("默认模型", selection: $viewModel.selectedModel) {
                ForEach(ASRModelName.allCases, id: \.self) { model in
                    Text(model.rawValue).tag(model)
                }
            }
            Picker("语言模式", selection: $viewModel.selectedLanguage) {
                ForEach(LanguageMode.allCases, id: \.self) { language in
                    Text(language.displayName).tag(language)
                }
            }

            Section("模型状态") {
                ForEach(viewModel.modelAssets, id: \.id) { asset in
                    let progress = viewModel.preparationProgress(for: asset)
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(asset.modelName.rawValue)
                            Text(progress?.displayText ?? (asset.errorMessage.isEmpty ? asset.status.recoveryHint : asset.errorMessage))
                                .font(.caption)
                                .foregroundStyle(asset.status == .failed ? .red : .secondary)
                                .lineLimit(2)
                        }
                        Spacer()
                        HStack(spacing: 8) {
                            ProgressView(value: Double(progress?.percent ?? asset.clampedDownloadProgress), total: 100)
                                .frame(width: 72)
                            Text("\(asset.status.displayName) · \(progress?.displayText ?? asset.progressText)")
                                .foregroundStyle(modelStatusColor(asset.status))
                        }
                    }
                }
                Button {
                    viewModel.refreshModels()
                } label: {
                    Label("刷新", systemImage: "arrow.clockwise")
                }
                Button {
                    viewModel.requestPrepareSelectedModel()
                } label: {
                    Label(
                        viewModel.isPreparingModel ? "准备中" : (viewModel.selectedModelAsset?.preparationActionTitle ?? "准备模型"),
                        systemImage: viewModel.selectedModelAsset?.status == .failed ? "arrow.clockwise.circle" : "square.and.arrow.down"
                    )
                }
                .disabled(!viewModel.canPrepareSelectedModel)
                if viewModel.isPreparingModel {
                    Button {
                        viewModel.cancelPrepareSelectedModel()
                    } label: {
                        Label("取消准备", systemImage: "xmark.circle")
                    }
                }
            }

            Section("本地路径") {
                Text(viewModel.databasePath)
                Text(viewModel.modelPath)
            }
            .textSelection(.enabled)
        }
        .padding(24)
    }
}

struct AudioPlaybackView: View {
    let path: String
    @StateObject private var playback = AudioPlaybackController()

    var body: some View {
        HStack(spacing: 12) {
            Button {
                playback.toggle(path: path)
            } label: {
                Label(playback.isPlaying ? "停止播放" : "播放音频", systemImage: playback.isPlaying ? "stop.fill" : "play.fill")
            }
            .disabled(!FileManager.default.fileExists(atPath: path))

            Text(URL(fileURLWithPath: path).lastPathComponent)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
        }
        .padding(.vertical, 6)
        .onDisappear {
            playback.stop()
        }
    }
}

struct ModelPreparationRow: View {
    let asset: ASRModelAsset
    let progress: ModelPreparationProgress?
    let isPreparing: Bool
    let canPrepare: Bool
    let onPrepare: () -> Void
    let onCancel: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: modelStatusIcon(asset.status))
                .foregroundStyle(modelStatusColor(asset.status))
                .frame(width: 18)

            VStack(alignment: .leading, spacing: 2) {
                Text("\(asset.modelName.rawValue) · \(asset.status.displayName)")
                    .font(.subheadline.weight(.medium))
                Text(progress?.displayText ?? (asset.errorMessage.isEmpty ? asset.status.recoveryHint : asset.errorMessage))
                    .font(.caption)
                    .foregroundStyle(asset.status == .failed ? .red : .secondary)
                    .lineLimit(2)
            }

            Spacer()

            ProgressView(value: Double(progress?.percent ?? asset.clampedDownloadProgress), total: 100)
                .frame(width: 96)

            Text(progress?.displayText ?? asset.progressText)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(width: 136, alignment: .trailing)
                .lineLimit(1)

            if isPreparing {
                Button {
                    onCancel()
                } label: {
                    Label("取消", systemImage: "xmark.circle")
                }
            } else {
                Button {
                    onPrepare()
                } label: {
                    Label(asset.preparationActionTitle, systemImage: asset.status == .failed ? "arrow.clockwise.circle" : "square.and.arrow.down")
                }
                .disabled(!canPrepare)
            }
        }
        .padding(.vertical, 4)
        .accessibilityIdentifier("model-preparation-row")
    }
}

@MainActor
final class AudioPlaybackController: NSObject, ObservableObject, AVAudioPlayerDelegate {
    @Published private(set) var isPlaying = false
    private var player: AVAudioPlayer?
    private var currentPath: String?

    func toggle(path: String) {
        if isPlaying, currentPath == path {
            stop()
        } else {
            play(path: path)
        }
    }

    func play(path: String) {
        stop()
        do {
            let player = try AVAudioPlayer(contentsOf: URL(fileURLWithPath: path))
            player.delegate = self
            player.prepareToPlay()
            player.play()
            self.player = player
            currentPath = path
            isPlaying = true
        } catch {
            isPlaying = false
            currentPath = nil
            player = nil
        }
    }

    func stop() {
        player?.stop()
        player = nil
        currentPath = nil
        isPlaying = false
    }

    nonisolated func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        Task { @MainActor in
            if self.player === player {
                self.stop()
            }
        }
    }
}

struct TranscriptSegmentRow: View {
    let segment: TranscriptSegment

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("\(timecode(segment.startMilliseconds)) - \(timecode(segment.endMilliseconds)) · \(segment.status.rawValue)")
                .foregroundStyle(segment.status == .failed ? .red : .secondary)
                .font(.caption)
            Text(segment.status == .failed ? (segment.errorMessage ?? "转写失败") : segment.text)
                .textSelection(.enabled)
        }
        .padding(.vertical, 6)
    }
}

private func statusText(_ state: RecordingAgentState) -> String {
    switch state {
    case .idle: return "未开始"
    case .checkingPermissions: return "检查麦克风权限"
    case .checkingModel: return "检查本地模型"
    case .downloadingModel: return "准备模型"
    case .recording: return "录音中"
    case .slicing: return "切片中"
    case .transcribing: return "转写中"
    case .persisting: return "保存中"
    case .completed: return "已完成"
    case .failed: return "失败"
    }
}

private func formatDuration(_ milliseconds: Int) -> String {
    let seconds = milliseconds / 1000
    return "\(seconds / 60):\(String(format: "%02d", seconds % 60))"
}

private func timecode(_ milliseconds: Int) -> String {
    let seconds = milliseconds / 1000
    return "\(seconds / 60):\(String(format: "%02d", seconds % 60))"
}

private func modelStatusIcon(_ status: ASRModelAssetStatus) -> String {
    switch status {
    case .notDownloaded: return "tray.and.arrow.down"
    case .downloading: return "arrow.down.circle"
    case .verifying: return "checkmark.shield"
    case .ready: return "checkmark.circle.fill"
    case .failed: return "exclamationmark.triangle.fill"
    }
}

private func modelStatusColor(_ status: ASRModelAssetStatus) -> Color {
    switch status {
    case .notDownloaded: return .secondary
    case .downloading, .verifying: return .orange
    case .ready: return .green
    case .failed: return .red
    }
}
