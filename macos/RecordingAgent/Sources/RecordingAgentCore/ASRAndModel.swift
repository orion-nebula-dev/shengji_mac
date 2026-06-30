import Foundation

public struct ASRSegmentResult: Equatable, Sendable {
    public let text: String
    public let language: String
    public let provider: String
    public let modelName: ASRModelName

    public init(text: String, language: String, provider: String = "whisperkit", modelName: ASRModelName) {
        self.text = text
        self.language = language
        self.provider = provider
        self.modelName = modelName
    }
}

public protocol LocalASREngine: Sendable {
    func checkReady(model: ASRModelName) async -> Result<Void, RecordingAgentError>
    func transcribe(slice: AudioSlice, model: ASRModelName, language: LanguageMode) async -> Result<ASRSegmentResult, RecordingAgentError>
}

public protocol ModelManaging: Sendable {
    func listModels() async throws -> [ASRModelAsset]
    func prepare(model: ASRModelName, trigger: ModelPrepareTrigger) async throws -> ASRModelAsset
    func verify(model: ASRModelName) async throws -> ASRModelAsset
}

public final class MockModelManager: ModelManaging, @unchecked Sendable {
    private let readyModels: Set<ASRModelName>

    public init(readyModels: Set<ASRModelName>) {
        self.readyModels = readyModels
    }

    public func listModels() async throws -> [ASRModelAsset] {
        let now = Date()
        return ASRModelName.allCases.map { model in
            ASRModelAsset(
                id: "mock-\(model.rawValue)",
                provider: "mock",
                modelName: model,
                displayName: model.rawValue,
                status: readyModels.contains(model) ? .ready : .notDownloaded,
                downloadProgress: readyModels.contains(model) ? 100 : 0,
                localPath: readyModels.contains(model) ? "/tmp/models/\(model.rawValue)" : "",
                checksum: "",
                sizeBytes: 0,
                isDefault: model == ASRModelName.default,
                errorMessage: "",
                createdAt: now,
                updatedAt: now
            )
        }
    }

    public func prepare(model: ASRModelName, trigger: ModelPrepareTrigger) async throws -> ASRModelAsset {
        try await verify(model: model)
    }

    public func verify(model: ASRModelName) async throws -> ASRModelAsset {
        guard readyModels.contains(model) else {
            throw RecordingAgentError(code: .modelMissing, message: "模型 \(model.rawValue) 未下载")
        }
        return try await listModels().first { $0.modelName == model }!
    }
}

public enum MockASRResponse: Equatable, Sendable {
    case success(text: String, language: String)
    case failure(code: RecordingAgentErrorCode, message: String)
}

public final class MockASREngine: LocalASREngine, @unchecked Sendable {
    private let results: [Int: MockASRResponse]

    public init(results: [Int: MockASRResponse]) {
        self.results = results
    }

    public func checkReady(model: ASRModelName) async -> Result<Void, RecordingAgentError> {
        .success(())
    }

    public func transcribe(slice: AudioSlice, model: ASRModelName, language: LanguageMode) async -> Result<ASRSegmentResult, RecordingAgentError> {
        switch results[slice.index] ?? .success(text: "Mock transcript \(slice.index)", language: language.whisperLanguage ?? "auto") {
        case let .success(text, detectedLanguage):
            return .success(ASRSegmentResult(text: text, language: detectedLanguage, provider: "mock", modelName: model))
        case let .failure(code, message):
            return .failure(RecordingAgentError(code: code, message: message))
        }
    }
}
