import Foundation

/// Everything the app shares: the Rust core, settings, library, Tidal and the player.
@MainActor
@Observable
final class AppModel {
    let settings: SettingsStore
    let library: LibraryStore
    let tidal: TidalStore
    let player: PlayerController

    init() {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("homefi", isDirectory: true)
        // Credentials and the API cache live here (like ~/.homefi on a computer).
        let engine = try? HomefiEngine(appDir: support.path)
        settings = SettingsStore()
        library = LibraryStore(engine: engine)
        tidal = TidalStore(engine: engine)
        player = PlayerController(engine: engine)
        Task { await library.scan() }
    }
}

enum Folders {
    /// The app's Documents: what Finder and the Files app show as "HomeFi".
    static let music = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    /// Tidal songs fetched for streaming.
    static let streamCache = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("stream", isDirectory: true)
}
