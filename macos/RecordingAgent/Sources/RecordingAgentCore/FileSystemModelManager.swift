import Foundation

public final class FileSystemModelManager: ModelManaging, @unchecked Sendable {
    public let baseDirectory: URL
    private let fileManager: FileManager

    public init(baseDirectory: URL, fileManager: FileManager = .default) {
        self.baseDirectory = baseDirectory
        self.fileManager = fileManager
    }

    public func listModels() async throws -> [ASRModelAsset] {
        let now = Date()
        return ASRModelName.allCases.map { model in
            let modelDirectory = readyDirectory(for: model)
            let ready = modelDirectory != nil
            return ASRModelAsset(
                id: "whisperkit-\(model.rawValue)",
                provider: "whisperkit",
                modelName: model,
                displayName: model.rawValue,
                status: ready ? .ready : .notDownloaded,
                downloadProgress: ready ? 100 : 0,
                localPath: modelDirectory?.path ?? "",
                checksum: "",
                sizeBytes: 0,
                isDefault: model == .default,
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
        let models = try await listModels()
        guard let asset = models.first(where: { $0.modelName == model }) else {
            throw RecordingAgentError(code: .modelMissing, message: "模型 \(model.rawValue) 未登记")
        }
        guard asset.status == .ready else {
            throw RecordingAgentError(code: .modelMissing, message: "模型 \(model.rawValue) 未下载或缺少必要文件")
        }
        return asset
    }

    public func directory(for model: ASRModelName) -> URL {
        baseDirectory.appending(path: model.whisperKitRepositoryDirectoryName)
    }

    public static let requiredWhisperKitFiles: [String] = [
        "AudioEncoder.mlmodelc",
        "MelSpectrogram.mlmodelc",
        "TextDecoder.mlmodelc",
        "config.json",
        "generation_config.json",
        "tokenizer.json"
    ]

    private static let requiredCoreMLModelNames = [
        "AudioEncoder",
        "MelSpectrogram",
        "TextDecoder"
    ]

    private static let requiredTokenizerFiles = [
        "tokenizer.json",
        "vocab.json",
        "merges.txt"
    ]

    private func readyDirectory(for model: ASRModelName) -> URL? {
        candidateDirectories(for: model).first { isReadyModelDirectory($0, model: model) }
    }

    private func candidateDirectories(for model: ASRModelName) -> [URL] {
        let preparedDirectory = baseDirectory
            .appending(path: "prepared")
            .appending(path: model.whisperKitRepositoryDirectoryName)

        let repoDirectory = baseDirectory
            .appending(path: "models")
            .appending(path: "argmaxinc")
            .appending(path: "whisperkit-coreml")
            .appending(path: model.whisperKitRepositoryDirectoryName)

        let directCandidates = [
            directory(for: model),
            preparedDirectory,
            repoDirectory,
            baseDirectory.appending(path: model.rawValue)
        ]

        let discoveredCandidates = discoverDirectories(named: model.whisperKitRepositoryDirectoryName)
        return (directCandidates + discoveredCandidates).uniquedByPath()
    }

    private func discoverDirectories(named directoryName: String) -> [URL] {
        guard fileManager.fileExists(atPath: baseDirectory.path),
              let enumerator = fileManager.enumerator(
                at: baseDirectory,
                includingPropertiesForKeys: [.isDirectoryKey],
                options: [.skipsHiddenFiles]
              )
        else {
            return []
        }

        var matches: [URL] = []
        while let url = enumerator.nextObject() as? URL {
            guard url.lastPathComponent == directoryName else { continue }
            let values = try? url.resourceValues(forKeys: [.isDirectoryKey])
            if values?.isDirectory == true {
                matches.append(url)
            }
        }
        return matches
    }

    private func isReadyModelDirectory(_ directory: URL, model: ASRModelName) -> Bool {
        Self.requiredCoreMLModelNames.allSatisfy { modelName in
            coreMLModelExists(named: modelName, in: directory)
        } && tokenizerCacheExists(for: model, modelDirectory: directory)
    }

    private func coreMLModelExists(named modelName: String, in directory: URL) -> Bool {
        let compiledModel = directory.appending(path: "\(modelName).mlmodelc")
        if fileManager.fileExists(atPath: compiledModel.appending(path: "Manifest.json").path) ||
            fileManager.fileExists(atPath: compiledModel.appending(path: "coremldata.bin").path) {
            return true
        }

        let packageModel = directory
            .appending(path: "\(modelName).mlpackage")
            .appending(path: "Data")
            .appending(path: "com.apple.CoreML")
            .appending(path: "model.mlmodel")
        return fileManager.fileExists(atPath: packageModel.path)
    }

    private func tokenizerCacheExists(for model: ASRModelName, modelDirectory: URL) -> Bool {
        if tokenizerFilesExist(in: modelDirectory) {
            return true
        }

        let tokenizerDirectory = baseDirectory
            .appending(path: "models")
            .appending(path: "openai")
            .appending(path: model.openAIWhisperRepositoryDirectoryName)
        return tokenizerFilesExist(in: tokenizerDirectory)
    }

    private func tokenizerFilesExist(in directory: URL) -> Bool {
        Self.requiredTokenizerFiles.allSatisfy { filename in
            fileManager.fileExists(atPath: directory.appending(path: filename).path)
        }
    }
}

private extension Array where Element == URL {
    func uniquedByPath() -> [URL] {
        var seen = Set<String>()
        return filter { seen.insert($0.standardizedFileURL.path).inserted }
    }
}
