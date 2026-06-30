import Foundation

public struct AudioSliceWindow: Equatable, Sendable {
    public let index: Int
    public let startMilliseconds: Int
    public let endMilliseconds: Int

    public var durationMilliseconds: Int {
        endMilliseconds - startMilliseconds
    }
}

public enum AudioSlicePlanner {
    public static let defaultWindowMilliseconds = 15_000

    public static func planWindows(durationMilliseconds: Int, windowMilliseconds: Int = defaultWindowMilliseconds) -> [AudioSliceWindow] {
        guard durationMilliseconds > 0, windowMilliseconds > 0 else {
            return []
        }

        var windows: [AudioSliceWindow] = []
        var start = 0
        var index = 0

        while start < durationMilliseconds {
            let end = min(start + windowMilliseconds, durationMilliseconds)
            windows.append(AudioSliceWindow(index: index, startMilliseconds: start, endMilliseconds: end))
            start = end
            index += 1
        }

        return windows
    }
}

public enum MicrophonePermissionStatus: Equatable, Sendable {
    case authorized
    case denied
    case notDetermined
}

public struct AudioInputLevel: Equatable, Sendable {
    public let linear: Double

    public init(linear: Double) {
        guard linear.isFinite else {
            self.linear = 0
            return
        }
        self.linear = min(max(linear, 0), 1)
    }

    public static let silent = AudioInputLevel(linear: 0)

    public var percent: Int {
        Int((linear * 100).rounded())
    }

    public var displayText: String {
        "\(percent)%"
    }
}

public struct RecordingRuntimeSnapshot: Equatable, Sendable {
    public let recordingID: String?
    public let startedAt: Date?
    public let elapsedMilliseconds: Int
    public let inputLevel: AudioInputLevel

    public init(recordingID: String?, startedAt: Date?, elapsedMilliseconds: Int, inputLevel: AudioInputLevel) {
        self.recordingID = recordingID
        self.startedAt = startedAt
        self.elapsedMilliseconds = max(elapsedMilliseconds, 0)
        self.inputLevel = inputLevel
    }

    public static let inactive = RecordingRuntimeSnapshot(
        recordingID: nil,
        startedAt: nil,
        elapsedMilliseconds: 0,
        inputLevel: .silent
    )

    public var elapsedText: String {
        let seconds = elapsedMilliseconds / 1000
        return "\(seconds / 60):\(String(format: "%02d", seconds % 60))"
    }
}

public struct AudioCaptureConfiguration: Equatable, Sendable {
    public let recordingID: String
    public let outputDirectory: URL
    public let sampleRate: Int
    public let channels: Int
    public let sliceDurationMilliseconds: Int

    public init(recordingID: String, outputDirectory: URL, sampleRate: Int = 16_000, channels: Int = 1, sliceDurationMilliseconds: Int = AudioSlicePlanner.defaultWindowMilliseconds) {
        self.recordingID = recordingID
        self.outputDirectory = outputDirectory
        self.sampleRate = sampleRate
        self.channels = channels
        self.sliceDurationMilliseconds = sliceDurationMilliseconds
    }
}

public struct AudioSlice: Equatable, Sendable {
    public let id: String
    public let recordingID: String
    public let index: Int
    public let fileURL: URL
    public let startMilliseconds: Int
    public let endMilliseconds: Int
    public let sampleRate: Int
    public let channels: Int

    public var durationMilliseconds: Int {
        endMilliseconds - startMilliseconds
    }

    public init(id: String, recordingID: String, index: Int, fileURL: URL, startMilliseconds: Int, endMilliseconds: Int, sampleRate: Int = 16_000, channels: Int = 1) {
        self.id = id
        self.recordingID = recordingID
        self.index = index
        self.fileURL = fileURL
        self.startMilliseconds = startMilliseconds
        self.endMilliseconds = endMilliseconds
        self.sampleRate = sampleRate
        self.channels = channels
    }

    public static func mock(index: Int, start: Int, end: Int, recordingID: String = "mock-recording") -> AudioSlice {
        AudioSlice(
            id: "slice-\(index)",
            recordingID: recordingID,
            index: index,
            fileURL: URL(fileURLWithPath: "/tmp/recording-agent/slices/\(String(format: "%06d", index)).wav"),
            startMilliseconds: start,
            endMilliseconds: end
        )
    }
}

public struct AudioCaptureResult: Equatable, Sendable {
    public let recordingID: String
    public let fullAudioURL: URL
    public let durationMilliseconds: Int
    public let sampleRate: Int
    public let channels: Int

    public init(recordingID: String, fullAudioURL: URL, durationMilliseconds: Int, sampleRate: Int = 16_000, channels: Int = 1) {
        self.recordingID = recordingID
        self.fullAudioURL = fullAudioURL
        self.durationMilliseconds = durationMilliseconds
        self.sampleRate = sampleRate
        self.channels = channels
    }
}

public protocol MicrophonePermissionService: Sendable {
    func requestMicrophonePermission() async -> MicrophonePermissionStatus
}

public protocol AudioCaptureService: Sendable {
    func start(configuration: AudioCaptureConfiguration) async throws -> AsyncThrowingStream<AudioSlice, Error>
    func stop() async throws -> AudioCaptureResult
    func currentInputLevel() async -> AudioInputLevel
}

public extension AudioCaptureService {
    func currentInputLevel() async -> AudioInputLevel {
        .silent
    }
}

public final class MockAudioCaptureService: MicrophonePermissionService, AudioCaptureService, @unchecked Sendable {
    private let permission: MicrophonePermissionStatus
    private let initialSlices: [AudioSlice]
    private let startError: Error?
    private let stopError: Error?
    private let inputLevel: AudioInputLevel
    private var activeConfiguration: AudioCaptureConfiguration?

    public init(permission: MicrophonePermissionStatus, slices: [AudioSlice], startError: Error? = nil, stopError: Error? = nil, inputLevel: AudioInputLevel = .silent) {
        self.permission = permission
        self.initialSlices = slices
        self.startError = startError
        self.stopError = stopError
        self.inputLevel = inputLevel
    }

    public func requestMicrophonePermission() async -> MicrophonePermissionStatus {
        permission
    }

    public func start(configuration: AudioCaptureConfiguration) async throws -> AsyncThrowingStream<AudioSlice, Error> {
        if let startError {
            throw startError
        }
        activeConfiguration = configuration
        let recordingID = configuration.recordingID
        let outputDirectory = configuration.outputDirectory

        return AsyncThrowingStream { continuation in
            for slice in initialSlices {
                continuation.yield(AudioSlice(
                    id: slice.id,
                    recordingID: recordingID,
                    index: slice.index,
                    fileURL: outputDirectory.appending(path: "slices/\(String(format: "%06d", slice.index)).wav"),
                    startMilliseconds: slice.startMilliseconds,
                    endMilliseconds: slice.endMilliseconds,
                    sampleRate: configuration.sampleRate,
                    channels: configuration.channels
                ))
            }
            continuation.finish()
        }
    }

    public func stop() async throws -> AudioCaptureResult {
        if let stopError {
            throw stopError
        }
        let configuration = activeConfiguration ?? AudioCaptureConfiguration(recordingID: "unknown", outputDirectory: URL(fileURLWithPath: "/tmp/recording-agent"))
        let duration = initialSlices.map(\.endMilliseconds).max() ?? 0
        return AudioCaptureResult(
            recordingID: configuration.recordingID,
            fullAudioURL: configuration.outputDirectory.appending(path: "full.wav"),
            durationMilliseconds: duration,
            sampleRate: configuration.sampleRate,
            channels: configuration.channels
        )
    }

    public func currentInputLevel() async -> AudioInputLevel {
        inputLevel
    }
}
