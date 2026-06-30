import Foundation
import SQLite3

public enum SQLiteRecordingStoreError: Error, CustomStringConvertible, Sendable {
    case openFailed(String)
    case prepareFailed(String)
    case stepFailed(String)
    case missingRecording(String)

    public var description: String {
        switch self {
        case let .openFailed(message): return "SQLite open failed: \(message)"
        case let .prepareFailed(message): return "SQLite prepare failed: \(message)"
        case let .stepFailed(message): return "SQLite step failed: \(message)"
        case let .missingRecording(id): return "Missing recording: \(id)"
        }
    }
}

enum SQLiteValue {
    case text(String)
    case int(Int)
    case double(Double)
    case null
}

public actor SQLiteRecordingStore {
    private let db: OpaquePointer

    private init(db: OpaquePointer) {
        self.db = db
    }

    deinit {
        sqlite3_close(db)
    }

    public static func inMemory() throws -> SQLiteRecordingStore {
        try open(path: ":memory:")
    }

    public static func open(path: String) throws -> SQLiteRecordingStore {
        var db: OpaquePointer?
        let flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX
        guard sqlite3_open_v2(path, &db, flags, nil) == SQLITE_OK, let db else {
            let message = db.map { String(cString: sqlite3_errmsg($0)) } ?? "unknown"
            throw SQLiteRecordingStoreError.openFailed(message)
        }
        sqlite3_exec(db, "PRAGMA foreign_keys = ON;", nil, nil, nil)
        return SQLiteRecordingStore(db: db)
    }

    public func migrate() throws {
        try executeScript("""
        CREATE TABLE IF NOT EXISTS recordings (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          status TEXT NOT NULL,
          audio_file_path TEXT NOT NULL DEFAULT '',
          duration_ms INTEGER NOT NULL DEFAULT 0,
          sample_rate INTEGER NOT NULL DEFAULT 16000,
          channels INTEGER NOT NULL DEFAULT 1,
          model_name TEXT NOT NULL DEFAULT 'small',
          language_mode TEXT NOT NULL DEFAULT 'auto',
          transcript_segment_count INTEGER NOT NULL DEFAULT 0,
          failed_segment_count INTEGER NOT NULL DEFAULT 0,
          error_code TEXT NOT NULL DEFAULT '',
          error_message TEXT NOT NULL DEFAULT '',
          started_at REAL NOT NULL,
          ended_at REAL,
          created_at REAL NOT NULL,
          updated_at REAL NOT NULL
        );

        CREATE TABLE IF NOT EXISTS audio_segments (
          id TEXT PRIMARY KEY,
          recording_id TEXT NOT NULL,
          segment_index INTEGER NOT NULL,
          file_path TEXT NOT NULL,
          start_ms INTEGER NOT NULL,
          end_ms INTEGER NOT NULL,
          duration_ms INTEGER NOT NULL,
          sample_rate INTEGER NOT NULL DEFAULT 16000,
          channels INTEGER NOT NULL DEFAULT 1,
          status TEXT NOT NULL,
          error_code TEXT NOT NULL DEFAULT '',
          error_message TEXT NOT NULL DEFAULT '',
          created_at REAL NOT NULL,
          FOREIGN KEY (recording_id) REFERENCES recordings(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS transcript_segments (
          id TEXT PRIMARY KEY,
          recording_id TEXT NOT NULL,
          audio_segment_id TEXT NOT NULL,
          segment_index INTEGER NOT NULL,
          start_ms INTEGER NOT NULL,
          end_ms INTEGER NOT NULL,
          text TEXT NOT NULL DEFAULT '',
          language TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL,
          provider TEXT NOT NULL DEFAULT 'whisperkit',
          model_name TEXT NOT NULL DEFAULT 'small',
          error_code TEXT NOT NULL DEFAULT '',
          error_message TEXT NOT NULL DEFAULT '',
          created_at REAL NOT NULL,
          FOREIGN KEY (recording_id) REFERENCES recordings(id) ON DELETE CASCADE,
          FOREIGN KEY (audio_segment_id) REFERENCES audio_segments(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS agent_tasks (
          id TEXT PRIMARY KEY,
          agent_type TEXT NOT NULL DEFAULT 'recording',
          recording_id TEXT NOT NULL,
          status TEXT NOT NULL,
          input_json TEXT NOT NULL DEFAULT '{}',
          output_json TEXT NOT NULL DEFAULT '{}',
          error_code TEXT NOT NULL DEFAULT '',
          error_message TEXT NOT NULL DEFAULT '',
          started_at REAL NOT NULL,
          finished_at REAL,
          created_at REAL NOT NULL,
          updated_at REAL NOT NULL
        );

        CREATE TABLE IF NOT EXISTS agent_task_events (
          id TEXT PRIMARY KEY,
          task_id TEXT NOT NULL,
          event_type TEXT NOT NULL,
          status TEXT NOT NULL,
          progress INTEGER NOT NULL DEFAULT 0,
          message TEXT NOT NULL DEFAULT '',
          error_code TEXT NOT NULL DEFAULT '',
          error_message TEXT NOT NULL DEFAULT '',
          payload_json TEXT NOT NULL DEFAULT '{}',
          created_at REAL NOT NULL,
          FOREIGN KEY (task_id) REFERENCES agent_tasks(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS agent_task_steps (
          id TEXT PRIMARY KEY,
          task_id TEXT NOT NULL,
          step_type TEXT NOT NULL,
          step_index INTEGER NOT NULL DEFAULT 0,
          status TEXT NOT NULL,
          ref_type TEXT NOT NULL DEFAULT '',
          ref_id TEXT NOT NULL DEFAULT '',
          input_json TEXT NOT NULL DEFAULT '{}',
          output_json TEXT NOT NULL DEFAULT '{}',
          error_code TEXT NOT NULL DEFAULT '',
          error_message TEXT NOT NULL DEFAULT '',
          started_at REAL,
          finished_at REAL,
          created_at REAL NOT NULL,
          updated_at REAL NOT NULL,
          FOREIGN KEY (task_id) REFERENCES agent_tasks(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS agent_task_artifacts (
          id TEXT PRIMARY KEY,
          task_id TEXT NOT NULL,
          recording_id TEXT NOT NULL,
          artifact_type TEXT NOT NULL,
          uri TEXT NOT NULL DEFAULT '',
          ref_type TEXT NOT NULL DEFAULT '',
          ref_id TEXT NOT NULL DEFAULT '',
          metadata_json TEXT NOT NULL DEFAULT '{}',
          created_at REAL NOT NULL,
          FOREIGN KEY (task_id) REFERENCES agent_tasks(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS asr_model_assets (
          id TEXT PRIMARY KEY,
          provider TEXT NOT NULL DEFAULT 'whisperkit',
          model_name TEXT NOT NULL,
          display_name TEXT NOT NULL,
          status TEXT NOT NULL,
          download_progress INTEGER NOT NULL DEFAULT 0,
          local_path TEXT NOT NULL DEFAULT '',
          checksum TEXT NOT NULL DEFAULT '',
          size_bytes INTEGER NOT NULL DEFAULT 0,
          is_default INTEGER NOT NULL DEFAULT 0,
          error_message TEXT NOT NULL DEFAULT '',
          created_at REAL NOT NULL,
          updated_at REAL NOT NULL,
          UNIQUE(provider, model_name)
        );
        """)
    }

    public func upsertRecording(_ recording: Recording) throws {
        try execute(
            """
            INSERT INTO recordings (
              id, title, status, audio_file_path, duration_ms, sample_rate, channels,
              model_name, language_mode, transcript_segment_count, failed_segment_count,
              error_code, error_message, started_at, ended_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              status = excluded.status,
              audio_file_path = excluded.audio_file_path,
              duration_ms = excluded.duration_ms,
              sample_rate = excluded.sample_rate,
              channels = excluded.channels,
              model_name = excluded.model_name,
              language_mode = excluded.language_mode,
              transcript_segment_count = excluded.transcript_segment_count,
              failed_segment_count = excluded.failed_segment_count,
              error_code = excluded.error_code,
              error_message = excluded.error_message,
              started_at = excluded.started_at,
              ended_at = excluded.ended_at,
              created_at = excluded.created_at,
              updated_at = excluded.updated_at;
            """,
            [
                .text(recording.id),
                .text(recording.title),
                .text(recording.status.rawValue),
                .text(recording.audioFilePath),
                .int(recording.durationMilliseconds),
                .int(recording.sampleRate),
                .int(recording.channels),
                .text(recording.modelName.rawValue),
                .text(recording.languageMode.rawValue),
                .int(recording.transcriptSegmentCount),
                .int(recording.failedSegmentCount),
                .text(recording.errorCode?.rawValue ?? ""),
                .text(recording.errorMessage ?? ""),
                .double(recording.startedAt.timeIntervalSince1970),
                recording.endedAt.map { .double($0.timeIntervalSince1970) } ?? .null,
                .double(recording.createdAt.timeIntervalSince1970),
                .double(recording.updatedAt.timeIntervalSince1970)
            ]
        )
    }

    public func insertAudioSegment(_ segment: AudioSegment) throws {
        try execute(
            """
            INSERT OR REPLACE INTO audio_segments (
              id, recording_id, segment_index, file_path, start_ms, end_ms, duration_ms,
              sample_rate, channels, status, error_code, error_message, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
            """,
            [
                .text(segment.id),
                .text(segment.recordingID),
                .int(segment.segmentIndex),
                .text(segment.filePath),
                .int(segment.startMilliseconds),
                .int(segment.endMilliseconds),
                .int(segment.durationMilliseconds),
                .int(segment.sampleRate),
                .int(segment.channels),
                .text(segment.status.rawValue),
                .text(segment.errorCode?.rawValue ?? ""),
                .text(segment.errorMessage ?? ""),
                .double(segment.createdAt.timeIntervalSince1970)
            ]
        )
    }

    public func updateAudioSegmentStatus(id: String, status: AudioSegmentStatus, errorCode: RecordingAgentErrorCode? = nil, errorMessage: String? = nil) throws {
        try execute(
            """
            UPDATE audio_segments
            SET status = ?, error_code = ?, error_message = ?
            WHERE id = ?;
            """,
            [
                .text(status.rawValue),
                .text(errorCode?.rawValue ?? ""),
                .text(errorMessage ?? ""),
                .text(id)
            ]
        )
    }

    public func insertTranscriptSegment(_ segment: TranscriptSegment) throws {
        try execute(
            """
            INSERT OR REPLACE INTO transcript_segments (
              id, recording_id, audio_segment_id, segment_index, start_ms, end_ms, text,
              language, status, provider, model_name, error_code, error_message, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
            """,
            [
                .text(segment.id),
                .text(segment.recordingID),
                .text(segment.audioSegmentID),
                .int(segment.segmentIndex),
                .int(segment.startMilliseconds),
                .int(segment.endMilliseconds),
                .text(segment.text),
                .text(segment.language),
                .text(segment.status.rawValue),
                .text(segment.provider),
                .text(segment.modelName.rawValue),
                .text(segment.errorCode?.rawValue ?? ""),
                .text(segment.errorMessage ?? ""),
                .double(segment.createdAt.timeIntervalSince1970)
            ]
        )
    }

    public func upsertAgentTask(_ task: AgentTask) throws {
        try execute(
            """
            INSERT INTO agent_tasks (
              id, agent_type, recording_id, status, input_json, output_json,
              error_code, error_message, started_at, finished_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              agent_type = excluded.agent_type,
              recording_id = excluded.recording_id,
              status = excluded.status,
              input_json = excluded.input_json,
              output_json = excluded.output_json,
              error_code = excluded.error_code,
              error_message = excluded.error_message,
              started_at = excluded.started_at,
              finished_at = excluded.finished_at,
              created_at = excluded.created_at,
              updated_at = excluded.updated_at;
            """,
            [
                .text(task.id),
                .text(task.agentType),
                .text(task.recordingID),
                .text(task.status.rawValue),
                .text(task.inputJSON),
                .text(task.outputJSON),
                .text(task.errorCode?.rawValue ?? ""),
                .text(task.errorMessage ?? ""),
                .double(task.startedAt.timeIntervalSince1970),
                task.finishedAt.map { .double($0.timeIntervalSince1970) } ?? .null,
                .double(task.createdAt.timeIntervalSince1970),
                .double(task.updatedAt.timeIntervalSince1970)
            ]
        )
    }

    public func insertAgentTaskEvent(_ event: AgentTaskEvent) throws {
        try execute(
            """
            INSERT OR REPLACE INTO agent_task_events (
              id, task_id, event_type, status, progress, message, error_code,
              error_message, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
            """,
            [
                .text(event.id),
                .text(event.taskID),
                .text(event.eventType),
                .text(event.status),
                .int(event.progress),
                .text(event.message),
                .text(event.errorCode?.rawValue ?? ""),
                .text(event.errorMessage ?? ""),
                .text(event.payloadJSON),
                .double(event.createdAt.timeIntervalSince1970)
            ]
        )
    }

    public func insertAgentTaskStep(_ step: AgentTaskStep) throws {
        try execute(
            """
            INSERT OR REPLACE INTO agent_task_steps (
              id, task_id, step_type, step_index, status, ref_type, ref_id, input_json,
              output_json, error_code, error_message, started_at, finished_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
            """,
            [
                .text(step.id),
                .text(step.taskID),
                .text(step.stepType),
                .int(step.stepIndex),
                .text(step.status.rawValue),
                .text(step.refType),
                .text(step.refID),
                .text(step.inputJSON),
                .text(step.outputJSON),
                .text(step.errorCode?.rawValue ?? ""),
                .text(step.errorMessage ?? ""),
                step.startedAt.map { .double($0.timeIntervalSince1970) } ?? .null,
                step.finishedAt.map { .double($0.timeIntervalSince1970) } ?? .null,
                .double(step.createdAt.timeIntervalSince1970),
                .double(step.updatedAt.timeIntervalSince1970)
            ]
        )
    }

    public func insertAgentTaskArtifact(_ artifact: AgentTaskArtifact) throws {
        try execute(
            """
            INSERT OR REPLACE INTO agent_task_artifacts (
              id, task_id, recording_id, artifact_type, uri, ref_type, ref_id, metadata_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);
            """,
            [
                .text(artifact.id),
                .text(artifact.taskID),
                .text(artifact.recordingID),
                .text(artifact.artifactType.rawValue),
                .text(artifact.uri),
                .text(artifact.refType),
                .text(artifact.refID),
                .text(artifact.metadataJSON),
                .double(artifact.createdAt.timeIntervalSince1970)
            ]
        )
    }

    public func updateRecordingCounts(recordingID: String) throws {
        let successCount = try scalarInt(
            "SELECT COUNT(*) FROM transcript_segments WHERE recording_id = ? AND status = ?;",
            [.text(recordingID), .text(TranscriptSegmentStatus.success.rawValue)]
        )
        let failedCount = try scalarInt(
            "SELECT COUNT(*) FROM transcript_segments WHERE recording_id = ? AND status = ?;",
            [.text(recordingID), .text(TranscriptSegmentStatus.failed.rawValue)]
        )
        try execute(
            """
            UPDATE recordings
            SET transcript_segment_count = ?,
              failed_segment_count = ?,
              updated_at = ?
            WHERE id = ?;
            """,
            [.int(successCount), .int(failedCount), .double(Date().timeIntervalSince1970), .text(recordingID)]
        )
    }

    public func fetchRecordings() throws -> [Recording] {
        try query("SELECT * FROM recordings ORDER BY created_at DESC;") { statement in
            readRecording(statement)
        }
    }

    public func fetchRecordingDetail(id: String) throws -> RecordingDetail? {
        let recordings = try query("SELECT * FROM recordings WHERE id = ? LIMIT 1;", [.text(id)]) { statement in
            readRecording(statement)
        }
        guard let recording = recordings.first else {
            return nil
        }

        let audioSegments = try query("SELECT * FROM audio_segments WHERE recording_id = ? ORDER BY segment_index;", [.text(id)]) { statement in
            readAudioSegment(statement)
        }
        let segments = try query("SELECT * FROM transcript_segments WHERE recording_id = ? ORDER BY segment_index;", [.text(id)]) { statement in
            readTranscriptSegment(statement)
        }
        let events = try query(
            """
            SELECT e.* FROM agent_task_events e
            INNER JOIN agent_tasks t ON t.id = e.task_id
            WHERE t.recording_id = ?
            ORDER BY e.created_at;
            """,
            [.text(id)]
        ) { statement in
            readAgentTaskEvent(statement)
        }
        let steps = try query(
            """
            SELECT s.* FROM agent_task_steps s
            INNER JOIN agent_tasks t ON t.id = s.task_id
            WHERE t.recording_id = ?
            ORDER BY s.step_index, s.created_at;
            """,
            [.text(id)]
        ) { statement in
            readAgentTaskStep(statement)
        }
        let artifacts = try query("SELECT * FROM agent_task_artifacts WHERE recording_id = ? ORDER BY created_at;", [.text(id)]) { statement in
            readAgentTaskArtifact(statement)
        }

        return RecordingDetail(recording: recording, audioSegments: audioSegments, segments: segments, events: events, steps: steps, artifacts: artifacts)
    }

    private func execute(_ sql: String, _ values: [SQLiteValue] = []) throws {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK, let statement else {
            throw SQLiteRecordingStoreError.prepareFailed(errorMessage)
        }
        defer { sqlite3_finalize(statement) }
        try bind(values, to: statement)
        guard sqlite3_step(statement) == SQLITE_DONE else {
            throw SQLiteRecordingStoreError.stepFailed(errorMessage)
        }
    }

    private func executeScript(_ sql: String) throws {
        var error: UnsafeMutablePointer<CChar>?
        let result = sqlite3_exec(db, sql, nil, nil, &error)
        if result != SQLITE_OK {
            let message = error.map { String(cString: $0) } ?? errorMessage
            sqlite3_free(error)
            throw SQLiteRecordingStoreError.stepFailed(message)
        }
    }

    private func query<T>(_ sql: String, _ values: [SQLiteValue] = [], map: (OpaquePointer) throws -> T) throws -> [T] {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK, let statement else {
            throw SQLiteRecordingStoreError.prepareFailed(errorMessage)
        }
        defer { sqlite3_finalize(statement) }
        try bind(values, to: statement)

        var rows: [T] = []
        while true {
            let result = sqlite3_step(statement)
            if result == SQLITE_ROW {
                rows.append(try map(statement))
            } else if result == SQLITE_DONE {
                return rows
            } else {
                throw SQLiteRecordingStoreError.stepFailed(errorMessage)
            }
        }
    }

    private func scalarInt(_ sql: String, _ values: [SQLiteValue]) throws -> Int {
        try query(sql, values) { statement in
            int(statement, 0)
        }.first ?? 0
    }

    private func bind(_ values: [SQLiteValue], to statement: OpaquePointer) throws {
        for (index, value) in values.enumerated() {
            let position = Int32(index + 1)
            let result: Int32
            switch value {
            case let .text(text):
                result = sqlite3_bind_text(statement, position, text, -1, SQLITE_TRANSIENT)
            case let .int(int):
                result = sqlite3_bind_int64(statement, position, sqlite3_int64(int))
            case let .double(double):
                result = sqlite3_bind_double(statement, position, double)
            case .null:
                result = sqlite3_bind_null(statement, position)
            }
            guard result == SQLITE_OK else {
                throw SQLiteRecordingStoreError.stepFailed(errorMessage)
            }
        }
    }

    private var errorMessage: String {
        String(cString: sqlite3_errmsg(db))
    }
}

private let SQLITE_TRANSIENT = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

private func text(_ statement: OpaquePointer, _ index: Int32) -> String {
    guard let raw = sqlite3_column_text(statement, index) else {
        return ""
    }
    return String(cString: raw)
}

private func int(_ statement: OpaquePointer, _ index: Int32) -> Int {
    Int(sqlite3_column_int64(statement, index))
}

private func date(_ statement: OpaquePointer, _ index: Int32) -> Date {
    Date(timeIntervalSince1970: sqlite3_column_double(statement, index))
}

private func optionalDate(_ statement: OpaquePointer, _ index: Int32) -> Date? {
    if sqlite3_column_type(statement, index) == SQLITE_NULL {
        return nil
    }
    return date(statement, index)
}

private func optionalErrorCode(_ raw: String) -> RecordingAgentErrorCode? {
    raw.isEmpty ? nil : RecordingAgentErrorCode(rawValue: raw)
}

private func readRecording(_ statement: OpaquePointer) -> Recording {
    Recording(
        id: text(statement, 0),
        title: text(statement, 1),
        status: RecordingStatus(rawValue: text(statement, 2)) ?? .failed,
        audioFilePath: text(statement, 3),
        durationMilliseconds: int(statement, 4),
        sampleRate: int(statement, 5),
        channels: int(statement, 6),
        modelName: ASRModelName(rawValue: text(statement, 7)) ?? .small,
        languageMode: LanguageMode(rawValue: text(statement, 8)) ?? .auto,
        transcriptSegmentCount: int(statement, 9),
        failedSegmentCount: int(statement, 10),
        errorCode: optionalErrorCode(text(statement, 11)),
        errorMessage: text(statement, 12).nilIfEmpty,
        startedAt: date(statement, 13),
        endedAt: optionalDate(statement, 14),
        createdAt: date(statement, 15),
        updatedAt: date(statement, 16)
    )
}

private func readAudioSegment(_ statement: OpaquePointer) -> AudioSegment {
    AudioSegment(
        id: text(statement, 0),
        recordingID: text(statement, 1),
        segmentIndex: int(statement, 2),
        filePath: text(statement, 3),
        startMilliseconds: int(statement, 4),
        endMilliseconds: int(statement, 5),
        durationMilliseconds: int(statement, 6),
        sampleRate: int(statement, 7),
        channels: int(statement, 8),
        status: AudioSegmentStatus(rawValue: text(statement, 9)) ?? .failed,
        errorCode: optionalErrorCode(text(statement, 10)),
        errorMessage: text(statement, 11).nilIfEmpty,
        createdAt: date(statement, 12)
    )
}

private func readTranscriptSegment(_ statement: OpaquePointer) -> TranscriptSegment {
    TranscriptSegment(
        id: text(statement, 0),
        recordingID: text(statement, 1),
        audioSegmentID: text(statement, 2),
        segmentIndex: int(statement, 3),
        startMilliseconds: int(statement, 4),
        endMilliseconds: int(statement, 5),
        text: text(statement, 6),
        language: text(statement, 7),
        status: TranscriptSegmentStatus(rawValue: text(statement, 8)) ?? .failed,
        provider: text(statement, 9),
        modelName: ASRModelName(rawValue: text(statement, 10)) ?? .small,
        errorCode: optionalErrorCode(text(statement, 11)),
        errorMessage: text(statement, 12).nilIfEmpty,
        createdAt: date(statement, 13)
    )
}

private func readAgentTaskEvent(_ statement: OpaquePointer) -> AgentTaskEvent {
    AgentTaskEvent(
        id: text(statement, 0),
        taskID: text(statement, 1),
        eventType: text(statement, 2),
        status: text(statement, 3),
        progress: int(statement, 4),
        message: text(statement, 5),
        errorCode: optionalErrorCode(text(statement, 6)),
        errorMessage: text(statement, 7).nilIfEmpty,
        payloadJSON: text(statement, 8),
        createdAt: date(statement, 9)
    )
}

private func readAgentTaskStep(_ statement: OpaquePointer) -> AgentTaskStep {
    AgentTaskStep(
        id: text(statement, 0),
        taskID: text(statement, 1),
        stepType: text(statement, 2),
        stepIndex: int(statement, 3),
        status: AgentTaskStepStatus(rawValue: text(statement, 4)) ?? .failed,
        refType: text(statement, 5),
        refID: text(statement, 6),
        inputJSON: text(statement, 7),
        outputJSON: text(statement, 8),
        errorCode: optionalErrorCode(text(statement, 9)),
        errorMessage: text(statement, 10).nilIfEmpty,
        startedAt: optionalDate(statement, 11),
        finishedAt: optionalDate(statement, 12),
        createdAt: date(statement, 13),
        updatedAt: date(statement, 14)
    )
}

private func readAgentTaskArtifact(_ statement: OpaquePointer) -> AgentTaskArtifact {
    AgentTaskArtifact(
        id: text(statement, 0),
        taskID: text(statement, 1),
        recordingID: text(statement, 2),
        artifactType: AgentTaskArtifactType(rawValue: text(statement, 3)) ?? .transcriptRaw,
        uri: text(statement, 4),
        refType: text(statement, 5),
        refID: text(statement, 6),
        metadataJSON: text(statement, 7),
        createdAt: date(statement, 8)
    )
}

private extension String {
    var nilIfEmpty: String? {
        isEmpty ? nil : self
    }
}
