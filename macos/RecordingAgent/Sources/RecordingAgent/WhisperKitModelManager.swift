import Foundation
import CoreML
import RecordingAgentCore

actor WhisperKitModelManager: ModelManaging {
    private let baseDirectory: URL
    private let fileSystem: FileSystemModelManager
    private let downloader: WhisperKitModelFileDownloader

    init(baseDirectory: URL) {
        self.baseDirectory = baseDirectory
        self.fileSystem = FileSystemModelManager(baseDirectory: baseDirectory)
        self.downloader = WhisperKitModelFileDownloader(baseDirectory: baseDirectory)
    }

    func listModels() async throws -> [ASRModelAsset] {
        try await fileSystem.listModels()
    }

    func prepare(model: ASRModelName, trigger: ModelPrepareTrigger) async throws -> ASRModelAsset {
        try await prepare(model: model, trigger: trigger, progress: nil)
    }

    func prepare(model: ASRModelName, trigger: ModelPrepareTrigger, progress: ModelPreparationProgressHandler?) async throws -> ASRModelAsset {
        try FileManager.default.createDirectory(at: baseDirectory, withIntermediateDirectories: true)

        try await downloader.downloadRequiredFiles(for: model, progress: progress)
        await progress?(ModelPreparationProgress(
            modelName: model,
            status: .verifying,
            completedUnitCount: 1,
            totalUnitCount: 1,
            message: "校验本地模型"
        ))
        try prepareLocalModelLayout(for: model)

        let asset = try await verify(model: model)
        await progress?(ModelPreparationProgress(
            modelName: model,
            status: .ready,
            completedUnitCount: 1,
            totalUnitCount: 1,
            message: "模型已就绪"
        ))
        return asset
    }

    func verify(model: ASRModelName) async throws -> ASRModelAsset {
        try await fileSystem.verify(model: model)
    }

    private func prepareLocalModelLayout(for model: ASRModelName) throws {
        let fileManager = FileManager.default
        let repositoryRoot = baseDirectory
            .appending(path: "models")
            .appending(path: "argmaxinc")
            .appending(path: "whisperkit-coreml")
        let sourceDirectory = repositoryRoot.appending(path: model.whisperKitRepositoryDirectoryName)
        let sharedMelModel = repositoryRoot
            .appending(path: ASRModelName.whisperKitSharedMelPackageDirectoryName)
            .appending(path: "MelSpectrogram.mlpackage")
            .appending(path: "Data")
            .appending(path: "com.apple.CoreML")
            .appending(path: "model.mlmodel")

        let preparedRoot = baseDirectory.appending(path: "prepared")
        let preparedDirectory = preparedRoot.appending(path: model.whisperKitRepositoryDirectoryName)
        let temporaryDirectory = preparedRoot.appending(path: ".\(model.whisperKitRepositoryDirectoryName)-\(UUID().uuidString)")

        try fileManager.createDirectory(at: temporaryDirectory, withIntermediateDirectories: true)
        do {
            try compileModel(component: "AudioEncoder", from: sourceDirectory, to: temporaryDirectory)
            try compileModel(sourceModel: sharedMelModel, component: "MelSpectrogram", to: temporaryDirectory)
            try compileModel(component: "TextDecoder", from: sourceDirectory, to: temporaryDirectory)
            try copyOptionalFile(named: "config.json", from: sourceDirectory, to: temporaryDirectory)
            try copyOptionalFile(named: "generation_config.json", from: sourceDirectory, to: temporaryDirectory)

            if fileManager.fileExists(atPath: preparedDirectory.path) {
                try fileManager.removeItem(at: preparedDirectory)
            }
            try fileManager.createDirectory(at: preparedRoot, withIntermediateDirectories: true)
            try fileManager.moveItem(at: temporaryDirectory, to: preparedDirectory)
        } catch {
            try? fileManager.removeItem(at: temporaryDirectory)
            throw error
        }
    }

    private func compileModel(component: String, from sourceDirectory: URL, to preparedDirectory: URL) throws {
        let sourceComponent = sourceDirectory.appending(path: "\(component).mlmodelc")
        let sourceModel = sourceComponent.appending(path: "model.mlmodel")
        try compileModel(sourceModel: sourceModel, component: component, to: preparedDirectory)
    }

    private func compileModel(sourceModel: URL, component: String, to preparedDirectory: URL) throws {
        guard FileManager.default.fileExists(atPath: sourceModel.path) else {
            throw RecordingAgentError(
                code: .downloadFailed,
                message: "模型 \(component) 缺少 model.mlmodel：\(sourceModel.path)"
            )
        }

        do {
            let compiledModel = try MLModel.compileModel(at: sourceModel)
            let destination = preparedDirectory.appending(path: "\(component).mlmodelc")
            if FileManager.default.fileExists(atPath: destination.path) {
                try FileManager.default.removeItem(at: destination)
            }
            try FileManager.default.copyItem(at: compiledModel, to: destination)
        } catch {
            throw RecordingAgentError(
                code: .verifyFailed,
                message: "模型 \(component) 编译失败：\(error.localizedDescription)"
            )
        }
    }

    private func copyOptionalFile(named filename: String, from sourceDirectory: URL, to destinationDirectory: URL) throws {
        let source = sourceDirectory.appending(path: filename)
        guard FileManager.default.fileExists(atPath: source.path) else { return }
        try FileManager.default.copyItem(at: source, to: destinationDirectory.appending(path: filename))
    }

}
