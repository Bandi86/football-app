import SwiftUI

@main
struct FootballAppApp: App {
    @State private var model = DayModel()

    var body: some Scene {
        WindowGroup {
            DayView(model: model)
        }
    }
}
