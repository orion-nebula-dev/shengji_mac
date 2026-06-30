import Foundation
import RecordingAgentCore
import WhisperKit

actor WhisperKitASREngine: LocalASREngine {
    private let modelBaseDirectory: URL
    private let fileSystemModelManager: FileSystemModelManager
    private var pipelines: [ASRModelName: WhisperKit] = [:]

    init(modelBaseDirectory: URL) {
        self.modelBaseDirectory = modelBaseDirectory
        self.fileSystemModelManager = FileSystemModelManager(baseDirectory: modelBaseDirectory)
    }

    func checkReady(model: ASRModelName) async -> Result<Void, RecordingAgentError> {
        do {
            _ = try await pipeline(for: model, allowNetwork: false)
            return .success(())
        } catch {
            return .failure(RecordingAgentError(code: .asrEngineUnavailable, message: "本地 WhisperKit 引擎不可用：\(error.localizedDescription)"))
        }
    }

    func transcribe(slice: AudioSlice, model: ASRModelName, language: LanguageMode) async -> Result<ASRSegmentResult, RecordingAgentError> {
        do {
            let pipe = try await pipeline(for: model, allowNetwork: false)
            let policy = LocalASRDecodingPolicy.whisperKitPolicy(for: language)
            let options = DecodingOptions(
                task: .transcribe,
                language: policy.languageCode,
                temperatureFallbackCount: policy.temperatureFallbackCount,
                usePrefillPrompt: true,
                detectLanguage: policy.shouldDetectLanguage,
                skipSpecialTokens: true,
                wordTimestamps: false,
                concurrentWorkerCount: 1
            )
            let results = try await pipe.transcribe(audioPath: slice.fileURL.path, decodeOptions: options)
            let text = results.map(\.text).joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines)
            let detectedLanguage = results.first?.language ?? language.rawValue
            switch ASRTranscriptPostProcessor.process(text: text) {
            case let .success(processedText):
                return .success(ASRSegmentResult(text: processedText, language: detectedLanguage, provider: "whisperkit", modelName: model))
            case let .failure(error):
                return .failure(error)
            }
        } catch {
            return .failure(RecordingAgentError(code: .transcribeFailed, message: "WhisperKit 转写失败：\(error.localizedDescription)"))
        }
    }

    private func pipeline(for model: ASRModelName, allowNetwork: Bool) async throws -> WhisperKit {
        if let existing = pipelines[model] {
            return existing
        }

        let modelAsset = try await fileSystemModelManager.verify(model: model)
        let modelDirectory = URL(fileURLWithPath: modelAsset.localPath)
        let config = WhisperKitConfig(
            model: model.whisperKitVariantName,
            downloadBase: modelBaseDirectory,
            modelRepo: "argmaxinc/whisperkit-coreml",
            modelFolder: modelDirectory.path,
            tokenizerFolder: modelBaseDirectory,
            verbose: false,
            prewarm: true,
            load: true,
            download: allowNetwork
        )
        let pipe = try await WhisperKit(config)
        pipelines[model] = pipe
        return pipe
    }
}
