import SwiftUI

struct RootView: View {
    @Environment(PlayerController.self) private var player
    @Environment(LibraryStore.self) private var library
    @Environment(TidalStore.self) private var tidal
    @Environment(\.scenePhase) private var scenePhase
    @State private var showPlayer = false

    var body: some View {
        TabView {
            LibraryView()
                .miniPlayer { showPlayer = true }
                .tabItem { Label("Library", systemImage: "music.note.house") }
            SearchView()
                .miniPlayer { showPlayer = true }
                .tabItem { Label("Search", systemImage: "magnifyingglass") }
            TidalView()
                .miniPlayer { showPlayer = true }
                .tabItem { Label("Tidal", systemImage: "waveform") }
            SettingsView()
                .tabItem { Label("Settings", systemImage: "gearshape") }
        }
        // back from signing in with Safari: check the Tidal login right away
        .onChange(of: scenePhase) { _, phase in if phase == .active { tidal.checkNow() } }
        .sheet(isPresented: $showPlayer) {
            NowPlayingView()
        }
        #if DEBUG
        // `-HomeFiDemo`: plays the first song and opens the player (simulator screenshots)
        .task {
            guard ProcessInfo.processInfo.arguments.contains("-HomeFiDemo") else { return }
            while library.tracks.isEmpty { try? await Task.sleep(for: .milliseconds(200)) }
            player.play(library.tracks.map(PlayItem.init))
            showPlayer = true
        }
        #endif
        .alert("Could not play", isPresented: Binding(get: { player.error != nil }, set: { if !$0 { player.error = nil } })) {
            Button("OK") { player.error = nil }
        } message: {
            Text(player.error ?? "")
        }
    }
}

private struct MiniPlayerInset: ViewModifier {
    @Environment(PlayerController.self) private var player
    let open: () -> Void

    func body(content: Content) -> some View {
        content.safeAreaInset(edge: .bottom, spacing: 0) {
            if player.current != nil { MiniPlayer(open: open) }
        }
    }
}

extension View {
    /// The "now playing" bar above the tab bar; tapping it opens the player.
    func miniPlayer(open: @escaping () -> Void) -> some View {
        modifier(MiniPlayerInset(open: open))
    }
}
