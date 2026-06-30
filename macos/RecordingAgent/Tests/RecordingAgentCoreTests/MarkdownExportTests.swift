import XCTest
@testable import RecordingAgentCore

final class MarkdownExportTests: XCTestCase {
    func testMarkdownExportSuggestsSafeMarkdownFileName() {
        XCTAssertEqual(MarkdownTranscriptExporter.suggestedFileName(title: "产品例会"), "产品例会.md")
        XCTAssertEqual(MarkdownTranscriptExporter.suggestedFileName(title: "  A/B: test?  "), "A-B- test.md")
        XCTAssertEqual(MarkdownTranscriptExporter.suggestedFileName(title: " "), "声记导出.md")
    }

    func testMarkdownExportUsesExplicitCurrentDraft() {
        let now = Date(timeIntervalSince1970: 1_782_432_000)
        let recording = Recording(
            id: "rec-1",
            title: "产品例会",
            status: .completed,
            audioFilePath: "/tmp/full.wav",
            durationMilliseconds: 30_000,
            sampleRate: 16_000,
            channels: 1,
            modelName: .small,
            languageMode: .zh,
            transcriptSegmentCount: 1,
            failedSegmentCount: 0,
            errorCode: nil,
            errorMessage: nil,
            startedAt: now,
            endedAt: now.addingTimeInterval(30),
            createdAt: now,
            updatedAt: now
        )
        let draft = TranscriptDraft(
            kind: .corrected,
            segments: [
                TranscriptDraftSegment(
                    segmentIndex: 0,
                    startMilliseconds: 0,
                    endMilliseconds: 30_000,
                    text: "这是人工修正后的当前稿件",
                    isFailed: false,
                    errorMessage: nil
                )
            ]
        )

        let markdown = MarkdownTranscriptExporter.format(
            recording: recording,
            draft: draft,
            calendar: .gregorianUTC
        )

        XCTAssertTrue(markdown.contains("- 稿件：修正稿"))
        XCTAssertTrue(markdown.contains("### 00:00-00:30"))
        XCTAssertTrue(markdown.contains("这是人工修正后的当前稿件"))
        XCTAssertFalse(markdown.contains("实时原始稿"))
    }

    func testMarkdownExportKeepsUserControlledTextInsideCodeFences() {
        let now = Date(timeIntervalSince1970: 1_782_432_000)
        let recording = Recording(
            id: "rec-1",
            title: "# 计划\n- fake metadata",
            status: .completed,
            audioFilePath: "/tmp/full.wav",
            durationMilliseconds: 15_000,
            sampleRate: 16_000,
            channels: 1,
            modelName: .small,
            languageMode: .zh,
            transcriptSegmentCount: 1,
            failedSegmentCount: 1,
            errorCode: nil,
            errorMessage: nil,
            startedAt: now,
            endedAt: now.addingTimeInterval(15),
            createdAt: now,
            updatedAt: now
        )
        let draft = TranscriptDraft(
            kind: .realtimeRaw,
            segments: [
                TranscriptDraftSegment(
                    segmentIndex: 0,
                    startMilliseconds: 0,
                    endMilliseconds: 15_000,
                    text: "第一行\n# 伪标题\n```swift\nlet injected = true\n```",
                    isFailed: false,
                    errorMessage: nil
                ),
                TranscriptDraftSegment(
                    segmentIndex: 1,
                    startMilliseconds: 15_000,
                    endMilliseconds: 30_000,
                    text: "",
                    isFailed: true,
                    errorMessage: "失败原因\n- fake item"
                )
            ]
        )

        let markdown = MarkdownTranscriptExporter.format(
            recording: recording,
            draft: draft,
            calendar: .gregorianUTC
        )

        XCTAssertEqual(
            markdown,
            """
            # \\# 计划 - fake metadata

            - 创建时间：2026-06-26 00:00
            - 时长：0:15
            - 语言：中文
            - 模型：small
            - 稿件：实时原始稿

            ## 正文

            ### 00:00-00:15

            ````text
            第一行
            # 伪标题
            ```swift
            let injected = true
            ```
            ````

            ### 00:15-00:30

            ```text
            转写失败：失败原因
            - fake item
            ```
            """
        )
    }

    func testMarkdownExportIncludesRecordingMetadataCurrentDraftAndTimestampedSegments() {
        let now = Date(timeIntervalSince1970: 1_782_432_000)
        let detail = RecordingDetail(
            recording: Recording(
                id: "rec-1",
                title: "产品例会",
                status: .completed,
                audioFilePath: "/tmp/full.wav",
                durationMilliseconds: 65_000,
                sampleRate: 16_000,
                channels: 1,
                modelName: .small,
                languageMode: .auto,
                transcriptSegmentCount: 2,
                failedSegmentCount: 1,
                errorCode: nil,
                errorMessage: nil,
                startedAt: now,
                endedAt: now.addingTimeInterval(65),
                createdAt: now,
                updatedAt: now
            ),
            audioSegments: [],
            segments: [
                TranscriptSegment(
                    id: "ts-2",
                    recordingID: "rec-1",
                    audioSegmentID: "slice-2",
                    segmentIndex: 1,
                    startMilliseconds: 15_000,
                    endMilliseconds: 30_000,
                    text: "第二段结论",
                    language: "zh",
                    status: .success,
                    provider: "whisperkit",
                    modelName: .small,
                    errorCode: nil,
                    errorMessage: nil,
                    createdAt: now
                ),
                TranscriptSegment(
                    id: "ts-failed",
                    recordingID: "rec-1",
                    audioSegmentID: "slice-failed",
                    segmentIndex: 2,
                    startMilliseconds: 30_000,
                    endMilliseconds: 45_000,
                    text: "",
                    language: "zh",
                    status: .failed,
                    provider: "whisperkit",
                    modelName: .small,
                    errorCode: .transcribeFailed,
                    errorMessage: "本地转写失败",
                    createdAt: now
                ),
                TranscriptSegment(
                    id: "ts-1",
                    recordingID: "rec-1",
                    audioSegmentID: "slice-1",
                    segmentIndex: 0,
                    startMilliseconds: 0,
                    endMilliseconds: 15_000,
                    text: "第一段内容",
                    language: "zh",
                    status: .success,
                    provider: "whisperkit",
                    modelName: .small,
                    errorCode: nil,
                    errorMessage: nil,
                    createdAt: now
                )
            ],
            events: [],
            steps: [],
            artifacts: []
        )

        let markdown = MarkdownTranscriptExporter.format(
            detail: detail,
            calendar: .gregorianUTC
        )

        XCTAssertEqual(
            markdown,
            """
            # 产品例会

            - 创建时间：2026-06-26 00:00
            - 时长：1:05
            - 语言：自动识别
            - 模型：small
            - 稿件：实时原始稿

            ## 正文

            ### 00:00-00:15

            ```text
            第一段内容
            ```

            ### 00:15-00:30

            ```text
            第二段结论
            ```

            ### 00:30-00:45

            ```text
            转写失败：本地转写失败
            ```
            """
        )
    }
}
