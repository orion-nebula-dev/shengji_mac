import Foundation
import RecordingAgentCore

struct AppContainer {
    let store: SQLiteRecordingStore
    let agent: RecordingAgent
    let modelManager: WhisperKitModelManager
    let modelBaseDirectory: URL
    let databaseURL: URL

    static func live() throws -> AppContainer {
        let root = try applicationSupportRoot()
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let databaseURL = root.appending(path: "recording-agent.sqlite")
        let modelBaseDirectory = root.appending(path: "models/whisperkit")
        try FileManager.default.createDirectory(at: modelBaseDirectory, withIntermediateDirectories: true)

        let store = try SQLiteRecordingStore.open(path: databaseURL.path)
        let modelManager = WhisperKitModelManager(baseDirectory: modelBaseDirectory)
        let permission = NativeMicrophonePermissionService()
        let audio = NativeAudioCaptureService()
        let asr = WhisperKitASREngine(modelBaseDirectory: modelBaseDirectory)
        let agent = RecordingAgent(
            store: store,
            permissionService: permission,
            audioCaptureService: audio,
            modelManager: modelManager,
            asrEngine: asr
        )
        return AppContainer(store: store, agent: agent, modelManager: modelManager, modelBaseDirectory: modelBaseDirectory, databaseURL: databaseURL)
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
