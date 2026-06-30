import XCTest

final class RecordingAgentUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testLaunchShowsHomeRecordingControls() throws {
        let supportRoot = FileManager.default.temporaryDirectory
            .appending(path: "RecordingAgentUITests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: supportRoot, withIntermediateDirectories: true)
        addTeardownBlock {
            try? FileManager.default.removeItem(at: supportRoot)
        }

        let app = XCUIApplication()
        app.launchEnvironment["RECORDING_AGENT_APP_SUPPORT_ROOT"] = supportRoot.path
        app.launch()
        addTeardownBlock {
            app.terminate()
        }

        XCTAssertTrue(app.descendants(matching: .any)["home-title"].waitForExistence(timeout: 8))
        XCTAssertTrue(app.descendants(matching: .any)["home-recording-toggle-button"].exists)
        XCTAssertTrue(app.descendants(matching: .any)["home-model-picker"].exists)
        XCTAssertTrue(app.descendants(matching: .any)["home-language-picker"].exists)
        XCTAssertTrue(app.descendants(matching: .any)["model-preparation-row"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["home-confirmed-segments-title"].exists)
    }
}
