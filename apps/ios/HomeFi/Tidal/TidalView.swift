import SwiftUI

struct TidalView: View {
    @Environment(TidalStore.self) private var tidal
    @Environment(PlayerController.self) private var player

    var body: some View {
        NavigationStack {
            Group {
                if tidal.account == nil {
                    LoginPanel()
                } else if tidal.favorites.isEmpty && tidal.loading {
                    ProgressView()
                } else {
                    let items = tidal.favorites.map(PlayItem.init)
                    List(Array(tidal.favorites.enumerated()), id: \.element.id) { i, hit in
                        HitRow(hit: hit).contentShape(Rectangle())
                            .onTapGesture { player.play(items, at: i) }
                            .contextMenu { QueueButtons(items: [items[i]]) }
                    }
                    .listStyle(.plain)
                    .refreshable { await tidal.loadFavorites() }
                }
            }
            .navigationTitle("My Tidal")
            .task { if tidal.favorites.isEmpty { await tidal.loadFavorites() } }
        }
    }
}

private struct LoginPanel: View {
    @Environment(TidalStore.self) private var tidal

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "waveform.circle").font(.system(size: 64)).foregroundStyle(.tint)
            if let login = tidal.login {
                Text("Approve HomeFi on tidal.com with this code:")
                Text(login.userCode).font(.system(.largeTitle, design: .monospaced)).bold().textSelection(.enabled)
                ProgressView()
                Button("Cancel", role: .cancel) { tidal.cancelLogin() }
            } else {
                Text("Sign in to stream and search Tidal").multilineTextAlignment(.center)
                Button("Sign in with Tidal") { Task { await tidal.startLogin() } }
                    .buttonStyle(.borderedProminent)
            }
            if let error = tidal.error { Text(error).font(.footnote).foregroundStyle(.red) }
        }
        .padding()
    }
}

struct HitRow: View {
    let hit: SearchHit

    var body: some View {
        HStack(spacing: 12) {
            AsyncImage(url: hit.coverUrl.flatMap(URL.init(string:))) { image in
                image.resizable().scaledToFill()
            } placeholder: {
                Image(systemName: "music.note").foregroundStyle(.secondary)
            }
            .frame(width: 44, height: 44)
            .background(.quaternary)
            .clipShape(RoundedRectangle(cornerRadius: 6))
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    Text(hit.title).lineLimit(1)
                    if hit.explicit { Image(systemName: "e.square.fill").font(.caption).foregroundStyle(.secondary) }
                }
                Text(hit.subtitle).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
    }
}

struct SearchView: View {
    @Environment(TidalStore.self) private var tidal
    @Environment(LibraryStore.self) private var library
    @Environment(PlayerController.self) private var player
    @State private var query = ""
    @State private var results: SearchResults?
    @State private var failure: String?

    private var localMatches: [LibraryTrack] {
        let q = query.trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return [] }
        return library.tracks.filter {
            $0.title.localizedCaseInsensitiveContains(q) || $0.artist.localizedCaseInsensitiveContains(q) || $0.album.localizedCaseInsensitiveContains(q)
        }
    }

    var body: some View {
        NavigationStack {
            List {
                let local = localMatches
                if !local.isEmpty {
                    Section("On this iPhone") {
                        let items = local.map(PlayItem.init)
                        ForEach(Array(local.prefix(20).enumerated()), id: \.element.path) { i, t in
                            TrackRow(track: t).contentShape(Rectangle()).onTapGesture { player.play(items, at: i) }
                        }
                    }
                }
                if let tracks = results?.tracks, !tracks.isEmpty {
                    Section("Tidal") {
                        let items = tracks.map(PlayItem.init)
                        ForEach(Array(tracks.enumerated()), id: \.element.id) { i, hit in
                            HitRow(hit: hit).contentShape(Rectangle())
                                .onTapGesture { player.play(items, at: i) }
                                .contextMenu { QueueButtons(items: [items[i]]) }
                        }
                    }
                }
                if let failure { Text(failure).foregroundStyle(.red) }
            }
            .listStyle(.plain)
            .navigationTitle("Search")
            .searchable(text: $query, prompt: "Songs, artists, albums")
            .task(id: query) {
                // wait for the user to stop typing
                try? await Task.sleep(for: .milliseconds(400))
                guard !Task.isCancelled else { return }
                do {
                    results = try await tidal.search(query)
                    failure = nil
                } catch {
                    failure = error.localizedDescription
                }
            }
        }
    }
}
