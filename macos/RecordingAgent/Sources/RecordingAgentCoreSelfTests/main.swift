import Foundation
import RecordingAgentCore

struct TestFailure: Error, CustomStringConvertible {
    let description: String
}

func expect(_ condition: @autoclosure () -> Bool, _ message: String) throws {
    if !condition() {
        throw TestFailure(description: message)
    }
}

@main
struct RecordingAgentCoreSelfTests {
    static func main() async {
        do {
            try testDomain()
            try testSlicePlanner()
            try testAudioInputLevel()
            try testModelDownloadFileProgress()
            try await testSQLiteRepository()
            try await testMockPipeline()
            try await testModelMissing()
            try await testRuntimeSnapshot()
            try await testPrepareModelFailureState()
            try await testCaptureStartFailurePersistsFailedRecording()
            print("SELFTEST PASS: RecordingAgentCore")
        } catch {
            fputs("SELFTEST FAIL: \(error)\n", stderr)
            exit(1)
        }
    }

    static func testDomain() throws {
        try expect(ASRModelName.default == .small, "default model should be small")
        try expect(ASRModelName.allCases.map(\.rawValue) == ["tiny", "base", "small"], "model options should be tiny/base/small")
        try expect(LanguageMode.allCases.map(\.rawValue) == ["auto", "zh", "en"], "language modes should be auto/zh/en")
        try expect(LanguageMode.allCases.map(\.displayName) == ["自动识别", "中文", "英文"], "language display names should be localized")
        try expect(
            LocalASRDecodingPolicy.whisperKitPolicy(for: .auto) == LocalASRDecodingPolicy(taskName: "transcribe", languageCode: nil, shouldDetectLanguage: true, temperatureFallbackCount: 1),
            "auto should use WhisperKit language detection"
        )
        try expect(
            LocalASRDecodingPolicy.whisperKitPolicy(for: .zh) == LocalASRDecodingPolicy(taskName: "transcribe", languageCode: "zh", shouldDetectLanguage: false, temperatureFallbackCount: 1),
            "zh should force Chinese transcription"
        )
        try expect(
            LocalASRDecodingPolicy.whisperKitPolicy(for: .en) == LocalASRDecodingPolicy(taskName: "transcribe", languageCode: "en", shouldDetectLanguage: false, temperatureFallbackCount: 1),
            "en should force English transcription"
        )
        switch ASRTranscriptPostProcessor.process(text: "(字幕製作:貝爾)") {
        case .failure:
            break
        case .success:
            throw TestFailure(description: "subtitle-style hallucinations should be rejected")
        }
        try expect(
            RecordingAgentErrorCode.allCases.map(\.rawValue) == [
                "permission_denied",
                "model_missing",
                "download_failed",
                "verify_failed",
                "asr_engine_unavailable",
                "write_failed",
                "transcribe_failed"
            ],
            "error code list drifted"
        )
    }

    static func testSlicePlanner() throws {
        let windows = AudioSlicePlanner.planWindows(durationMilliseconds: 31_200)
        try expect(windows.map(\.index) == [0, 1, 2], "slice indexes should be fixed windows")
        try expect(windows.map(\.startMilliseconds) == [0, 15_000, 30_000], "slice starts should be 15s apart")
        try expect(windows.map(\.endMilliseconds) == [15_000, 30_000, 31_200], "tail slice should end at duration")
        try expect(AudioSlicePlanner.planWindows(durationMilliseconds: 0).isEmpty, "zero duration should not create slices")
    }

    static func testAudioInputLevel() throws {
        try expect(AudioInputLevel(linear: -0.2).linear == 0, "input level should clamp low values")
        try expect(AudioInputLevel(linear: 1.7).linear == 1, "input level should clamp high values")
        try expect(AudioInputLevel(linear: 0.428).displayText == "43%", "input level should format percent")
    }

    static func testModelDownloadFileProgress() throws {
        let progress = ModelDownloadFileProgress(
            fileName: "weight.bin",
            fileIndex: 1,
            totalFileCount: 4,
            completedBytes: 1_572_864,
            totalBytes: 3_145_728
        )
        try expect(progress.filePercent == 50, "file byte progress should compute percent")
        try expect(progress.overallCompletedUnitCount == 150, "overall model progress should include completed files")
        try expect(progress.displayMessage == "weight.bin · 1.5 MB / 3.0 MB", "byte progress message should be readable")
        let preparationProgress = progress.preparationProgress(modelName: .small)
        try expect(preparationProgress.percent == 37, "overall preparation percent should be derived from file progress")
    }

    static func testSQLiteRepository() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()

        let now = Date(timeIntervalSince1970: 1_782_432_000)
        let recording = Recording(
            id: "rec-1",
            title: Recording.defaultTitle(now: now, calendar: .gregorianUTC),
            status: .recording,
            audioFilePath: "/tmp/full.wav",
            durationMilliseconds: 30_000,
            sampleRate: 16_000,
            channels: 1,
            modelName: .small,
            languageMode: .auto,
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
        try await store.insertAudioSegment(.init(
            id: "slice-1",
            recordingID: "rec-1",
            segmentIndex: 0,
            filePath: "/tmp/slices/000000.wav",
            startMilliseconds: 0,
            endMilliseconds: 15_000,
            durationMilliseconds: 15_000,
            sampleRate: 16_000,
            channels: 1,
            status: .transcribed,
            errorCode: nil,
            errorMessage: nil,
            createdAt: now
        ))
        try await store.insertTranscriptSegment(.init(
            id: "ts-1",
            recordingID: "rec-1",
            audioSegmentID: "slice-1",
            segmentIndex: 0,
            startMilliseconds: 0,
            endMilliseconds: 15_000,
            text: "hello world",
            language: "en",
            status: .success,
            provider: "mock",
            modelName: .small,
            errorCode: nil,
            errorMessage: nil,
            createdAt: now
        ))
        try await store.upsertAgentTask(.init(
            id: "task-1",
            agentType: "recording",
            recordingID: "rec-1",
            status: .recording,
            inputJSON: "{}",
            outputJSON: "{}",
            errorCode: nil,
            errorMessage: nil,
            startedAt: now,
            finishedAt: nil,
            createdAt: now,
            updatedAt: now
        ))
        try await store.insertAgentTaskEvent(.init(
            id: "event-1",
            taskID: "task-1",
            eventType: "recording",
            status: "started",
            progress: 20,
            message: "录音中",
            errorCode: nil,
            errorMessage: nil,
            payloadJSON: "{}",
            createdAt: now
        ))
        try await store.insertAgentTaskStep(.init(
            id: "step-1",
            taskID: "task-1",
            stepType: "slice",
            stepIndex: 0,
            status: .succeeded,
            refType: "audio_segment",
            refID: "slice-1",
            inputJSON: "{}",
            outputJSON: "{}",
            errorCode: nil,
            errorMessage: nil,
            startedAt: now,
            finishedAt: now,
            createdAt: now,
            updatedAt: now
        ))
        try await store.insertAgentTaskArtifact(.init(
            id: "artifact-1",
            taskID: "task-1",
            recordingID: "rec-1",
            artifactType: .audioSlice,
            uri: "/tmp/slices/000000.wav",
            refType: "audio_segment",
            refID: "slice-1",
            metadataJSON: "{}",
            createdAt: now
        ))

        try await store.updateRecordingCounts(recordingID: "rec-1")
        let detail = try await store.fetchRecordingDetail(id: "rec-1")
        try expect(detail?.recording.title == "2026-06-26 00:00 录音", "default title should be stable")
        try expect(detail?.segments.map(\.text) == ["hello world"], "detail should include transcript")
        try expect(detail?.events.map(\.message) == ["录音中"], "detail should include agent events")
        try expect(detail?.steps.map(\.refID) == ["slice-1"], "detail should include agent steps")
        try expect(detail?.artifacts.map(\.artifactType) == [.audioSlice], "detail should include artifacts")
        let history = try await store.fetchRecordings()
        try expect(history.map(\.id) == ["rec-1"], "history should return recording")
        try expect(history.first?.transcriptSegmentCount == 1, "recording count should be updated")
    }

    static func testMockPipeline() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()
        let capture = MockAudioCaptureService(
            permission: .authorized,
            slices: [.mock(index: 0, start: 0, end: 15_000), .mock(index: 1, start: 15_000, end: 30_000)]
        )
        let agent = RecordingAgent(
            store: store,
            permissionService: capture,
            audioCaptureService: capture,
            modelManager: MockModelManager(readyModels: [.small]),
            asrEngine: MockASREngine(results: [
                0: .success(text: "第一段确认文本", language: "zh"),
                1: .failure(code: .transcribeFailed, message: "模拟单片失败")
            ]),
            clock: .fixed(Date(timeIntervalSince1970: 1_782_432_000))
        )

        try await agent.startRecording(model: .small, language: .auto)
        try await Task.sleep(nanoseconds: 100_000_000)
        let liveConfirmedSegments = await agent.confirmedSegments
        try expect(liveConfirmedSegments.map(\.text) == ["第一段确认文本"], "confirmed segments should update while recording")
        try await agent.stopRecording()
        let completedState = await agent.state
        let completedConfirmedSegments = await agent.confirmedSegments
        try expect(completedState == .completed, "agent should complete when one slice fails")
        try expect(completedConfirmedSegments.map(\.text) == ["第一段确认文本"], "confirmed segments should only include successes")
        let history = try await store.fetchRecordings()
        let detail = try await store.fetchRecordingDetail(id: history[0].id)
        let statusSummary = detail?.segments.map { "\($0.recordingID):\($0.status.rawValue):\($0.text)" }.joined(separator: ",") ?? "nil"
        let confirmedSummary = completedConfirmedSegments.map { "\($0.recordingID):\($0.status.rawValue):\($0.text)" }.joined(separator: ",")
        try expect(history.first?.transcriptSegmentCount == 1, "success count should be one; got \(history.first?.transcriptSegmentCount ?? -1), history \(history.first?.id ?? "nil"), confirmed \(confirmedSummary), detail \(statusSummary)")
        try expect(history.first?.failedSegmentCount == 1, "failed count should be one; got \(history.first?.failedSegmentCount ?? -1), segments \(statusSummary)")
        try expect(detail?.audioSegments.count == 2, "detail should include two audio slices")
        try expect(detail?.events.contains(where: { $0.eventType == "transcribing" }) == true, "events should include transcribing")
        try expect(detail?.artifacts.contains(where: { $0.artifactType == .wavAudio }) == true, "artifacts should include WAV")
        try expect(detail?.artifacts.contains(where: { $0.artifactType == .transcriptSegment }) == true, "artifacts should include transcript segment")
    }

    static func testModelMissing() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()
        let capture = MockAudioCaptureService(permission: .authorized, slices: [])
        let agent = RecordingAgent(
            store: store,
            permissionService: capture,
            audioCaptureService: capture,
            modelManager: MockModelManager(readyModels: []),
            asrEngine: MockASREngine(results: [:]),
            clock: .fixed(Date(timeIntervalSince1970: 1_782_432_000))
        )

        do {
            try await agent.startRecording(model: .small, language: .auto)
            throw TestFailure(description: "expected model_missing")
        } catch let error as RecordingAgentError {
            try expect(error.code == .modelMissing, "model missing should be reported")
            let state = await agent.state
            try expect(state == .failed, "agent should enter failed state")
        }
    }

    static func testRuntimeSnapshot() async throws {
        final class TestClockSource: @unchecked Sendable {
            var now = Date(timeIntervalSince1970: 1_782_432_000)
        }

        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()
        let clockSource = TestClockSource()
        let capture = MockAudioCaptureService(
            permission: .authorized,
            slices: [],
            inputLevel: AudioInputLevel(linear: 0.62)
        )
        let agent = RecordingAgent(
            store: store,
            permissionService: capture,
            audioCaptureService: capture,
            modelManager: MockModelManager(readyModels: [.small]),
            asrEngine: MockASREngine(results: [:]),
            clock: Clock { clockSource.now }
        )

        let inactiveSnapshot = await agent.runtimeSnapshot()
        try expect(inactiveSnapshot == .inactive, "inactive agent should return inactive runtime snapshot")
        try await agent.startRecording(model: .small, language: .auto)
        clockSource.now = clockSource.now.addingTimeInterval(12.345)
        let snapshot = await agent.runtimeSnapshot()
        try expect(snapshot.recordingID != nil, "recording snapshot should include active recording id")
        try expect(snapshot.elapsedMilliseconds == 12_345, "recording snapshot should include elapsed time")
        try expect(snapshot.elapsedText == "0:12", "recording snapshot should format elapsed time")
        try expect(snapshot.inputLevel.percent == 62, "recording snapshot should include input level")
        try await agent.stopRecording()
        let stoppedSnapshot = await agent.runtimeSnapshot()
        try expect(stoppedSnapshot == .inactive, "stopped agent should return inactive runtime snapshot")
    }

    static func testPrepareModelFailureState() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()
        let capture = MockAudioCaptureService(permission: .authorized, slices: [])
        let agent = RecordingAgent(
            store: store,
            permissionService: capture,
            audioCaptureService: capture,
            modelManager: MockModelManager(readyModels: []),
            asrEngine: MockASREngine(results: [:]),
            clock: .fixed(Date(timeIntervalSince1970: 1_782_432_000))
        )

        do {
            try await agent.prepareModel(.small, trigger: .settings)
            throw TestFailure(description: "expected model_missing during prepare")
        } catch let error as RecordingAgentError {
            try expect(error.code == .modelMissing, "prepare failure should report model_missing")
            let state = await agent.state
            try expect(state == .failed, "prepare failure should not leave agent downloading")
        }
    }

    static func testCaptureStartFailurePersistsFailedRecording() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()
        let capture = MockAudioCaptureService(
            permission: .authorized,
            slices: [],
            startError: RecordingAgentError(code: .writeFailed, message: "模拟写盘失败")
        )
        let agent = RecordingAgent(
            store: store,
            permissionService: capture,
            audioCaptureService: capture,
            modelManager: MockModelManager(readyModels: [.small]),
            asrEngine: MockASREngine(results: [:]),
            clock: .fixed(Date(timeIntervalSince1970: 1_782_432_000))
        )

        do {
            try await agent.startRecording(model: .small, language: .auto)
            throw TestFailure(description: "expected write_failed")
        } catch let error as RecordingAgentError {
            try expect(error.code == .writeFailed, "capture start failure should map to write_failed")
            let state = await agent.state
            try expect(state == .failed, "agent should enter failed state after capture start failure")
        }

        let history = try await store.fetchRecordings()
        try expect(history.count == 1, "failed capture start should leave one recording fact")
        try expect(history[0].status == .failed, "failed capture start should mark recording failed")
        try expect(history[0].errorCode == .writeFailed, "failed capture start should persist write_failed")
    }
}
