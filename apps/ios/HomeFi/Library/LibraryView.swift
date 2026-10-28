import SwiftUI

struct LibraryView: View {
    @Environment(LibraryStore.self) private var library
    @Environment(PlayerController.self) private var player
    @State private var section = Section.songs

    enum Section: CaseIterable { case songs, albums, artists }

    var body: some View {
        NavigationStack {
            Group {
                if library.tracks.isEmpty {
                    if library.scanning { ProgressView() } else { EmptyLibrary() }
                } else {
                    list
                }
            }
            .navigationTitle("Library")
            .toolbar {
                if !library.tracks.isEmpty {
                    Button("Shuffle", systemImage: "shuffle") {
                        player.play(library.tracks.map(PlayItem.init), shuffle: true)
                    }
                }
            }
            .refreshable { await library.scan() }
        }
    }

    private var list: some View {
        List {
            Picker("Show", selection: $section) {
                Text("Songs").tag(Section.songs)
                Text("Albums").tag(Section.albums)
                Text("Artists").tag(Section.artists)
            }
            .pickerStyle(.segmented)
            .listRowSeparator(.hidden)

            switch section {
            case .songs:
                let items = library.tracks.map(PlayItem.init)
                ForEach(Array(library.tracks.enumerated()), id: \.element.path) { i, t in
                    TrackRow(track: t).contentShape(Rectangle())
                        .onTapGesture { player.play(items, at: i) }
                        .contextMenu { QueueButtons(items: [items[i]]) }
                }
            case .albums:
                ForEach(library.albums) { album in
                    NavigationLink {
                        TrackList(title: album.title, tracks: album.tracks)
                    } label: {
                        HStack(spacing: 12) {
                            LocalCover(path: album.tracks[0].path, size: 52)
                            VStack(alignment: .leading) {
                                Text(album.title).lineLimit(1)
                                Text(album.artist).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                            }
                        }
                    }
                }
            case .artists:
                ForEach(library.artists, id: \.name) { artist in
                    NavigationLink(artist.name) { TrackList(title: artist.name, tracks: artist.tracks) }
                }
            }
        }
        .listStyle(.plain)
    }
}

/// Songs of an album or artist.
struct TrackList: View {
    @Environment(PlayerController.self) private var player
    let title: String
    let tracks: [LibraryTrack]

    var body: some View {
        let items = tracks.map(PlayItem.init)
        List(Array(tracks.enumerated()), id: \.element.path) { i, t in
            TrackRow(track: t).contentShape(Rectangle())
                .onTapGesture { player.play(items, at: i) }
                .contextMenu { QueueButtons(items: [items[i]]) }
        }
        .listStyle(.plain)
        .navigationTitle(title)
        .toolbar {
            Button("Play", systemImage: "play.fill") { player.play(items) }
        }
    }
}

struct QueueButtons: View {
    @Environment(PlayerController.self) private var player
    let items: [PlayItem]

    var body: some View {
        Button("Play next", systemImage: "text.line.first.and.arrowtriangle.forward") { player.enqueue(items, next: true) }
        Button("Add to queue", systemImage: "text.append") { player.enqueue(items, next: false) }
    }
}

struct TrackRow: View {
    let track: LibraryTrack

    var body: some View {
        HStack(spacing: 12) {
            LocalCover(path: track.path, size: 44)
            VStack(alignment: .leading, spacing: 2) {
                Text(track.title.isEmpty ? URL(fileURLWithPath: track.path).lastPathComponent : track.title).lineLimit(1)
                Text("\(track.artist) · \(track.quality)").font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
    }
}

struct LocalCover: View {
    let path: String
    let size: CGFloat
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 6).fill(.quaternary)
            if let image {
                Image(uiImage: image).resizable().scaledToFill()
            } else {
                Image(systemName: "music.note").foregroundStyle(.secondary)
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .task(id: path) {
            let p = path
            image = await Task.detached { Covers.local(p) }.value
        }
    }
}

private struct EmptyLibrary: View {
    var body: some View {
        ContentUnavailableView {
            Label("No music yet", systemImage: "music.note.list")
        } description: {
            Text("Connect the iPhone to your Mac, open Finder → iPhone → Files → HomeFi and drag your music there (or use the Files app). Then pull down to refresh.")
        }
    }
}
