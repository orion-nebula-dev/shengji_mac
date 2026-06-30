import AVFoundation
import XCTest
@testable import RecordingAgentCore

final class RecordingAgentTests: XCTestCase {
    func testMockPipelineRecordsConfirmedSegmentsAndAgentFacts() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()

        let capture = MockAudioCaptureService(
            permission: .authorized,
            slices: [
                .mock(index: 0, start: 0, end: 15_000),
                .mock(index: 1, start: 15_000, end: 30_000)
            ]
        )
        let modelManager = MockModelManager(readyModels: [.small])
        let asr = MockASREngine(results: [
            0: .success(text: "第一段确认文本", language: "zh"),
            1: .failure(code: .transcribeFailed, message: "模拟单片失败")
        ])
        let agent = RecordingAgent(
            store: store,
            permissionService: capture,
            audioCaptureService: capture,
            modelManager: modelManager,
            asrEngine: asr,
            clock: .fixed(Date(timeIntervalSince1970: 1_782_432_000))
        )

        try await agent.startRecording(model: .small, language: .auto)
        try await agent.stopRecording()

        let state = await agent.state
        let confirmedSegments = await agent.confirmedSegments
        XCTAssertEqual(state, .completed)
        XCTAssertEqual(confirmedSegments.map(\.text), ["第一段确认文本"])

        let history = try await store.fetchRecordings()
        XCTAssertEqual(history.count, 1)
        XCTAssertEqual(history.first?.status, .completed)
        XCTAssertEqual(history.first?.transcriptSegmentCount, 1)
        XCTAssertEqual(history.first?.failedSegmentCount, 1)

        let detail = try await store.fetchRecordingDetail(id: history[0].id)
        XCTAssertEqual(detail?.audioSegments.count, 2)
        XCTAssertTrue(detail?.events.contains(where: { $0.eventType == "transcribing" }) == true)
        XCTAssertTrue(detail?.steps.contains(where: { $0.stepType == "transcribe" && $0.status == .succeeded }) == true)
        XCTAssertTrue(detail?.steps.contains(where: { $0.stepType == "transcribe" && $0.status == .failed }) == true)
        XCTAssertTrue(detail?.artifacts.contains(where: { $0.artifactType == .wavAudio }) == true)
        XCTAssertTrue(detail?.artifacts.contains(where: { $0.artifactType == .transcriptSegment }) == true)
    }

    func testModelMissingDoesNotEnterRecording() async throws {
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
            XCTFail("Expected model_missing")
        } catch let error as RecordingAgentError {
            XCTAssertEqual(error.code, .modelMissing)
            let state = await agent.state
            XCTAssertEqual(state, .failed)
        }
    }

    func testPrepareModelFailureDoesNotLeaveAgentDownloadingForever() async throws {
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
            XCTFail("Expected model_missing")
        } catch let error as RecordingAgentError {
            XCTAssertEqual(error.code, .modelMissing)
            let state = await agent.state
            XCTAssertEqual(state, .failed)
        }
    }

    func testDuplicateStartIsRejectedWhileRecordingIsActive() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()

        let capture = MockAudioCaptureService(permission: .authorized, slices: [])
        let agent = RecordingAgent(
            store: store,
            permissionService: capture,
            audioCaptureService: capture,
            modelManager: MockModelManager(readyModels: [.small]),
            asrEngine: MockASREngine(results: [:]),
            clock: .fixed(Date(timeIntervalSince1970: 1_782_432_000))
        )

        try await agent.startRecording(model: .small, language: .auto)

        do {
            try await agent.startRecording(model: .small, language: .auto)
            XCTFail("Expected duplicate start to be rejected")
        } catch let error as RecordingAgentError {
            XCTAssertEqual(error.code, .writeFailed)
        }

        let history = try await store.fetchRecordings()
        XCTAssertEqual(history.count, 1)

        try await agent.stopRecording()
    }

    func testRuntimeSnapshotReportsElapsedAndInputLevelWhileRecording() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()

        final class TestClockSource: @unchecked Sendable {
            var now = Date(timeIntervalSince1970: 1_782_432_000)
        }

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
        XCTAssertEqual(inactiveSnapshot, .inactive)

        try await agent.startRecording(model: .small, language: .auto)
        clockSource.now = clockSource.now.addingTimeInterval(12.345)

        let snapshot = await agent.runtimeSnapshot()
        XCTAssertNotNil(snapshot.recordingID)
        XCTAssertEqual(snapshot.elapsedMilliseconds, 12_345)
        XCTAssertEqual(snapshot.elapsedText, "0:12")
        XCTAssertEqual(snapshot.inputLevel.percent, 62)

        try await agent.stopRecording()
        let stoppedSnapshot = await agent.runtimeSnapshot()
        XCTAssertEqual(stoppedSnapshot, .inactive)
    }

    func testRecordingOutputDirectoryDefaultsToApplicationSupportRecordings() async throws {
        let store = try SQLiteRecordingStore.inMemory()
        try await store.migrate()

        let capture = ConfigurationCapturingAudioCaptureService(permission: .authorized)
        let agent = RecordingAgent(
            store: store,
            permissionService: capture,
            audioCaptureService: capture,
            modelManager: MockModelManager(readyModels: [.small]),
            asrEngine: MockASREngine(results: [:]),
            clock: .fixed(Date(timeIntervalSince1970: 1_782_432_000))
        )

        try await agent.startRecording(model: .small, language: .auto)
        let outputDirectory = try XCTUnwrap(capture.lastStartConfiguration?.outputDirectory.standardizedFileURL)
        let appSupportRecordings = try XCTUnwrap(FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first)
            .appending(path: "com.shengji.recording-agent")
            .appending(path: "recordings")
            .standardizedFileURL
        let temporaryDirectory = URL(fileURLWithPath: NSTemporaryDirectory()).standardizedFileURL.path

        XCTAssertTrue(outputDirectory.path.hasPrefix(appSupportRecordings.path + "/"))
        XCTAssertFalse(outputDirectory.path.hasPrefix(temporaryDirectory))

        try await agent.stopRecording()
    }

    func testAudioSliceCoordinatorClosesCompletedSliceBeforePublishing() throws {
        let outputDirectory = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentSliceCoordinatorTests-\(UUID().uuidString)")
        let configuration = AudioCaptureConfiguration(
            recordingID: "recording-id",
            outputDirectory: outputDirectory,
            sampleRate: 1_000,
            channels: 1,
            sliceDurationMilliseconds: 10
        )
        let format = try XCTUnwrap(AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 1_000, channels: 1, interleaved: false))
        let factory = RecordingAudioFileWriterFactory()
        var wasClosedWhenPublished: [Bool] = []
        let coordinator = AudioSliceFileCoordinator(
            configuration: configuration,
            targetFormat: format,
            fileWriterFactory: factory
        ) { _ in
            wasClosedWhenPublished.append(factory.createdWriters.first?.isClosed == true)
        }
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 10))
        buffer.frameLength = 10

        try coordinator.start()
        try coordinator.write(buffer: buffer)

        XCTAssertEqual(wasClosedWhenPublished, [true])
    }
}

private final class ConfigurationCapturingAudioCaptureService: MicrophonePermissionService, AudioCaptureService, @unchecked Sendable {
    private let permission: MicrophonePermissionStatus
    private(set) var lastStartConfiguration: AudioCaptureConfiguration?

    init(permission: MicrophonePermissionStatus) {
        self.permission = permission
    }

    func requestMicrophonePermission() async -> MicrophonePermissionStatus {
        permission
    }

    func start(configuration: AudioCaptureConfiguration) async throws -> AsyncThrowingStream<AudioSlice, Error> {
        lastStartConfiguration = configuration
        return AsyncThrowingStream { continuation in
            continuation.finish()
        }
    }

    func stop() async throws -> AudioCaptureResult {
        let configuration = try XCTUnwrap(lastStartConfiguration)
        return AudioCaptureResult(
            recordingID: configuration.recordingID,
            fullAudioURL: configuration.outputDirectory.appending(path: "full.wav"),
            durationMilliseconds: 0,
            sampleRate: configuration.sampleRate,
            channels: configuration.channels
        )
    }
}

private final class RecordingAudioFileWriter: AudioFileWriting {
    private(set) var isClosed = false

    func write(from buffer: AVAudioPCMBuffer) throws {}

    func close() {
        isClosed = true
    }
}

private final class RecordingAudioFileWriterFactory: AudioFileWriterFactory {
    private(set) var createdWriters: [RecordingAudioFileWriter] = []

    func makeWriter(url: URL, settings: [String: Any]) throws -> AudioFileWriting {
        let writer = RecordingAudioFileWriter()
        createdWriters.append(writer)
        return writer
    }
}
