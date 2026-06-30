import Foundation
import RecordingAgentCore

typealias ModelPreparationProgressHandler = (ModelPreparationProgress) async -> Void

struct WhisperKitModelFileDownloader {
    private static let modelRepositoryID = "argmaxinc/whisperkit-coreml"
    private static let endpoint = "https://huggingface.co"

    private let baseDirectory: URL
    private let fileManager: FileManager

    init(baseDirectory: URL, fileManager: FileManager = .default) {
        self.baseDirectory = baseDirectory
        self.fileManager = fileManager
    }

    func downloadRequiredFiles(for model: ASRModelName, progress: ModelPreparationProgressHandler? = nil) async throws {
        let missingDownloads = requiredDownloads(for: model).filter { item in
            !fileManager.fileExists(atPath: item.destination.path) || fileSize(at: item.destination) <= 0
        }
        let total = max(missingDownloads.count, 1)

        for (index, item) in missingDownloads.enumerated() {
            try Task.checkCancellation()
            await progress?(ModelPreparationProgress(
                modelName: model,
                status: .downloading,
                completedUnitCount: index,
                totalUnitCount: total,
                message: item.displayName
            ))
            try await download(path: item.path, from: item.repositoryID, to: item.destination) { completedBytes, totalBytes in
                let fileProgress = ModelDownloadFileProgress(
                    fileName: item.displayName,
                    fileIndex: index,
                    totalFileCount: total,
                    completedBytes: completedBytes,
                    totalBytes: totalBytes
                )
                await progress?(fileProgress.preparationProgress(modelName: model))
            }
            await progress?(ModelPreparationProgress(
                modelName: model,
                status: .downloading,
                completedUnitCount: index + 1,
                totalUnitCount: total,
                message: item.displayName
            ))
        }
    }

    private var repositoryRoot: URL {
        baseDirectory
            .appending(path: "models")
            .appending(path: "argmaxinc")
            .appending(path: "whisperkit-coreml")
    }

    private func tokenizerRoot(for model: ASRModelName) -> URL {
        baseDirectory
            .appending(path: "models")
            .appending(path: "openai")
            .appending(path: "whisper-\(model.rawValue)")
    }

    private static func requiredRemotePaths(for model: ASRModelName) -> [String] {
        let modelDirectory = model.whisperKitRepositoryDirectoryName
        let sharedMelDirectory = ASRModelName.whisperKitSharedMelPackageDirectoryName
        return [
            "\(modelDirectory)/AudioEncoder.mlmodelc/model.mlmodel",
            "\(modelDirectory)/AudioEncoder.mlmodelc/weights/weight.bin",
            "\(modelDirectory)/TextDecoder.mlmodelc/model.mlmodel",
            "\(modelDirectory)/TextDecoder.mlmodelc/weights/weight.bin",
            "\(modelDirectory)/config.json",
            "\(modelDirectory)/generation_config.json",
            "\(sharedMelDirectory)/MelSpectrogram.mlpackage/Data/com.apple.CoreML/model.mlmodel",
            "\(sharedMelDirectory)/MelSpectrogram.mlpackage/Data/com.apple.CoreML/weights/weight.bin",
            "\(sharedMelDirectory)/MelSpectrogram.mlpackage/Manifest.json"
        ]
    }

    private static let requiredTokenizerPaths = [
        "config.json",
        "generation_config.json",
        "merges.txt",
        "normalizer.json",
        "preprocessor_config.json",
        "special_tokens_map.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "vocab.json"
    ]

    private static func tokenizerRepositoryID(for model: ASRModelName) -> String {
        "openai/whisper-\(model.rawValue)"
    }

    private func requiredDownloads(for model: ASRModelName) -> [RequiredDownload] {
        let modelDownloads = Self.requiredRemotePaths(for: model).map { path in
            RequiredDownload(
                repositoryID: Self.modelRepositoryID,
                path: path,
                destination: repositoryRoot.appending(path: path),
                displayName: URL(fileURLWithPath: path).lastPathComponent
            )
        }

        let tokenizerDownloads = Self.requiredTokenizerPaths.map { path in
            RequiredDownload(
                repositoryID: Self.tokenizerRepositoryID(for: model),
                path: path,
                destination: tokenizerRoot(for: model).appending(path: path),
                displayName: "tokenizer/\(path)"
            )
        }

        return modelDownloads + tokenizerDownloads
    }

    private func download(path: String, from repositoryID: String, to destination: URL, progress: ((Int64, Int64?) async -> Void)? = nil) async throws {
        try fileManager.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        let temporaryFile = destination.appendingPathExtension("download")
        guard let remoteURL = URL(string: Self.endpoint + "/" + repositoryID + "/resolve/main/" + path) else {
            throw RecordingAgentError(code: .downloadFailed, message: "模型文件下载地址无效：\(repositoryID)/\(path)")
        }
        let totalBytes = await remoteContentLength(for: remoteURL)
        await progress?(fileSize(at: temporaryFile), totalBytes)

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/curl")
        process.arguments = [
            "--location",
            "--fail",
            "--silent",
            "--show-error",
            "--retry", "3",
            "--retry-all-errors",
            "--retry-delay", "2",
            "--connect-timeout", "30",
            "--http1.1",
            "--continue-at", "-",
            "--output", temporaryFile.path,
            remoteURL.absoluteString
        ]

        let standardError = Pipe()
        process.standardError = standardError
        process.standardOutput = Pipe()

        try await withTaskCancellationHandler {
            try process.run()
            while process.isRunning {
                try Task.checkCancellation()
                await progress?(fileSize(at: temporaryFile), totalBytes)
                try await Task.sleep(nanoseconds: 300_000_000)
            }
            process.waitUntilExit()
        } onCancel: {
            process.terminate()
        }

        try Task.checkCancellation()
        await progress?(fileSize(at: temporaryFile), totalBytes)

        guard process.terminationStatus == 0 else {
            let errorData = standardError.fileHandleForReading.readDataToEndOfFile()
            let errorText = String(data: errorData, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            throw RecordingAgentError(
                code: .downloadFailed,
                message: "模型文件下载失败：\(repositoryID)/\(path)\(errorText.isEmpty ? "" : "，\(errorText)")"
            )
        }

        guard fileSize(at: temporaryFile) > 0 else {
            throw RecordingAgentError(code: .downloadFailed, message: "模型文件下载为空：\(repositoryID)/\(path)")
        }

        if fileManager.fileExists(atPath: destination.path) {
            try fileManager.removeItem(at: destination)
        }
        try fileManager.moveItem(at: temporaryFile, to: destination)
    }

    private func remoteContentLength(for remoteURL: URL) async -> Int64? {
        var request = URLRequest(url: remoteURL)
        request.httpMethod = "HEAD"
        request.timeoutInterval = 30
        do {
            let (_, response) = try await URLSession.shared.data(for: request)
            let length = response.expectedContentLength
            return length > 0 ? length : nil
        } catch {
            return nil
        }
    }

    private func fileSize(at url: URL) -> Int64 {
        let attributes = try? fileManager.attributesOfItem(atPath: url.path)
        return attributes?[.size] as? Int64 ?? 0
    }
}

private struct RequiredDownload {
    let repositoryID: String
    let path: String
    let destination: URL
    let displayName: String
}
