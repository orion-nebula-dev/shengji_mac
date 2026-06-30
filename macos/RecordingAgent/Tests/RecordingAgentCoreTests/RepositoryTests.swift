import XCTest
@testable import RecordingAgentCore

final class RepositoryTests: XCTestCase {
    func testSQLiteRepositoryInitializesAllMVPTablesAndFetchesDetail() async throws {
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

        let slice = AudioSegment(
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
        )
        try await store.insertAudioSegment(slice)

        let transcript = TranscriptSegment(
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
        )
        try await store.insertTranscriptSegment(transcript)

        let task = AgentTask(
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
        )
        try await store.upsertAgentTask(task)
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

        XCTAssertEqual(detail?.recording.title, "2026-06-26 00:00 录音")
        XCTAssertEqual(detail?.segments.map(\.text), ["hello world"])
        XCTAssertEqual(detail?.events.map(\.message), ["录音中"])
        XCTAssertEqual(detail?.steps.map(\.refID), ["slice-1"])
        XCTAssertEqual(detail?.artifacts.map(\.artifactType), [.audioSlice])

        let history = try await store.fetchRecordings()
        XCTAssertEqual(history.map(\.id), ["rec-1"])
        XCTAssertEqual(history.first?.transcriptSegmentCount, 1)
    }
}
