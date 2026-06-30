import Foundation

public enum ASRModelName: String, CaseIterable, Codable, Equatable, Sendable {
    case tiny
    case base
    case small

    public static let `default`: ASRModelName = .small

    public var whisperKitVariantName: String {
        rawValue
    }

    public var whisperKitRepositoryDirectoryName: String {
        "openai_whisper-\(rawValue)"
    }

    public var openAIWhisperRepositoryDirectoryName: String {
        "whisper-\(rawValue)"
    }

    public var whisperKitRepositoryGlob: String {
        "\(whisperKitRepositoryDirectoryName)/**"
    }

    public static let whisperKitSharedMelPackageDirectoryName = "openai_whisper-tiny.en"
}

public enum LanguageMode: String, CaseIterable, Codable, Equatable, Sendable {
    case auto
    case zh
    case en

    public var displayName: String {
        switch self {
        case .auto: return "自动识别"
        case .zh: return "中文"
        case .en: return "英文"
        }
    }

    public var whisperLanguage: String? {
        switch self {
        case .auto: return nil
        case .zh: return "zh"
        case .en: return "en"
        }
    }
}

public struct LocalASRDecodingPolicy: Equatable, Sendable {
    public let taskName: String
    public let languageCode: String?
    public let shouldDetectLanguage: Bool
    public let temperatureFallbackCount: Int

    public init(taskName: String, languageCode: String?, shouldDetectLanguage: Bool, temperatureFallbackCount: Int) {
        self.taskName = taskName
        self.languageCode = languageCode
        self.shouldDetectLanguage = shouldDetectLanguage
        self.temperatureFallbackCount = temperatureFallbackCount
    }

    public static func whisperKitPolicy(for languageMode: LanguageMode) -> LocalASRDecodingPolicy {
        LocalASRDecodingPolicy(
            taskName: "transcribe",
            languageCode: languageMode.whisperLanguage,
            shouldDetectLanguage: languageMode == .auto,
            temperatureFallbackCount: 1
        )
    }
}

public struct ASRTranscriptPostProcessor: Sendable {
    public static func process(text rawText: String) -> Result<String, RecordingAgentError> {
        let trimmed = normalized(rawText)
        guard !trimmed.isEmpty else {
            return .failure(.init(code: .transcribeFailed, message: "WhisperKit 返回空转写"))
        }

        let roleStripped = normalized(stripKnownRolePrefix(from: trimmed))
        guard !roleStripped.isEmpty else {
            return .failure(.init(code: .transcribeFailed, message: "WhisperKit 返回空转写"))
        }

        if isPureNonSpeechAnnotation(roleStripped) {
            return .failure(.init(code: .transcribeFailed, message: "疑似非语音或字幕模板幻觉，已标记失败"))
        }

        return .success(roleStripped)
    }

    private static func normalized(_ text: String) -> String {
        text
            .replacingOccurrences(of: "\n", with: " ")
            .split(whereSeparator: { $0.isWhitespace })
            .joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func stripKnownRolePrefix(from text: String) -> String {
        let unwrapped = unwrapIfWholeTextIsWrapped(text)
        let prefixes = ["主持人:", "主持人：", "主播:", "主播："]
        for prefix in prefixes where unwrapped.hasPrefix(prefix) {
            return String(unwrapped.dropFirst(prefix.count))
                .trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return text
    }

    private static func isPureNonSpeechAnnotation(_ text: String) -> Bool {
        let candidate = unwrapIfWholeTextIsWrapped(text)
            .lowercased()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard candidate.count <= 32 else { return false }
        if candidate.hasPrefix("字幕製作") || candidate.hasPrefix("字幕制作") {
            return true
        }
        if ["觀眾留言", "观众留言", "台語", "台语"].contains(candidate) {
            return true
        }
        return candidate.contains("music playing") || candidate.contains("speaking in foreign language")
    }

    private static func unwrapIfWholeTextIsWrapped(_ text: String) -> String {
        let pairs: [(Character, Character)] = [("(", ")"), ("（", "）"), ("[", "]"), ("【", "】")]
        guard let first = text.first, let last = text.last else { return text }
        for pair in pairs where first == pair.0 && last == pair.1 {
            return String(text.dropFirst().dropLast())
                .trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return text
    }
}

public enum RecordingAgentState: String, Codable, Equatable, Sendable {
    case idle
    case checkingPermissions = "checking_permissions"
    case checkingModel = "checking_model"
    case downloadingModel = "downloading_model"
    case recording
    case slicing
    case transcribing
    case persisting
    case completed
    case failed
}

extension RecordingAgentState {
    public var allowsRecordingStart: Bool {
        switch self {
        case .idle, .completed, .failed:
            return true
        case .checkingPermissions, .checkingModel, .downloadingModel, .recording, .slicing, .transcribing, .persisting:
            return false
        }
    }

    public var allowsRecordingStop: Bool {
        switch self {
        case .recording, .slicing, .transcribing:
            return true
        case .idle, .checkingPermissions, .checkingModel, .downloadingModel, .persisting, .completed, .failed:
            return false
        }
    }
}

public enum RecordingAgentErrorCode: String, CaseIterable, Codable, Equatable, Sendable {
    case permissionDenied = "permission_denied"
    case modelMissing = "model_missing"
    case downloadFailed = "download_failed"
    case verifyFailed = "verify_failed"
    case asrEngineUnavailable = "asr_engine_unavailable"
    case writeFailed = "write_failed"
    case transcribeFailed = "transcribe_failed"
}

public struct RecordingAgentError: Error, Equatable, Sendable {
    public let code: RecordingAgentErrorCode
    public let message: String

    public init(code: RecordingAgentErrorCode, message: String) {
        self.code = code
        self.message = message
    }
}

public enum RecordingStatus: String, Codable, Equatable, Sendable {
    case recording
    case transcribing
    case completed
    case failed
}

public enum AudioSegmentStatus: String, Codable, Equatable, Sendable {
    case pending
    case transcribing
    case transcribed
    case failed
}

public enum TranscriptSegmentStatus: String, Codable, Equatable, Sendable {
    case pending
    case success
    case failed
}

public enum AgentTaskStepStatus: String, Codable, Equatable, Sendable {
    case pending
    case running
    case succeeded
    case failed
    case skipped
}

public enum AgentTaskArtifactType: String, Codable, Equatable, Sendable {
    case wavAudio = "wav_audio"
    case audioSlice = "audio_slice"
    case transcriptRaw = "transcript_raw"
    case transcriptSegment = "transcript_segment"
}

public enum ASRModelAssetStatus: String, Codable, Equatable, Sendable {
    case notDownloaded = "not_downloaded"
    case downloading
    case verifying
    case ready
    case failed

    public var displayName: String {
        switch self {
        case .notDownloaded: return "未下载"
        case .downloading: return "下载中"
        case .verifying: return "校验中"
        case .ready: return "已就绪"
        case .failed: return "准备失败"
        }
    }

    public var recoveryHint: String {
        switch self {
        case .notDownloaded: return "需要先准备模型"
        case .downloading: return "请等待下载完成"
        case .verifying: return "请等待校验完成"
        case .ready: return "可以开始记录"
        case .failed: return "可重试准备模型"
        }
    }
}

public enum ModelPrepareTrigger: String, Codable, Equatable, Sendable {
    case firstLaunch = "first_launch"
    case settings
    case startRecording = "start_recording"
}

public struct Recording: Codable, Equatable, Sendable {
    public let id: String
    public var title: String
    public var status: RecordingStatus
    public var audioFilePath: String
    public var durationMilliseconds: Int
    public var sampleRate: Int
    public var channels: Int
    public var modelName: ASRModelName
    public var languageMode: LanguageMode
    public var transcriptSegmentCount: Int
    public var failedSegmentCount: Int
    public var errorCode: RecordingAgentErrorCode?
    public var errorMessage: String?
    public var startedAt: Date
    public var endedAt: Date?
    public var createdAt: Date
    public var updatedAt: Date

    public init(
        id: String,
        title: String,
        status: RecordingStatus,
        audioFilePath: String,
        durationMilliseconds: Int,
        sampleRate: Int,
        channels: Int,
        modelName: ASRModelName,
        languageMode: LanguageMode,
        transcriptSegmentCount: Int,
        failedSegmentCount: Int,
        errorCode: RecordingAgentErrorCode?,
        errorMessage: String?,
        startedAt: Date,
        endedAt: Date?,
        createdAt: Date,
        updatedAt: Date
    ) {
        self.id = id
        self.title = title
        self.status = status
        self.audioFilePath = audioFilePath
        self.durationMilliseconds = durationMilliseconds
        self.sampleRate = sampleRate
        self.channels = channels
        self.modelName = modelName
        self.languageMode = languageMode
        self.transcriptSegmentCount = transcriptSegmentCount
        self.failedSegmentCount = failedSegmentCount
        self.errorCode = errorCode
        self.errorMessage = errorMessage
        self.startedAt = startedAt
        self.endedAt = endedAt
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }

    public static func defaultTitle(now: Date, calendar: Calendar = .current) -> String {
        let formatter = DateFormatter()
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd HH:mm"
        return "\(formatter.string(from: now)) 录音"
    }
}

public struct AudioSegment: Codable, Equatable, Sendable {
    public let id: String
    public let recordingID: String
    public let segmentIndex: Int
    public let filePath: String
    public let startMilliseconds: Int
    public let endMilliseconds: Int
    public let durationMilliseconds: Int
    public let sampleRate: Int
    public let channels: Int
    public let status: AudioSegmentStatus
    public let errorCode: RecordingAgentErrorCode?
    public let errorMessage: String?
    public let createdAt: Date

    public init(id: String, recordingID: String, segmentIndex: Int, filePath: String, startMilliseconds: Int, endMilliseconds: Int, durationMilliseconds: Int, sampleRate: Int, channels: Int, status: AudioSegmentStatus, errorCode: RecordingAgentErrorCode?, errorMessage: String?, createdAt: Date) {
        self.id = id
        self.recordingID = recordingID
        self.segmentIndex = segmentIndex
        self.filePath = filePath
        self.startMilliseconds = startMilliseconds
        self.endMilliseconds = endMilliseconds
        self.durationMilliseconds = durationMilliseconds
        self.sampleRate = sampleRate
        self.channels = channels
        self.status = status
        self.errorCode = errorCode
        self.errorMessage = errorMessage
        self.createdAt = createdAt
    }
}

public struct TranscriptSegment: Codable, Equatable, Sendable {
    public let id: String
    public let recordingID: String
    public let audioSegmentID: String
    public let segmentIndex: Int
    public let startMilliseconds: Int
    public let endMilliseconds: Int
    public let text: String
    public let language: String
    public let status: TranscriptSegmentStatus
    public let provider: String
    public let modelName: ASRModelName
    public let errorCode: RecordingAgentErrorCode?
    public let errorMessage: String?
    public let createdAt: Date

    public init(id: String, recordingID: String, audioSegmentID: String, segmentIndex: Int, startMilliseconds: Int, endMilliseconds: Int, text: String, language: String, status: TranscriptSegmentStatus, provider: String, modelName: ASRModelName, errorCode: RecordingAgentErrorCode?, errorMessage: String?, createdAt: Date) {
        self.id = id
        self.recordingID = recordingID
        self.audioSegmentID = audioSegmentID
        self.segmentIndex = segmentIndex
        self.startMilliseconds = startMilliseconds
        self.endMilliseconds = endMilliseconds
        self.text = text
        self.language = language
        self.status = status
        self.provider = provider
        self.modelName = modelName
        self.errorCode = errorCode
        self.errorMessage = errorMessage
        self.createdAt = createdAt
    }
}

public struct AgentTask: Codable, Equatable, Sendable {
    public let id: String
    public let agentType: String
    public let recordingID: String
    public var status: RecordingAgentState
    public var inputJSON: String
    public var outputJSON: String
    public var errorCode: RecordingAgentErrorCode?
    public var errorMessage: String?
    public let startedAt: Date
    public var finishedAt: Date?
    public let createdAt: Date
    public var updatedAt: Date

    public init(id: String, agentType: String, recordingID: String, status: RecordingAgentState, inputJSON: String, outputJSON: String, errorCode: RecordingAgentErrorCode?, errorMessage: String?, startedAt: Date, finishedAt: Date?, createdAt: Date, updatedAt: Date) {
        self.id = id
        self.agentType = agentType
        self.recordingID = recordingID
        self.status = status
        self.inputJSON = inputJSON
        self.outputJSON = outputJSON
        self.errorCode = errorCode
        self.errorMessage = errorMessage
        self.startedAt = startedAt
        self.finishedAt = finishedAt
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
}

public struct AgentTaskEvent: Codable, Equatable, Sendable {
    public let id: String
    public let taskID: String
    public let eventType: String
    public let status: String
    public let progress: Int
    public let message: String
    public let errorCode: RecordingAgentErrorCode?
    public let errorMessage: String?
    public let payloadJSON: String
    public let createdAt: Date

    public init(id: String, taskID: String, eventType: String, status: String, progress: Int, message: String, errorCode: RecordingAgentErrorCode?, errorMessage: String?, payloadJSON: String, createdAt: Date) {
        self.id = id
        self.taskID = taskID
        self.eventType = eventType
        self.status = status
        self.progress = progress
        self.message = message
        self.errorCode = errorCode
        self.errorMessage = errorMessage
        self.payloadJSON = payloadJSON
        self.createdAt = createdAt
    }
}

public struct AgentTaskStep: Codable, Equatable, Sendable {
    public let id: String
    public let taskID: String
    public let stepType: String
    public let stepIndex: Int
    public let status: AgentTaskStepStatus
    public let refType: String
    public let refID: String
    public let inputJSON: String
    public let outputJSON: String
    public let errorCode: RecordingAgentErrorCode?
    public let errorMessage: String?
    public let startedAt: Date?
    public let finishedAt: Date?
    public let createdAt: Date
    public let updatedAt: Date

    public init(id: String, taskID: String, stepType: String, stepIndex: Int, status: AgentTaskStepStatus, refType: String, refID: String, inputJSON: String, outputJSON: String, errorCode: RecordingAgentErrorCode?, errorMessage: String?, startedAt: Date?, finishedAt: Date?, createdAt: Date, updatedAt: Date) {
        self.id = id
        self.taskID = taskID
        self.stepType = stepType
        self.stepIndex = stepIndex
        self.status = status
        self.refType = refType
        self.refID = refID
        self.inputJSON = inputJSON
        self.outputJSON = outputJSON
        self.errorCode = errorCode
        self.errorMessage = errorMessage
        self.startedAt = startedAt
        self.finishedAt = finishedAt
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
}

public struct AgentTaskArtifact: Codable, Equatable, Sendable {
    public let id: String
    public let taskID: String
    public let recordingID: String
    public let artifactType: AgentTaskArtifactType
    public let uri: String
    public let refType: String
    public let refID: String
    public let metadataJSON: String
    public let createdAt: Date

    public init(id: String, taskID: String, recordingID: String, artifactType: AgentTaskArtifactType, uri: String, refType: String, refID: String, metadataJSON: String, createdAt: Date) {
        self.id = id
        self.taskID = taskID
        self.recordingID = recordingID
        self.artifactType = artifactType
        self.uri = uri
        self.refType = refType
        self.refID = refID
        self.metadataJSON = metadataJSON
        self.createdAt = createdAt
    }
}

public struct ASRModelAsset: Codable, Equatable, Sendable {
    public let id: String
    public let provider: String
    public let modelName: ASRModelName
    public let displayName: String
    public var status: ASRModelAssetStatus
    public var downloadProgress: Int
    public var localPath: String
    public var checksum: String
    public var sizeBytes: Int
    public var isDefault: Bool
    public var errorMessage: String
    public let createdAt: Date
    public var updatedAt: Date
}

public struct ModelPreparationProgress: Equatable, Sendable {
    public let modelName: ASRModelName
    public let status: ASRModelAssetStatus
    public let completedUnitCount: Int
    public let totalUnitCount: Int
    public let message: String

    public init(
        modelName: ASRModelName,
        status: ASRModelAssetStatus,
        completedUnitCount: Int,
        totalUnitCount: Int,
        message: String
    ) {
        self.modelName = modelName
        self.status = status
        self.completedUnitCount = completedUnitCount
        self.totalUnitCount = totalUnitCount
        self.message = message
    }

    public var percent: Int {
        guard totalUnitCount > 0 else { return 0 }
        let clampedCompleted = min(max(completedUnitCount, 0), totalUnitCount)
        return Int((Double(clampedCompleted) / Double(totalUnitCount) * 100).rounded(.down))
    }

    public var displayText: String {
        let base = "\(percent)%"
        let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? base : "\(base) · \(trimmed)"
    }
}

public struct ModelDownloadFileProgress: Equatable, Sendable {
    public let fileName: String
    public let fileIndex: Int
    public let totalFileCount: Int
    public let completedBytes: Int64
    public let totalBytes: Int64?

    public init(fileName: String, fileIndex: Int, totalFileCount: Int, completedBytes: Int64, totalBytes: Int64?) {
        self.fileName = fileName
        self.fileIndex = fileIndex
        self.totalFileCount = totalFileCount
        self.completedBytes = completedBytes
        self.totalBytes = totalBytes
    }

    public var filePercent: Int {
        guard let totalBytes, totalBytes > 0 else { return 0 }
        let boundedCompletedBytes = min(max(completedBytes, 0), totalBytes)
        return Int((Double(boundedCompletedBytes) / Double(totalBytes) * 100).rounded(.down))
    }

    public var overallCompletedUnitCount: Int {
        guard totalFileCount > 0 else { return 0 }
        let clampedFileIndex = min(max(fileIndex, 0), totalFileCount)
        return min(clampedFileIndex * 100 + filePercent, overallTotalUnitCount)
    }

    public var overallTotalUnitCount: Int {
        max(totalFileCount, 0) * 100
    }

    public var displayMessage: String {
        let trimmedName = fileName.trimmingCharacters(in: .whitespacesAndNewlines)
        let name = trimmedName.isEmpty ? "下载文件" : trimmedName
        guard let totalBytes, totalBytes > 0 else {
            return "\(name) · \(Self.formatBytes(max(completedBytes, 0)))"
        }
        return "\(name) · \(Self.formatBytes(max(completedBytes, 0))) / \(Self.formatBytes(totalBytes))"
    }

    public func preparationProgress(modelName: ASRModelName, status: ASRModelAssetStatus = .downloading) -> ModelPreparationProgress {
        ModelPreparationProgress(
            modelName: modelName,
            status: status,
            completedUnitCount: overallCompletedUnitCount,
            totalUnitCount: max(overallTotalUnitCount, 1),
            message: displayMessage
        )
    }

    private static func formatBytes(_ bytes: Int64) -> String {
        if bytes < 1024 {
            return "\(bytes) B"
        }
        let kilobytes = Double(bytes) / 1024
        if kilobytes < 1024 {
            return String(format: "%.1f KB", kilobytes)
        }
        let megabytes = kilobytes / 1024
        if megabytes < 1024 {
            return String(format: "%.1f MB", megabytes)
        }
        return String(format: "%.1f GB", megabytes / 1024)
    }
}

public extension ASRModelAsset {
    var clampedDownloadProgress: Int {
        min(max(downloadProgress, 0), 100)
    }

    var progressText: String {
        switch status {
        case .notDownloaded:
            return "0%"
        case .downloading, .verifying, .ready:
            return "\(clampedDownloadProgress)%"
        case .failed:
            return "\(clampedDownloadProgress)% 后失败"
        }
    }

    var preparationActionTitle: String {
        switch status {
        case .notDownloaded:
            return "准备模型"
        case .downloading:
            return "准备中"
        case .verifying:
            return "校验中"
        case .ready:
            return "重新准备模型"
        case .failed:
            return "重试准备模型"
        }
    }

    var allowsPreparationStart: Bool {
        switch status {
        case .notDownloaded, .ready, .failed:
            return true
        case .downloading, .verifying:
            return false
        }
    }

    var isReadyForRecording: Bool {
        status == .ready
    }
}

public struct RecordingDetail: Equatable, Sendable {
    public let recording: Recording
    public let audioSegments: [AudioSegment]
    public let segments: [TranscriptSegment]
    public let events: [AgentTaskEvent]
    public let steps: [AgentTaskStep]
    public let artifacts: [AgentTaskArtifact]
}

public extension Calendar {
    static var gregorianUTC: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }
}
