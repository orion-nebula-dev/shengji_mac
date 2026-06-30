import AVFoundation
import Foundation

protocol AudioFileWriting: AnyObject {
    func write(from buffer: AVAudioPCMBuffer) throws
    func close()
}

protocol AudioFileWriterFactory {
    func makeWriter(url: URL, settings: [String: Any]) throws -> AudioFileWriting
}

struct AVAudioFileWriterFactory: AudioFileWriterFactory {
    func makeWriter(url: URL, settings: [String: Any]) throws -> AudioFileWriting {
        try AVAudioFileWriter(url: url, settings: settings)
    }
}

private final class AVAudioFileWriter: AudioFileWriting {
    private var file: AVAudioFile?

    init(url: URL, settings: [String: Any]) throws {
        self.file = try AVAudioFile(forWriting: url, settings: settings)
    }

    func write(from buffer: AVAudioPCMBuffer) throws {
        guard let file else {
            throw RecordingAgentError(code: .writeFailed, message: "音频文件已关闭")
        }
        try file.write(from: buffer)
    }

    func close() {
        file = nil
    }
}

final class AudioSliceFileCoordinator {
    private let configuration: AudioCaptureConfiguration
    private let targetFormat: AVAudioFormat
    private let fileWriterFactory: AudioFileWriterFactory
    private let publish: (AudioSlice) -> Void
    private var sliceFile: AudioFileWriting?
    private var currentSliceIndex = 0
    private var currentSliceFrames: AVAudioFramePosition = 0
    private var currentSliceURL: URL?
    private(set) var totalFrames: AVAudioFramePosition = 0

    init(
        configuration: AudioCaptureConfiguration,
        targetFormat: AVAudioFormat,
        fileWriterFactory: AudioFileWriterFactory,
        publish: @escaping (AudioSlice) -> Void
    ) {
        self.configuration = configuration
        self.targetFormat = targetFormat
        self.fileWriterFactory = fileWriterFactory
        self.publish = publish
    }

    func start() throws {
        currentSliceIndex = 0
        currentSliceFrames = 0
        totalFrames = 0
        try openNextSliceFile()
    }

    func write(buffer: AVAudioPCMBuffer) throws {
        let framesPerSlice = AVAudioFramePosition(configuration.sampleRate * configuration.sliceDurationMilliseconds / 1000)
        var offset: AVAudioFramePosition = 0

        while offset < AVAudioFramePosition(buffer.frameLength) {
            let remainingInBuffer = AVAudioFramePosition(buffer.frameLength) - offset
            let remainingInSlice = framesPerSlice - currentSliceFrames
            let framesToCopy = min(remainingInBuffer, remainingInSlice)
            guard let chunk = copy(buffer: buffer, offset: AVAudioFrameCount(offset), frameCount: AVAudioFrameCount(framesToCopy)) else {
                throw RecordingAgentError(code: .writeFailed, message: "无法切分音频缓冲区")
            }
            try sliceFile?.write(from: chunk)

            currentSliceFrames += framesToCopy
            totalFrames += framesToCopy
            offset += framesToCopy

            if currentSliceFrames >= framesPerSlice {
                publishCurrentSlice(endFrame: totalFrames)
                currentSliceIndex += 1
                currentSliceFrames = 0
                try openNextSliceFile()
            }
        }
    }

    func finishPartialSlice() {
        guard currentSliceFrames > 0 else {
            return
        }
        publishCurrentSlice(endFrame: totalFrames)
        currentSliceIndex += 1
        currentSliceFrames = 0
    }

    func close() {
        sliceFile?.close()
        sliceFile = nil
        currentSliceURL = nil
    }

    private func openNextSliceFile() throws {
        let url = configuration.outputDirectory
            .appending(path: "slices")
            .appending(path: "\(String(format: "%06d", currentSliceIndex)).wav")
        currentSliceURL = url
        sliceFile = try fileWriterFactory.makeWriter(url: url, settings: targetFormat.settings)
    }

    private func publishCurrentSlice(endFrame: AVAudioFramePosition) {
        guard let currentSliceURL else {
            return
        }
        let sliceFrames = currentSliceFrames
        let sliceIndex = currentSliceIndex
        sliceFile?.close()
        sliceFile = nil
        self.currentSliceURL = nil

        let startFrame = endFrame - sliceFrames
        let startMilliseconds = Int((Double(startFrame) / Double(configuration.sampleRate)) * 1000)
        let endMilliseconds = Int((Double(endFrame) / Double(configuration.sampleRate)) * 1000)
        publish(AudioSlice(
            id: UUID().uuidString,
            recordingID: configuration.recordingID,
            index: sliceIndex,
            fileURL: currentSliceURL,
            startMilliseconds: startMilliseconds,
            endMilliseconds: endMilliseconds,
            sampleRate: configuration.sampleRate,
            channels: configuration.channels
        ))
    }

    private func copy(buffer: AVAudioPCMBuffer, offset: AVAudioFrameCount, frameCount: AVAudioFrameCount) -> AVAudioPCMBuffer? {
        guard let output = AVAudioPCMBuffer(pcmFormat: buffer.format, frameCapacity: frameCount) else {
            return nil
        }
        output.frameLength = frameCount

        let channelCount = Int(buffer.format.channelCount)
        for channel in 0..<channelCount {
            guard let source = buffer.floatChannelData?[channel],
                  let target = output.floatChannelData?[channel] else {
                return nil
            }
            target.update(from: source.advanced(by: Int(offset)), count: Int(frameCount))
        }
        return output
    }
}

public final class NativeMicrophonePermissionService: MicrophonePermissionService, @unchecked Sendable {
    public init() {}

    public func requestMicrophonePermission() async -> MicrophonePermissionStatus {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return .authorized
        case .denied, .restricted:
            return .denied
        case .notDetermined:
            let granted = await AVCaptureDevice.requestAccess(for: .audio)
            return granted ? .authorized : .denied
        @unknown default:
            return .denied
        }
    }
}

public final class NativeAudioCaptureService: AudioCaptureService, @unchecked Sendable {
    private let engine = AVAudioEngine()
    private let fileWriterFactory: AudioFileWriterFactory
    private var converter: AVAudioConverter?
    private var targetFormat: AVAudioFormat?
    private var fullFile: AudioFileWriting?
    private var sliceCoordinator: AudioSliceFileCoordinator?
    private var continuation: AsyncThrowingStream<AudioSlice, Error>.Continuation?
    private var configuration: AudioCaptureConfiguration?
    private var latestInputLevel = AudioInputLevel.silent
    private let queue = DispatchQueue(label: "recording-agent.audio-capture")

    public convenience init() {
        self.init(fileWriterFactory: AVAudioFileWriterFactory())
    }

    init(fileWriterFactory: AudioFileWriterFactory) {
        self.fileWriterFactory = fileWriterFactory
    }

    public func start(configuration: AudioCaptureConfiguration) async throws -> AsyncThrowingStream<AudioSlice, Error> {
        resetCaptureSession(finishContinuation: true)
        try FileManager.default.createDirectory(at: configuration.outputDirectory.appending(path: "slices"), withIntermediateDirectories: true)
        self.configuration = configuration

        let input = engine.inputNode
        let inputFormat = input.outputFormat(forBus: 0)
        guard let targetFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: Double(configuration.sampleRate), channels: AVAudioChannelCount(configuration.channels), interleaved: false) else {
            throw RecordingAgentError(code: .writeFailed, message: "无法创建 16kHz mono 音频格式")
        }
        self.targetFormat = targetFormat
        self.converter = AVAudioConverter(from: inputFormat, to: targetFormat)

        let stream = AsyncThrowingStream<AudioSlice, Error> { continuation in
            self.continuation = continuation
        }
        self.fullFile = try fileWriterFactory.makeWriter(url: configuration.outputDirectory.appending(path: "full.wav"), settings: targetFormat.settings)
        let sliceCoordinator = AudioSliceFileCoordinator(
            configuration: configuration,
            targetFormat: targetFormat,
            fileWriterFactory: fileWriterFactory
        ) { [weak self] slice in
            self?.continuation?.yield(slice)
        }
        try sliceCoordinator.start()
        self.sliceCoordinator = sliceCoordinator

        do {
            input.installTap(onBus: 0, bufferSize: 4096, format: inputFormat) { [weak self] buffer, _ in
                self?.queue.async {
                    do {
                        try self?.handle(buffer: buffer)
                    } catch {
                        self?.continuation?.finish(throwing: error)
                    }
                }
            }

            engine.prepare()
            try engine.start()
            return stream
        } catch {
            resetCaptureSession(finishContinuation: true)
            throw error
        }
    }

    public func stop() async throws -> AudioCaptureResult {
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        queue.sync {}

        let configuration = try requireConfiguration()
        let durationMilliseconds = Int((Double(sliceCoordinator?.totalFrames ?? 0) / Double(configuration.sampleRate)) * 1000)
        sliceCoordinator?.finishPartialSlice()
        continuation?.finish()
        continuation = nil
        sliceCoordinator?.close()
        sliceCoordinator = nil
        fullFile?.close()
        fullFile = nil
        latestInputLevel = .silent

        return AudioCaptureResult(
            recordingID: configuration.recordingID,
            fullAudioURL: configuration.outputDirectory.appending(path: "full.wav"),
            durationMilliseconds: durationMilliseconds,
            sampleRate: configuration.sampleRate,
            channels: configuration.channels
        )
    }

    public func currentInputLevel() async -> AudioInputLevel {
        queue.sync {
            latestInputLevel
        }
    }

    private func resetCaptureSession(finishContinuation: Bool) {
        engine.inputNode.removeTap(onBus: 0)
        if engine.isRunning {
            engine.stop()
        }
        queue.sync {}
        if finishContinuation {
            continuation?.finish()
        }
        continuation = nil
        converter = nil
        targetFormat = nil
        sliceCoordinator?.close()
        sliceCoordinator = nil
        fullFile?.close()
        fullFile = nil
        configuration = nil
        latestInputLevel = .silent
    }

    private func handle(buffer: AVAudioPCMBuffer) throws {
        guard let converter, let targetFormat, let fullFile, let sliceCoordinator else {
            throw RecordingAgentError(code: .writeFailed, message: "音频采集尚未初始化")
        }

        let ratio = targetFormat.sampleRate / buffer.format.sampleRate
        let targetCapacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 16
        guard let converted = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: max(targetCapacity, 1)) else {
            throw RecordingAgentError(code: .writeFailed, message: "无法创建转换缓冲区")
        }

        var consumed = false
        let status = converter.convert(to: converted, error: nil) { _, outStatus in
            if consumed {
                outStatus.pointee = .noDataNow
                return nil
            }
            consumed = true
            outStatus.pointee = .haveData
            return buffer
        }
        guard status != .error else {
            throw RecordingAgentError(code: .writeFailed, message: "音频格式转换失败")
        }
        guard converted.frameLength > 0 else {
            return
        }

        latestInputLevel = inputLevel(from: converted)
        try fullFile.write(from: converted)
        try sliceCoordinator.write(buffer: converted)
    }

    private func inputLevel(from buffer: AVAudioPCMBuffer) -> AudioInputLevel {
        guard let channels = buffer.floatChannelData else {
            return .silent
        }
        let frameCount = Int(buffer.frameLength)
        let channelCount = Int(buffer.format.channelCount)
        guard frameCount > 0, channelCount > 0 else {
            return .silent
        }

        var sumSquares = 0.0
        var sampleCount = 0
        for channel in 0..<channelCount {
            let samples = channels[channel]
            for frame in 0..<frameCount {
                let sample = Double(samples[frame])
                sumSquares += sample * sample
                sampleCount += 1
            }
        }
        guard sampleCount > 0 else {
            return .silent
        }
        return AudioInputLevel(linear: sqrt(sumSquares / Double(sampleCount)))
    }

    private func requireConfiguration() throws -> AudioCaptureConfiguration {
        guard let configuration else {
            throw RecordingAgentError(code: .writeFailed, message: "音频采集配置缺失")
        }
        return configuration
    }
}

public struct WAVFileInfo: Equatable, Sendable {
    public let sampleRate: Double
    public let channels: Int
    public let durationMilliseconds: Int
}

public enum WAVFileInspector {
    public static func inspect(url: URL) throws -> WAVFileInfo {
        let file = try AVAudioFile(forReading: url)
        let duration = Int((Double(file.length) / file.fileFormat.sampleRate) * 1000)
        return WAVFileInfo(
            sampleRate: file.fileFormat.sampleRate,
            channels: Int(file.fileFormat.channelCount),
            durationMilliseconds: duration
        )
    }
}
