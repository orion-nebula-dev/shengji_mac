import Foundation

public struct Clock: Sendable {
    private let nowProvider: @Sendable () -> Date

    public init(now: @escaping @Sendable () -> Date = { Date() }) {
        self.nowProvider = now
    }

    public func now() -> Date {
        nowProvider()
    }

    public static func fixed(_ date: Date) -> Clock {
        Clock { date }
    }
}

public actor RecordingAgent {
    public private(set) var state: RecordingAgentState = .idle
    public private(set) var confirmedSegments: [TranscriptSegment] = []
    public var isRecordingActive: Bool {
        activeRecordingID != nil
    }

    private let store: SQLiteRecordingStore
    private let permissionService: MicrophonePermissionService
    private let audioCaptureService: AudioCaptureService
    private let modelManager: ModelManaging
    private let asrEngine: LocalASREngine
    private let clock: Clock
    private let recordingsBaseDirectory: URL

    private var activeRecordingID: String?
    private var activeTaskID: String?
    private var activeModel: ASRModelName = .default
    private var activeLanguage: LanguageMode = .auto
    private var activeAudioStream: AsyncThrowingStream<AudioSlice, Error>?
    private var activeOutputDirectory: URL?
    private var activeStartedAt: Date?
    private var processingTask: Task<Void, Error>?

    public init(
        store: SQLiteRecordingStore,
        permissionService: MicrophonePermissionService,
        audioCaptureService: AudioCaptureService,
        modelManager: ModelManaging,
        asrEngine: LocalASREngine,
        clock: Clock = Clock(),
        recordingsBaseDirectory: URL = RecordingAgent.defaultRecordingsBaseDirectory()
    ) {
        self.store = store
        self.permissionService = permissionService
        self.audioCaptureService = audioCaptureService
        self.modelManager = modelManager
        self.asrEngine = asrEngine
        self.clock = clock
        self.recordingsBaseDirectory = recordingsBaseDirectory
    }

    public static func defaultRecordingsBaseDirectory() -> URL {
        let applicationSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first ?? FileManager.default.homeDirectoryForCurrentUser.appending(path: "Library/Application Support")
        return applicationSupport
            .appending(path: "com.shengji.recording-agent")
            .appending(path: "recordings")
    }

    public func bootstrap() async throws {
        state = .idle
        _ = try await modelManager.listModels()
    }

    public func prepareModel(_ model: ASRModelName, trigger: ModelPrepareTrigger) async throws {
        state = .downloadingModel
        do {
            _ = try await modelManager.prepare(model: model, trigger: trigger)
            state = .idle
        } catch {
            state = .failed
            throw error
        }
    }

    public func runtimeSnapshot() async -> RecordingRuntimeSnapshot {
        guard let recordingID = activeRecordingID, let activeStartedAt else {
            return .inactive
        }
        let elapsedMilliseconds = Int(clock.now().timeIntervalSince(activeStartedAt) * 1000)
        let inputLevel = await audioCaptureService.currentInputLevel()
        return RecordingRuntimeSnapshot(
            recordingID: recordingID,
            startedAt: activeStartedAt,
            elapsedMilliseconds: elapsedMilliseconds,
            inputLevel: inputLevel
        )
    }

    public func startRecording(model: ASRModelName, language: LanguageMode) async throws {
        guard activeRecordingID == nil, state.allowsRecordingStart else {
            throw RecordingAgentError(code: .writeFailed, message: "已有记录正在进行，请先结束当前记录")
        }

        confirmedSegments = []
        activeModel = model
        activeLanguage = language
        let now = clock.now()
        let recordingID = UUID().uuidString
        let taskID = UUID().uuidString
        let outputDirectory = recordingsBaseDirectory.appending(path: recordingID)
        activeRecordingID = recordingID
        activeTaskID = taskID
        activeOutputDirectory = outputDirectory
        activeStartedAt = now

        state = .checkingPermissions
        let permission = await permissionService.requestMicrophonePermission()
        guard permission == .authorized else {
            try await failBeforeRecording(recordingID: recordingID, taskID: taskID, code: .permissionDenied, message: "麦克风权限未开启", now: now)
            throw RecordingAgentError(code: .permissionDenied, message: "麦克风权限未开启")
        }

        state = .checkingModel
        do {
            _ = try await modelManager.verify(model: model)
        } catch let error as RecordingAgentError {
            try await failBeforeRecording(recordingID: recordingID, taskID: taskID, code: error.code, message: error.message, now: now)
            throw error
        } catch {
            let wrapped = RecordingAgentError(code: .modelMissing, message: error.localizedDescription)
            try await failBeforeRecording(recordingID: recordingID, taskID: taskID, code: wrapped.code, message: wrapped.message, now: now)
            throw wrapped
        }

        switch await asrEngine.checkReady(model: model) {
        case .success:
            break
        case let .failure(error):
            try await failBeforeRecording(recordingID: recordingID, taskID: taskID, code: error.code, message: error.message, now: now)
            throw error
        }

        let recording = Recording(
            id: recordingID,
            title: Recording.defaultTitle(now: now, calendar: .gregorianUTC),
            status: .recording,
            audioFilePath: outputDirectory.appending(path: "full.wav").path,
            durationMilliseconds: 0,
            sampleRate: 16_000,
            channels: 1,
            modelName: model,
            languageMode: language,
            transcriptSegmentCount: 0,
            failedSegmentCount: 0,
            errorCode: nil,
            errorMessage: nil,
            startedAt: now,
            endedAt: nil,
            createdAt: now,
            updatedAt: now
        )
        try await store.upsertRecording(recording)
        try await store.upsertAgentTask(AgentTask(
            id: taskID,
            agentType: "recording",
            recordingID: recordingID,
            status: .recording,
            inputJSON: #"{"model":"\#(model.rawValue)","language":"\#(language.rawValue)"}"#,
            outputJSON: "{}",
            errorCode: nil,
            errorMessage: nil,
            startedAt: now,
            finishedAt: nil,
            createdAt: now,
            updatedAt: now
        ))
        try await recordEvent(type: "recording", status: "started", progress: 30, message: "录音中")

        state = .recording
        do {
            let stream = try await audioCaptureService.start(configuration: AudioCaptureConfiguration(recordingID: recordingID, outputDirectory: outputDirectory))
            activeAudioStream = stream
            processingTask = Task { [self] in
                try await processAudioStream(stream, taskID: taskID)
            }
        } catch {
            let agentError = normalize(error, fallback: .writeFailed)
            try await failActiveRecording(recordingID: recordingID, taskID: taskID, code: agentError.code, message: agentError.message)
            throw agentError
        }
    }

    public func stopRecording() async throws {
        guard let recordingID = activeRecordingID, let taskID = activeTaskID else {
            return
        }
        defer {
            clearActiveRecording()
        }

        let result: AudioCaptureResult
        do {
            result = try await audioCaptureService.stop()
            try await processingTask?.value
        } catch {
            processingTask = nil
            let agentError = normalize(error, fallback: .writeFailed)
            try await failActiveRecording(recordingID: recordingID, taskID: taskID, code: agentError.code, message: agentError.message)
            throw agentError
        }
        processingTask = nil
        state = .persisting
        let now = clock.now()
        try await store.insertAgentTaskArtifact(AgentTaskArtifact(
            id: UUID().uuidString,
            taskID: taskID,
            recordingID: recordingID,
            artifactType: .wavAudio,
            uri: result.fullAudioURL.path,
            refType: "recording",
            refID: recordingID,
            metadataJSON: #"{"sampleRate":16000,"channels":1}"#,
            createdAt: now
        ))

        guard var recording = try await store.fetchRecordingDetail(id: recordingID)?.recording else {
            throw SQLiteRecordingStoreError.missingRecording(recordingID)
        }
        recording.status = .completed
        recording.audioFilePath = result.fullAudioURL.path
        recording.durationMilliseconds = result.durationMilliseconds
        recording.endedAt = now
        recording.updatedAt = now
        try await store.upsertRecording(recording)
        try await store.updateRecordingCounts(recordingID: recordingID)

        try await store.upsertAgentTask(AgentTask(
            id: taskID,
            agentType: "recording",
            recordingID: recordingID,
            status: .completed,
            inputJSON: "{}",
            outputJSON: #"{"recordingID":"\#(recordingID)"}"#,
            errorCode: nil,
            errorMessage: nil,
            startedAt: recording.startedAt,
            finishedAt: now,
            createdAt: recording.createdAt,
            updatedAt: now
        ))
        try await recordEvent(type: "persisting", status: "completed", progress: 100, message: "保存完成")
        state = .completed
    }

    private func processAudioStream(_ stream: AsyncThrowingStream<AudioSlice, Error>, taskID: String) async throws {
        state = .slicing
        try await recordEvent(type: "slicing", status: "started", progress: 45, message: "正在切片")

        for try await slice in stream {
            try await persist(slice: slice, taskID: taskID)
            state = .transcribing
            try await recordEvent(type: "transcribing", status: "started", progress: 60, message: "正在转写第 \(slice.index + 1) 段")
            try await transcribe(slice: slice, taskID: taskID)
            state = .recording
        }
    }

    private func failBeforeRecording(recordingID: String, taskID: String, code: RecordingAgentErrorCode, message: String, now: Date) async throws {
        defer {
            clearActiveRecording()
        }
        state = .failed
        let recording = Recording(
            id: recordingID,
            title: Recording.defaultTitle(now: now, calendar: .gregorianUTC),
            status: .failed,
            audioFilePath: "",
            durationMilliseconds: 0,
            sampleRate: 16_000,
            channels: 1,
            modelName: activeModel,
            languageMode: activeLanguage,
            transcriptSegmentCount: 0,
            failedSegmentCount: 0,
            errorCode: code,
            errorMessage: message,
            startedAt: now,
            endedAt: now,
            createdAt: now,
            updatedAt: now
        )
        try await store.upsertRecording(recording)
        try await store.upsertAgentTask(AgentTask(
            id: taskID,
            agentType: "recording",
            recordingID: recordingID,
            status: .failed,
            inputJSON: "{}",
            outputJSON: "{}",
            errorCode: code,
            errorMessage: message,
            startedAt: now,
            finishedAt: now,
            createdAt: now,
            updatedAt: now
        ))
        try await store.insertAgentTaskEvent(AgentTaskEvent(
            id: UUID().uuidString,
            taskID: taskID,
            eventType: "failed",
            status: "failed",
            progress: 0,
            message: message,
            errorCode: code,
            errorMessage: message,
            payloadJSON: "{}",
            createdAt: now
        ))
    }

    private func persist(slice: AudioSlice, taskID: String) async throws {
        let now = clock.now()
        try await store.insertAudioSegment(AudioSegment(
            id: slice.id,
            recordingID: slice.recordingID,
            segmentIndex: slice.index,
            filePath: slice.fileURL.path,
            startMilliseconds: slice.startMilliseconds,
            endMilliseconds: slice.endMilliseconds,
            durationMilliseconds: slice.durationMilliseconds,
            sampleRate: slice.sampleRate,
            channels: slice.channels,
            status: .transcribing,
            errorCode: nil,
            errorMessage: nil,
            createdAt: now
        ))
        try await store.insertAgentTaskStep(AgentTaskStep(
            id: UUID().uuidString,
            taskID: taskID,
            stepType: "slice",
            stepIndex: slice.index,
            status: .succeeded,
            refType: "audio_segment",
            refID: slice.id,
            inputJSON: "{}",
            outputJSON: #"{"startMs":\#(slice.startMilliseconds),"endMs":\#(slice.endMilliseconds)}"#,
            errorCode: nil,
            errorMessage: nil,
            startedAt: now,
            finishedAt: now,
            createdAt: now,
            updatedAt: now
        ))
        try await store.insertAgentTaskArtifact(AgentTaskArtifact(
            id: UUID().uuidString,
            taskID: taskID,
            recordingID: slice.recordingID,
            artifactType: .audioSlice,
            uri: slice.fileURL.path,
            refType: "audio_segment",
            refID: slice.id,
            metadataJSON: #"{"index":\#(slice.index)}"#,
            createdAt: now
        ))
    }

    private func transcribe(slice: AudioSlice, taskID: String) async throws {
        let now = clock.now()
        switch await asrEngine.transcribe(slice: slice, model: activeModel, language: activeLanguage) {
        case let .success(result):
            let segment = TranscriptSegment(
                id: UUID().uuidString,
                recordingID: slice.recordingID,
                audioSegmentID: slice.id,
                segmentIndex: slice.index,
                startMilliseconds: slice.startMilliseconds,
                endMilliseconds: slice.endMilliseconds,
                text: result.text,
                language: result.language,
                status: .success,
                provider: result.provider,
                modelName: result.modelName,
                errorCode: nil,
                errorMessage: nil,
                createdAt: now
            )
            try await store.insertTranscriptSegment(segment)
            try await store.updateAudioSegmentStatus(id: slice.id, status: .transcribed)
            confirmedSegments.append(segment)
            try await store.insertAgentTaskStep(AgentTaskStep(
                id: UUID().uuidString,
                taskID: taskID,
                stepType: "transcribe",
                stepIndex: slice.index,
                status: .succeeded,
                refType: "transcript_segment",
                refID: segment.id,
                inputJSON: "{}",
                outputJSON: #"{"provider":"\#(result.provider)","language":"\#(result.language)","model":"\#(result.modelName.rawValue)"}"#,
                errorCode: nil,
                errorMessage: nil,
                startedAt: now,
                finishedAt: now,
                createdAt: now,
                updatedAt: now
            ))
            try await store.insertAgentTaskArtifact(AgentTaskArtifact(
                id: UUID().uuidString,
                taskID: taskID,
                recordingID: slice.recordingID,
                artifactType: .transcriptSegment,
                uri: "",
                refType: "transcript_segment",
                refID: segment.id,
                metadataJSON: #"{"provider":"\#(result.provider)"}"#,
                createdAt: now
            ))
        case let .failure(error):
            let segment = TranscriptSegment(
                id: UUID().uuidString,
                recordingID: slice.recordingID,
                audioSegmentID: slice.id,
                segmentIndex: slice.index,
                startMilliseconds: slice.startMilliseconds,
                endMilliseconds: slice.endMilliseconds,
                text: "",
                language: activeLanguage.rawValue,
                status: .failed,
                provider: "whisperkit",
                modelName: activeModel,
                errorCode: error.code,
                errorMessage: error.message,
                createdAt: now
            )
            try await store.insertTranscriptSegment(segment)
            try await store.updateAudioSegmentStatus(id: slice.id, status: .failed, errorCode: error.code, errorMessage: error.message)
            try await store.insertAgentTaskStep(AgentTaskStep(
                id: UUID().uuidString,
                taskID: taskID,
                stepType: "transcribe",
                stepIndex: slice.index,
                status: .failed,
                refType: "transcript_segment",
                refID: segment.id,
                inputJSON: "{}",
                outputJSON: "{}",
                errorCode: error.code,
                errorMessage: error.message,
                startedAt: now,
                finishedAt: now,
                createdAt: now,
                updatedAt: now
            ))
        }
    }

    private func recordEvent(type: String, status: String, progress: Int, message: String) async throws {
        guard let taskID = activeTaskID else {
            return
        }
        try await store.insertAgentTaskEvent(AgentTaskEvent(
            id: UUID().uuidString,
            taskID: taskID,
            eventType: type,
            status: status,
            progress: progress,
            message: message,
            errorCode: nil,
            errorMessage: nil,
            payloadJSON: "{}",
            createdAt: clock.now()
        ))
    }

    private func failActiveRecording(recordingID: String, taskID: String, code: RecordingAgentErrorCode, message: String) async throws {
        defer {
            clearActiveRecording()
        }
        let now = clock.now()
        state = .failed

        if var recording = try await store.fetchRecordingDetail(id: recordingID)?.recording {
            recording.status = .failed
            recording.errorCode = code
            recording.errorMessage = message
            recording.endedAt = now
            recording.updatedAt = now
            try await store.upsertRecording(recording)
            try await store.updateRecordingCounts(recordingID: recordingID)
        }

        try await store.upsertAgentTask(AgentTask(
            id: taskID,
            agentType: "recording",
            recordingID: recordingID,
            status: .failed,
            inputJSON: "{}",
            outputJSON: "{}",
            errorCode: code,
            errorMessage: message,
            startedAt: now,
            finishedAt: now,
            createdAt: now,
            updatedAt: now
        ))
        try await store.insertAgentTaskEvent(AgentTaskEvent(
            id: UUID().uuidString,
            taskID: taskID,
            eventType: "failed",
            status: "failed",
            progress: 0,
            message: message,
            errorCode: code,
            errorMessage: message,
            payloadJSON: "{}",
            createdAt: now
        ))
    }

    private func normalize(_ error: Error, fallback: RecordingAgentErrorCode) -> RecordingAgentError {
        if let agentError = error as? RecordingAgentError {
            return agentError
        }
        return RecordingAgentError(code: fallback, message: error.localizedDescription)
    }

    private func clearActiveRecording() {
        activeRecordingID = nil
        activeTaskID = nil
        activeAudioStream = nil
        activeOutputDirectory = nil
        activeStartedAt = nil
    }
}
