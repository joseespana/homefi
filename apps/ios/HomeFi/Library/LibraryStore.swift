import Foundation

/// The music in the app's Documents folder (copied in with Finder or the Files app).
@MainActor
@Observable
final class LibraryStore {
    private(set) var tracks: [LibraryTrack] = []
    private(set) var scanning = false

    @ObservationIgnored private let engine: HomefiEngine?

    init(engine: HomefiEngine?) {
        self.engine = engine
    }

    struct Album: Identifiable {
        var id: String { "\(artist)\u{1}\(title)" }
        let title: String
        let artist: String
        let tracks: [LibraryTrack]
    }

    var albums: [Album] {
        Dictionary(grouping: tracks) { "\($0.albumArtist.isEmpty ? $0.artist : $0.albumArtist)\u{1}\($0.album)" }
            .map { _, list in
                let first = list[0]
                return Album(title: first.album, artist: first.albumArtist.isEmpty ? first.artist : first.albumArtist,
                             tracks: list.sorted { ($0.discNumber, $0.trackNumber) < ($1.discNumber, $1.trackNumber) })
            }
            .sorted { $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending }
    }

    var artists: [(name: String, tracks: [LibraryTrack])] {
        Dictionary(grouping: tracks) { $0.albumArtist.isEmpty ? $0.artist : $0.albumArtist }
            .map { ($0.key, $0.value.sorted { ($0.album, $0.trackNumber) < ($1.album, $1.trackNumber) }) }
            .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    func scan() async {
        guard let engine, !scanning else { return }
        scanning = true
        let found = await engine.scanLibrary(roots: [Folders.music.path])
        tracks = found.sorted { $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending }
        scanning = false
    }
}
