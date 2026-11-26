import SwiftUI

@main
struct FootballAppApp: App {
    @NSApplicationDelegateAdaptor(FootballAppWindowDelegate.self) private var windowDelegate

    static let uiWindowWidth: CGFloat? = {
#if DEBUG
        guard let raw = ProcessInfo.processInfo.environment["FOOTBALL_UI_WINDOW_WIDTH"],
              let width = Double(raw), width.isFinite, width > 0 else { return nil }
        return CGFloat(width)
#else
        return nil
#endif
    }()

    var body: some Scene {
        WindowGroup {
            HomeView()
        }
        .defaultSize(width: 1040, height: 680)
    }
}

private final class FootballAppWindowDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        guard let width = FootballAppApp.uiWindowWidth else { return }
        DispatchQueue.main.async {
            for window in NSApp.windows where window.isVisible {
                window.setContentSize(CGSize(width: width, height: 680))
            }
        }
    }
}
