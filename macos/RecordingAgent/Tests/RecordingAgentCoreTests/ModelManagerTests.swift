import XCTest
@testable import RecordingAgentCore

final class ModelManagerTests: XCTestCase {
    func testWhisperKitVariantDirectoryIsRecognizedAsReadyModel() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentModelManagerTests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }

        let modelDirectory = root.appending(path: "openai_whisper-small")
        try FileManager.default.createDirectory(at: modelDirectory, withIntermediateDirectories: true)
        try createCompiledModelBundle(named: "AudioEncoder", in: modelDirectory)
        try createCompiledModelBundle(named: "MelSpectrogram", in: modelDirectory)
        try createCompiledModelBundle(named: "TextDecoder", in: modelDirectory)
        try Data("fixture".utf8).write(to: modelDirectory.appending(path: "tokenizer.json"))
        try Data("fixture".utf8).write(to: modelDirectory.appending(path: "vocab.json"))
        try Data("fixture".utf8).write(to: modelDirectory.appending(path: "merges.txt"))

        let manager = FileSystemModelManager(baseDirectory: root)
        let asset = try await manager.verify(model: .small)

        XCTAssertEqual(asset.status, .ready)
        XCTAssertEqual(asset.downloadProgress, 100)
        XCTAssertEqual(asset.localPath, modelDirectory.path)
    }

    func testWhisperKitCoreMLDirectoryRequiresTokenizerCache() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentModelManagerTests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }

        let modelDirectory = root.appending(path: "openai_whisper-small")
        try FileManager.default.createDirectory(at: modelDirectory, withIntermediateDirectories: true)
        try createCompiledModelBundle(named: "AudioEncoder", in: modelDirectory)
        try createCompiledModelBundle(named: "MelSpectrogram", in: modelDirectory)
        try createCompiledModelBundle(named: "TextDecoder", in: modelDirectory)

        let manager = FileSystemModelManager(baseDirectory: root)
        do {
            _ = try await manager.verify(model: .small)
            XCTFail("Expected model_missing when tokenizer cache is absent")
        } catch let error as RecordingAgentError {
            XCTAssertEqual(error.code, .modelMissing)
        }
    }

    func testShallowWhisperKitDownloadWithoutCompiledManifestIsNotReady() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentModelManagerTests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }

        let modelDirectory = root.appending(path: "openai_whisper-small")
        try FileManager.default.createDirectory(at: modelDirectory, withIntermediateDirectories: true)
        for name in ["AudioEncoder", "MelSpectrogram", "TextDecoder"] {
            let shallowDirectory = modelDirectory.appending(path: "\(name).mlmodelc")
            try FileManager.default.createDirectory(at: shallowDirectory, withIntermediateDirectories: true)
            try Data("fixture".utf8).write(to: shallowDirectory.appending(path: "model.mil"))
        }

        let manager = FileSystemModelManager(baseDirectory: root)
        do {
            _ = try await manager.verify(model: .small)
            XCTFail("Expected model_missing for shallow, uncompiled Core ML directories")
        } catch let error as RecordingAgentError {
            XCTAssertEqual(error.code, .modelMissing)
        }
    }

    func testPreparedWhisperKitPackageDirectoryIsRecognizedAsReadyModel() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentModelManagerTests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }

        let modelDirectory = root
            .appending(path: "prepared")
            .appending(path: "openai_whisper-small")
        try createModelPackage(named: "AudioEncoder", in: modelDirectory)
        try createModelPackage(named: "MelSpectrogram", in: modelDirectory)
        try createModelPackage(named: "TextDecoder", in: modelDirectory)
        try createTokenizerCache(in: root, model: .small)

        let manager = FileSystemModelManager(baseDirectory: root)
        let asset = try await manager.verify(model: .small)

        XCTAssertEqual(asset.status, .ready)
        XCTAssertEqual(
            URL(fileURLWithPath: asset.localPath).standardizedFileURL.path,
            modelDirectory.standardizedFileURL.path
        )
    }

    func testCoreMLCompiledDirectoryWithoutManifestIsRecognizedAsReadyModel() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentModelManagerTests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }

        let modelDirectory = root
            .appending(path: "prepared")
            .appending(path: "openai_whisper-small")
        try createCoreMLCompiledDirectory(named: "AudioEncoder", in: modelDirectory)
        try createCoreMLCompiledDirectory(named: "MelSpectrogram", in: modelDirectory)
        try createCoreMLCompiledDirectory(named: "TextDecoder", in: modelDirectory)
        try createTokenizerCache(in: root, model: .small)

        let manager = FileSystemModelManager(baseDirectory: root)
        let asset = try await manager.verify(model: .small)

        XCTAssertEqual(asset.status, .ready)
        XCTAssertEqual(
            URL(fileURLWithPath: asset.localPath).standardizedFileURL.path,
            modelDirectory.standardizedFileURL.path
        )
    }

    func testPreparedDirectoryTakesPrecedenceOverRepositoryCache() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentModelManagerTests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }

        let repoDirectory = root
            .appending(path: "models")
            .appending(path: "argmaxinc")
            .appending(path: "whisperkit-coreml")
            .appending(path: "openai_whisper-small")
        try FileManager.default.createDirectory(at: repoDirectory, withIntermediateDirectories: true)
        try createCompiledModelBundle(named: "AudioEncoder", in: repoDirectory)
        try createCompiledModelBundle(named: "MelSpectrogram", in: repoDirectory)
        try createCompiledModelBundle(named: "TextDecoder", in: repoDirectory)

        let preparedDirectory = root
            .appending(path: "prepared")
            .appending(path: "openai_whisper-small")
        try createCoreMLCompiledDirectory(named: "AudioEncoder", in: preparedDirectory)
        try createCoreMLCompiledDirectory(named: "MelSpectrogram", in: preparedDirectory)
        try createCoreMLCompiledDirectory(named: "TextDecoder", in: preparedDirectory)
        try createTokenizerCache(in: root, model: .small)

        let manager = FileSystemModelManager(baseDirectory: root)
        let asset = try await manager.verify(model: .small)

        XCTAssertEqual(
            URL(fileURLWithPath: asset.localPath).standardizedFileURL.path,
            preparedDirectory.standardizedFileURL.path
        )
    }

    private func createCompiledModelBundle(named name: String, in directory: URL) throws {
        let modelDirectory = directory.appending(path: "\(name).mlmodelc")
        try FileManager.default.createDirectory(at: modelDirectory, withIntermediateDirectories: true)
        try Data("fixture".utf8).write(to: modelDirectory.appending(path: "Manifest.json"))
    }

    private func createCoreMLCompiledDirectory(named name: String, in directory: URL) throws {
        let modelDirectory = directory.appending(path: "\(name).mlmodelc")
        try FileManager.default.createDirectory(at: modelDirectory, withIntermediateDirectories: true)
        try Data("fixture".utf8).write(to: modelDirectory.appending(path: "coremldata.bin"))
        try Data("fixture".utf8).write(to: modelDirectory.appending(path: "model.mil"))
    }

    private func createModelPackage(named name: String, in directory: URL) throws {
        let modelDirectory = directory
            .appending(path: "\(name).mlpackage")
            .appending(path: "Data")
            .appending(path: "com.apple.CoreML")
        try FileManager.default.createDirectory(at: modelDirectory, withIntermediateDirectories: true)
        try Data("fixture".utf8).write(to: modelDirectory.appending(path: "model.mlmodel"))
    }

    private func createTokenizerCache(in root: URL, model: ASRModelName) throws {
        let tokenizerDirectory = root
            .appending(path: "models")
            .appending(path: "openai")
            .appending(path: "whisper-\(model.rawValue)")
        try FileManager.default.createDirectory(at: tokenizerDirectory, withIntermediateDirectories: true)
        try Data("fixture".utf8).write(to: tokenizerDirectory.appending(path: "tokenizer.json"))
        try Data("fixture".utf8).write(to: tokenizerDirectory.appending(path: "vocab.json"))
        try Data("fixture".utf8).write(to: tokenizerDirectory.appending(path: "merges.txt"))
    }
}
