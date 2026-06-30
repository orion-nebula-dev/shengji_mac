import SwiftUI
import RecordingAgentCore

@main
struct RecordingAgentNativeApp: App {
    @NSApplicationDelegateAdaptor(RecordingAgentApplicationDelegate.self) private var appDelegate

    var body: some Scene {
        Settings {
            SettingsView()
                .environmentObject(appDelegate.viewModel)
                .frame(width: 520)
                .padding(20)
        }
    }
}

@MainActor
final class RecordingAgentApplicationDelegate: NSObject, NSApplicationDelegate {
    let viewModel = RecordingAgentViewModel()
    private var mainWindow: NSWindow?
    private var didBootstrap = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        showMainWindow()
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag {
            showMainWindow()
        }
        return true
    }

    private func showMainWindow() {
        if mainWindow == nil {
            let content = RootView()
                .environmentObject(viewModel)
                .frame(minWidth: 1080, minHeight: 720)
            let hostingController = NSHostingController(rootView: content)
            let window = NSWindow(contentViewController: hostingController)
            window.title = "声记"
            window.setContentSize(NSSize(width: 1080, height: 720))
            window.minSize = NSSize(width: 1080, height: 720)
            window.isReleasedWhenClosed = false
            mainWindow = window
        }

        if !didBootstrap {
            didBootstrap = true
            viewModel.bootstrap()
        }

        mainWindow?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
}
