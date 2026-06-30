import Darwin
import Foundation
import RecordingAgentCore
import WhisperKit

@main
struct RecordingAgentSmoke {
    static func main() async {
        do {
            try await run()
            print("SMOKE PASS: RecordingAgent smoke completed")
        } catch {
            fputs("SMOKE FAIL: \(error)\n", stderr)
            exit(1)
        }
    }

    private static func run() async throws {
        let supportRoot = try applicationSupportRoot()
        let modelBaseDirectory = supportRoot.appending(path: "models/whisperkit")
        let modelManager = FileSystemModelManager(baseDirectory: modelBaseDirectory)
        let asset = try await modelManager.verify(model: .small)
        print("model: \(asset.modelName.rawValue) ready at \(asset.localPath)")

        if let audioPath = ProcessInfo.processInfo.environment["RECORDING_AGENT_SMOKE_AUDIO_PATH"],
           !audioPath.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            let audioURL = URL(fileURLWithPath: audioPath)
            _ = try WAVFileInspector.inspect(url: audioURL)
            try await transcribe(audioURL: audioURL, modelBaseDirectory: modelBaseDirectory, asset: asset)
            return
        }

        let permission = await NativeMicrophonePermissionService().requestMicrophonePermission()
        guard permission == .authorized else {
            throw RecordingAgentError(code: .permissionDenied, message: "麦克风未授权：\(permission)")
        }
        print("microphone: authorized")

        let outputDirectory = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentSmoke-\(UUID().uuidString)")
        defer {
            try? FileManager.default.removeItem(at: outputDirectory)
        }

        let audio = NativeAudioCaptureService()
        let stream = try await audio.start(configuration: AudioCaptureConfiguration(
            recordingID: "smoke-\(UUID().uuidString)",
            outputDirectory: outputDirectory,
            sampleRate: 16_000,
            channels: 1,
            sliceDurationMilliseconds: 3_000
        ))
        let sliceCounter = Task<Int, Error> {
            var count = 0
            for try await _ in stream {
                count += 1
            }
            return count
        }

        try await Task.sleep(nanoseconds: 3_000_000_000)
        let capture = try await audio.stop()
        let sliceCount = try await sliceCounter.value
        let wavInfo = try WAVFileInspector.inspect(url: capture.fullAudioURL)
        guard wavInfo.durationMilliseconds > 0 else {
            throw RecordingAgentError(code: .writeFailed, message: "录音文件时长为 0")
        }
        print("audio: \(capture.fullAudioURL.path)")
        print("audio: \(wavInfo.durationMilliseconds)ms, \(Int(wavInfo.sampleRate))Hz, \(wavInfo.channels)ch, slices \(sliceCount)")

        try await transcribe(audioURL: capture.fullAudioURL, modelBaseDirectory: modelBaseDirectory, asset: asset)
    }

    private static func transcribe(audioURL: URL, modelBaseDirectory: URL, asset: ASRModelAsset) async throws {
        let modelDirectory = URL(fileURLWithPath: asset.localPath)
        let config = WhisperKitConfig(
            model: ASRModelName.small.whisperKitVariantName,
            downloadBase: modelBaseDirectory,
            modelRepo: "argmaxinc/whisperkit-coreml",
            modelFolder: modelDirectory.path,
            tokenizerFolder: modelBaseDirectory,
            verbose: false,
            prewarm: true,
            load: true,
            download: false
        )
        let whisperKit = try await WhisperKit(config)
        let options = DecodingOptions(
            task: .transcribe,
            language: nil,
            temperatureFallbackCount: 1,
            usePrefillPrompt: true,
            detectLanguage: true,
            skipSpecialTokens: true,
            wordTimestamps: false,
            concurrentWorkerCount: 1
        )
        let results = try await whisperKit.transcribe(audioPath: audioURL.path, decodeOptions: options)
        let text = results.map(\.text).joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines)
        print("transcript-language: \(results.first?.language ?? "unknown")")
        print("transcript-text: \(text.isEmpty ? "<empty>" : text)")
    }

    private static func applicationSupportRoot() throws -> URL {
        if let override = ProcessInfo.processInfo.environment["RECORDING_AGENT_APP_SUPPORT_ROOT"],
           !override.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return URL(fileURLWithPath: override, isDirectory: true)
        }
        let base = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        return base.appending(path: "com.shengji.recording-agent")
    }
}
