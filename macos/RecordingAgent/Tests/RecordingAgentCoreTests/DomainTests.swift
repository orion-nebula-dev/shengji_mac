import XCTest
@testable import RecordingAgentCore

final class DomainTests: XCTestCase {
    func testDefaultModelAndSelectableModelsMatchMVP() {
        XCTAssertEqual(ASRModelName.default, .small)
        XCTAssertEqual(ASRModelName.allCases.map(\.rawValue), ["tiny", "base", "small"])
    }

    func testLanguageModesMatchMVP() {
        XCTAssertEqual(LanguageMode.allCases.map(\.rawValue), ["auto", "zh", "en"])
        XCTAssertEqual(LanguageMode.allCases.map(\.displayName), ["自动识别", "中文", "英文"])
        XCTAssertEqual(LanguageMode.auto.whisperLanguage, nil)
        XCTAssertEqual(LanguageMode.zh.whisperLanguage, "zh")
        XCTAssertEqual(LanguageMode.en.whisperLanguage, "en")
    }

    func testWhisperKitPolicyKeepsAutoAsLanguageDetectionNotEnglishPrompt() {
        XCTAssertEqual(
            LocalASRDecodingPolicy.whisperKitPolicy(for: .auto),
            LocalASRDecodingPolicy(taskName: "transcribe", languageCode: nil, shouldDetectLanguage: true, temperatureFallbackCount: 1)
        )
        XCTAssertEqual(
            LocalASRDecodingPolicy.whisperKitPolicy(for: .zh),
            LocalASRDecodingPolicy(taskName: "transcribe", languageCode: "zh", shouldDetectLanguage: false, temperatureFallbackCount: 1)
        )
        XCTAssertEqual(
            LocalASRDecodingPolicy.whisperKitPolicy(for: .en),
            LocalASRDecodingPolicy(taskName: "transcribe", languageCode: "en", shouldDetectLanguage: false, temperatureFallbackCount: 1)
        )
    }

    func testASRModelAssetStatusProvidesDisplayNameAndRecoveryHintForSettingsUI() {
        let expectations: [(status: ASRModelAssetStatus, displayName: String, recoveryHint: String)] = [
            (.notDownloaded, "未下载", "需要先准备模型"),
            (.downloading, "下载中", "请等待下载完成"),
            (.verifying, "校验中", "请等待校验完成"),
            (.ready, "已就绪", "可以开始记录"),
            (.failed, "准备失败", "可重试准备模型")
        ]

        for expectation in expectations {
            XCTAssertEqual(expectation.status.displayName, expectation.displayName)
            XCTAssertEqual(expectation.status.recoveryHint, expectation.recoveryHint)
        }
    }

    func testASRModelAssetProvidesPreparationUIState() {
        let expectations: [(status: ASRModelAssetStatus, progress: Int, actionTitle: String, progressText: String, canPrepare: Bool, canRecord: Bool)] = [
            (.notDownloaded, 0, "准备模型", "0%", true, false),
            (.downloading, 38, "准备中", "38%", false, false),
            (.verifying, 100, "校验中", "100%", false, false),
            (.ready, 100, "重新准备模型", "100%", true, true),
            (.failed, 42, "重试准备模型", "42% 后失败", true, false)
        ]

        for expectation in expectations {
            var asset = ASRModelAsset.fixture(status: expectation.status, downloadProgress: expectation.progress)

            XCTAssertEqual(asset.preparationActionTitle, expectation.actionTitle)
            XCTAssertEqual(asset.progressText, expectation.progressText)
            XCTAssertEqual(asset.allowsPreparationStart, expectation.canPrepare)
            XCTAssertEqual(asset.isReadyForRecording, expectation.canRecord)

            asset.downloadProgress = -20
            XCTAssertEqual(asset.clampedDownloadProgress, 0)
            asset.downloadProgress = 120
            XCTAssertEqual(asset.clampedDownloadProgress, 100)
        }
    }

    func testModelPreparationProgressClampsCountsAndBuildsDisplayText() {
        let downloading = ModelPreparationProgress(
            modelName: .small,
            status: .downloading,
            completedUnitCount: 3,
            totalUnitCount: 12,
            message: "TextDecoder weight.bin"
        )

        XCTAssertEqual(downloading.percent, 25)
        XCTAssertEqual(downloading.displayText, "25% · TextDecoder weight.bin")

        let overflow = ModelPreparationProgress(
            modelName: .small,
            status: .verifying,
            completedUnitCount: 20,
            totalUnitCount: 12,
            message: "校验本地模型"
        )
        XCTAssertEqual(overflow.percent, 100)

        let empty = ModelPreparationProgress(
            modelName: .small,
            status: .downloading,
            completedUnitCount: -1,
            totalUnitCount: 0,
            message: ""
        )
        XCTAssertEqual(empty.percent, 0)
        XCTAssertEqual(empty.displayText, "0%")
    }

    func testModelDownloadFileProgressBuildsByteLevelPreparationProgress() {
        let fileProgress = ModelDownloadFileProgress(
            fileName: "weight.bin",
            fileIndex: 1,
            totalFileCount: 4,
            completedBytes: 1_572_864,
            totalBytes: 3_145_728
        )

        XCTAssertEqual(fileProgress.filePercent, 50)
        XCTAssertEqual(fileProgress.overallCompletedUnitCount, 150)
        XCTAssertEqual(fileProgress.overallTotalUnitCount, 400)
        XCTAssertEqual(fileProgress.displayMessage, "weight.bin · 1.5 MB / 3.0 MB")

        let preparationProgress = fileProgress.preparationProgress(modelName: .small)
        XCTAssertEqual(preparationProgress.status, .downloading)
        XCTAssertEqual(preparationProgress.percent, 37)
        XCTAssertEqual(preparationProgress.displayText, "37% · weight.bin · 1.5 MB / 3.0 MB")
    }

    func testAudioInputLevelClampsAndFormatsPercent() {
        XCTAssertEqual(AudioInputLevel(linear: -0.2).linear, 0)
        XCTAssertEqual(AudioInputLevel(linear: 1.7).linear, 1)
        XCTAssertEqual(AudioInputLevel(linear: .nan).linear, 0)

        let level = AudioInputLevel(linear: 0.428)
        XCTAssertEqual(level.percent, 43)
        XCTAssertEqual(level.displayText, "43%")
    }

    func testTranscriptPostProcessorRejectsPureSubtitleStyleHallucinations() {
        let samples = [
            "(字幕製作:貝爾)",
            "(台語)",
            "(觀眾留言)",
            "[MUSIC PLAYING]",
            "(speaking in foreign language)"
        ]

        for sample in samples {
            switch ASRTranscriptPostProcessor.process(text: sample) {
            case let .failure(error):
                XCTAssertEqual(error.code, .transcribeFailed, "Expected transcribe_failed for \(sample)")
            case .success:
                XCTFail("Expected to reject \(sample)")
            }
        }
    }

    func testTranscriptPostProcessorStripsRolePrefixButKeepsRecognizedSpeech() throws {
        let processed = try ASRTranscriptPostProcessor.process(text: "(主持人: 这一面叫什么? 怎么写是台语?)").get()
        XCTAssertEqual(processed, "这一面叫什么? 怎么写是台语?")
    }

    func testTranscriptPostProcessorKeepsNormalChineseAndMixedSpeech() throws {
        let processed = try ASRTranscriptPostProcessor.process(text: "然后录音过程中你开始进入一个切片 Excuse me tell me why").get()
        XCTAssertEqual(processed, "然后录音过程中你开始进入一个切片 Excuse me tell me why")
    }

    func testRecordingAgentErrorCodesAreStableForPersistenceAndUI() {
        XCTAssertEqual(
            RecordingAgentErrorCode.allCases.map(\.rawValue),
            [
                "permission_denied",
                "model_missing",
                "download_failed",
                "verify_failed",
                "asr_engine_unavailable",
                "write_failed",
                "transcribe_failed"
            ]
        )
    }
}

private extension ASRModelAsset {
    static func fixture(status: ASRModelAssetStatus, downloadProgress: Int) -> ASRModelAsset {
        ASRModelAsset(
            id: "fixture-small",
            provider: "fixture",
            modelName: .small,
            displayName: "small",
            status: status,
            downloadProgress: downloadProgress,
            localPath: "",
            checksum: "",
            sizeBytes: 0,
            isDefault: true,
            errorMessage: "",
            createdAt: Date(timeIntervalSince1970: 0),
            updatedAt: Date(timeIntervalSince1970: 0)
        )
    }
}
