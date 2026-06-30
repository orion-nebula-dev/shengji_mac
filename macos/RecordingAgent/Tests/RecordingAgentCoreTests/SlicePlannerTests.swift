import XCTest
@testable import RecordingAgentCore

final class SlicePlannerTests: XCTestCase {
    func testPlansFixedFifteenSecondWindowsAndTailSlice() {
        let windows = AudioSlicePlanner.planWindows(durationMilliseconds: 31_200)

        XCTAssertEqual(windows.map(\.index), [0, 1, 2])
        XCTAssertEqual(windows.map(\.startMilliseconds), [0, 15_000, 30_000])
        XCTAssertEqual(windows.map(\.endMilliseconds), [15_000, 30_000, 31_200])
        XCTAssertEqual(windows.last?.durationMilliseconds, 1_200)
    }

    func testDoesNotCreateEmptySliceForZeroDuration() {
        XCTAssertEqual(AudioSlicePlanner.planWindows(durationMilliseconds: 0), [])
    }
}
