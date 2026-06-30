import Foundation

public enum TranscriptDraftKind: String, Codable, Equatable, Sendable {
    case realtimeRaw = "realtime_raw"
    case highAccuracy = "high_accuracy"
    case corrected
    case summary

    public var displayName: String {
        switch self {
        case .realtimeRaw: return "实时原始稿"
        case .highAccuracy: return "会后高精度稿"
        case .corrected: return "修正稿"
        case .summary: return "摘要稿"
        }
    }
}

public struct TranscriptDraftSegment: Codable, Equatable, Sendable {
    public let segmentIndex: Int
    public let startMilliseconds: Int
    public let endMilliseconds: Int
    public let text: String
    public let isFailed: Bool
    public let errorMessage: String?

    public init(
        segmentIndex: Int,
        startMilliseconds: Int,
        endMilliseconds: Int,
        text: String,
        isFailed: Bool,
        errorMessage: String?
    ) {
        self.segmentIndex = segmentIndex
        self.startMilliseconds = startMilliseconds
        self.endMilliseconds = endMilliseconds
        self.text = text
        self.isFailed = isFailed
        self.errorMessage = errorMessage
    }

    public init(transcriptSegment: TranscriptSegment) {
        self.init(
            segmentIndex: transcriptSegment.segmentIndex,
            startMilliseconds: transcriptSegment.startMilliseconds,
            endMilliseconds: transcriptSegment.endMilliseconds,
            text: transcriptSegment.text,
            isFailed: transcriptSegment.status == .failed,
            errorMessage: transcriptSegment.errorMessage
        )
    }
}

public struct TranscriptDraft: Codable, Equatable, Sendable {
    public let kind: TranscriptDraftKind
    public let segments: [TranscriptDraftSegment]

    public init(kind: TranscriptDraftKind, segments: [TranscriptDraftSegment]) {
        self.kind = kind
        self.segments = segments
    }

    public static func realtimeRaw(segments: [TranscriptSegment]) -> TranscriptDraft {
        TranscriptDraft(
            kind: .realtimeRaw,
            segments: segments.map(TranscriptDraftSegment.init(transcriptSegment:))
        )
    }
}
