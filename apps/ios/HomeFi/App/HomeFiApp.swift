import SwiftUI

@main
struct HomeFiApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .environment(model.settings)
                .environment(model.player)
                .environment(model.library)
                .environment(model.tidal)
                .preferredColorScheme(model.settings.theme.colorScheme)
                .tint(.homefiAccent)
        }
    }
}

extension Color {
    /// The brand teal, same as the Android app's primary colour.
    static let homefiAccent = Color(red: 0x13 / 255, green: 0x7A / 255, blue: 0x92 / 255)
}
