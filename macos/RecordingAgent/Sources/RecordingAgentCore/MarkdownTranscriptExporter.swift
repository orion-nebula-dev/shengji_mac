import Foundation

public enum MarkdownTranscriptExporter {
    public static func suggestedFileName(title: String) -> String {
        let invalidCharacters = CharacterSet(charactersIn: "/\\:*?\"<>|")
        let cleaned = title
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .components(separatedBy: invalidCharacters)
            .joined(separator: "-")
            .trimmingCharacters(in: CharacterSet(charactersIn: " -"))
        return "\(cleaned.isEmpty ? "声记导出" : cleaned).md"
    }

    public static func format(detail: RecordingDetail, calendar: Calendar = .current) -> String {
        format(
            recording: detail.recording,
            draft: .realtimeRaw(segments: detail.segments),
            calendar: calendar
        )
    }

    public static func format(recording: Recording, draft: TranscriptDraft, calendar: Calendar = .current) -> String {
        var lines: [String] = [
            "# \(escapeHeading(recording.title))",
            "",
            "- 创建时间：\(formatDate(recording.createdAt, calendar: calendar))",
            "- 时长：\(formatDuration(recording.durationMilliseconds))",
            "- 语言：\(recording.languageMode.displayName)",
            "- 模型：\(recording.modelName.rawValue)",
            "- 稿件：\(draft.kind.displayName)",
            "",
            "## 正文"
        ]

        for segment in draft.segments.sorted(by: { $0.segmentIndex < $1.segmentIndex }) {
            lines.append("")
            lines.append("### \(formatTimecode(segment.startMilliseconds))-\(formatTimecode(segment.endMilliseconds))")
            lines.append("")
            lines.append(contentsOf: fencedCodeBlock(text(for: segment)))
        }

        return lines.joined(separator: "\n")
    }

    private static func text(for segment: TranscriptDraftSegment) -> String {
        guard !segment.isFailed else {
            let message = segment.errorMessage?.trimmingCharacters(in: .whitespacesAndNewlines)
            return "转写失败：\(message?.isEmpty == false ? message! : "未知错误")"
        }
        return segment.text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func escapeHeading(_ text: String) -> String {
        let collapsed = text
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\r", with: "\n")
            .split(whereSeparator: { $0.isWhitespace })
            .joined(separator: " ")
        let htmlEscaped = collapsed
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
        var escaped = ""
        for character in htmlEscaped {
            if ["\\", "`", "*", "_", "[", "]", "(", ")", "#", "+", "!", "|"].contains(character) {
                escaped.append("\\")
            }
            escaped.append(character)
        }
        return escaped.isEmpty ? "未命名记录" : escaped
    }

    private static func fencedCodeBlock(_ text: String) -> [String] {
        let normalized = text
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\r", with: "\n")
        let fence = String(repeating: "`", count: max(3, longestBacktickRun(in: normalized) + 1))
        return ["\(fence)text"] + normalized.components(separatedBy: "\n") + [fence]
    }

    private static func longestBacktickRun(in text: String) -> Int {
        var current = 0
        var longest = 0
        for character in text {
            if character == "`" {
                current += 1
                longest = max(longest, current)
            } else {
                current = 0
            }
        }
        return longest
    }

    private static func formatDate(_ date: Date, calendar: Calendar) -> String {
        let formatter = DateFormatter()
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd HH:mm"
        return formatter.string(from: date)
    }

    private static func formatDuration(_ milliseconds: Int) -> String {
        let seconds = max(milliseconds, 0) / 1000
        return "\(seconds / 60):\(String(format: "%02d", seconds % 60))"
    }

    private static func formatTimecode(_ milliseconds: Int) -> String {
        let seconds = max(milliseconds, 0) / 1000
        return "\(String(format: "%02d", seconds / 60)):\(String(format: "%02d", seconds % 60))"
    }
}
