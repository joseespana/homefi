import SwiftUI

struct MiniPlayer: View {
    @Environment(PlayerController.self) private var player
    let open: () -> Void

    var body: some View {
        if let item = player.current {
            HStack(spacing: 12) {
                Cover(item: item, size: 44)
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                    Text(item.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer()
                if player.isLoading {
                    ProgressView()
                } else {
                    Button(player.isPlaying ? "Pause" : "Play", systemImage: player.isPlaying ? "pause.fill" : "play.fill") { player.toggle() }
                        .labelStyle(.iconOnly).font(.title3)
                }
                Button("Next", systemImage: "forward.fill") { player.next() }
                    .labelStyle(.iconOnly).font(.title3)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14))
            .padding(.horizontal, 8)
            .padding(.bottom, 4)
            .contentShape(Rectangle())
            .onTapGesture(perform: open)
        }
    }
}

/// The cover of what plays: embedded art for files, Tidal's for streams.
struct Cover: View {
    let item: PlayItem
    let size: CGFloat?

    var body: some View {
        Group {
            switch item.source {
            case .file(let path):
                if let size { LocalCover(path: path, size: size) } else { BigLocalCover(path: path) }
            case .tidal:
                AsyncImage(url: item.coverURL) { $0.resizable().scaledToFit() } placeholder: { Color.secondary.opacity(0.2) }
                    .frame(width: size, height: size)
                    .clipShape(RoundedRectangle(cornerRadius: size == nil ? 18 : 6))
            }
        }
    }
}

private struct BigLocalCover: View {
    let path: String
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 18).fill(.quaternary)
            if let image { Image(uiImage: image).resizable().scaledToFit() } else { Image(systemName: "music.note").font(.system(size: 80)).foregroundStyle(.secondary) }
        }
        .aspectRatio(1, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: 18))
        .task(id: path) {
            let p = path
            image = await Task.detached { Covers.local(p) }.value
        }
    }
}

struct NowPlayingView: View {
    @Environment(PlayerController.self) private var player
    @Environment(SettingsStore.self) private var settings
    @Environment(\.dismiss) private var dismiss
    @State private var scrubbing: Double?

    var body: some View {
        @Bindable var settings = settings
        VStack(spacing: 18) {
            Capsule().fill(.secondary).frame(width: 40, height: 5).padding(.top, 8)
            if let item = player.current {
                TabView(selection: $settings.stage) {
                    Cover(item: item, size: nil).padding(.horizontal, 24).tag(Stage.cover)
                    MascotStage(item: item).tag(Stage.mascot)
                }
                .tabViewStyle(.page(indexDisplayMode: .always))
                .indexViewStyle(.page(backgroundDisplayMode: .always))

                VStack(spacing: 4) {
                    Text(item.title).font(.title2.bold()).lineLimit(1)
                    Text(item.artist).font(.title3).foregroundStyle(.secondary).lineLimit(1)
                    if let q = item.quality, !q.isEmpty {
                        Text(q).font(.caption.weight(.semibold)).padding(.horizontal, 8).padding(.vertical, 3)
                            .background(.green.opacity(0.15), in: Capsule()).foregroundStyle(.green)
                    }
                }
                .padding(.horizontal)

                VStack(spacing: 2) {
                    Slider(value: Binding(get: { scrubbing ?? player.positionMs }, set: { scrubbing = $0 }),
                           in: 0...max(player.durationMs, 1)) { editing in
                        if !editing, let s = scrubbing { player.seek(to: s); scrubbing = nil }
                    }
                    HStack {
                        Text(clock(scrubbing ?? player.positionMs))
                        Spacer()
                        Text(clock(player.durationMs))
                    }
                    .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                }
                .padding(.horizontal, 24)

                HStack(spacing: 44) {
                    Button("Previous", systemImage: "backward.fill") { player.previous() }
                    Button(player.isPlaying ? "Pause" : "Play", systemImage: player.isPlaying ? "pause.circle.fill" : "play.circle.fill") { player.toggle() }
                        .font(.system(size: 72))
                    Button("Next", systemImage: "forward.fill") { player.next() }
                }
                .labelStyle(.iconOnly)
                .font(.largeTitle)
                .padding(.bottom, 24)
            } else {
                Spacer()
                Text("Nothing playing").foregroundStyle(.secondary)
                Spacer()
            }
        }
    }

    private func clock(_ ms: Double) -> String {
        let s = Int(ms / 1000)
        return String(format: "%d:%02d", s / 60, s % 60)
    }
}

/// The dancing mascot as the player's stage, with the user's mascot settings.
struct MascotStage: View {
    @Environment(PlayerController.self) private var player
    @Environment(SettingsStore.self) private var settings
    let item: PlayItem

    var body: some View {
        @Bindable var settings = settings
        // No audio analysis on iPhone yet: a steady 120 BPM and medium energy.
        MascotView(
            playing: player.isPlaying,
            mood: settings.mascotMood ?? moodFor(genre: item.genre, description: "\(item.title) \(item.album)"),
            bpm: 120,
            energy: 0.5,
            dance: settings.mascotDance,
            specials: settings.mascotSpecials,
            turns: settings.mascotTurns,
            limbs: settings.mascotLimbs,
            accent: .homefiAccent,
            offset: $settings.mascotOffset,
            scale: $settings.mascotScale
        )
        // the house is white: a soft shadow (like the desktop's) keeps it visible on light backgrounds
        .shadow(color: .black.opacity(0.3), radius: 14, y: 10)
        .padding(.horizontal, 12)
    }
}
