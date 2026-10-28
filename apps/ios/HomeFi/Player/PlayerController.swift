import AVFoundation
import MediaPlayer
import UIKit

/// Something the player can play: a file of the library or a Tidal track
/// (fetched to a local file by the core right before it plays).
struct PlayItem: Identifiable, Equatable {
    enum Source: Equatable {
        case file(String)
        case tidal(String)
    }

    let id = UUID()
    let source: Source
    let title: String
    let artist: String
    let album: String
    let genre: String
    let durationMs: UInt64?
    /// `FLAC · 24-bit / 96 kHz`, `Lossless`, ...
    let quality: String?
    let coverURL: URL?

    static func == (a: PlayItem, b: PlayItem) -> Bool { a.id == b.id }
}

extension PlayItem {
    init(_ t: LibraryTrack) {
        self.init(source: .file(t.path), title: t.title, artist: t.artist, album: t.album, genre: t.genre,
                  durationMs: t.durationMs, quality: t.quality, coverURL: nil)
    }

    init(_ h: SearchHit) {
        self.init(source: .tidal(h.id), title: h.title, artist: h.artist, album: h.album, genre: "",
                  durationMs: h.durationMs, quality: h.quality, coverURL: h.coverUrl.flatMap(URL.init(string:)))
    }
}

/// The queue and AVPlayer, plus the lock screen / Control Center controls.
@MainActor
@Observable
final class PlayerController {
    private(set) var queue: [PlayItem] = []
    private(set) var index = -1
    private(set) var isPlaying = false
    private(set) var isLoading = false
    private(set) var positionMs: Double = 0
    private(set) var durationMs: Double = 0
    var error: String?

    var current: PlayItem? { queue.indices.contains(index) ? queue[index] : nil }

    @ObservationIgnored private let engine: HomefiEngine?
    @ObservationIgnored private let player = AVPlayer()
    @ObservationIgnored private var timeObserver: Any?
    @ObservationIgnored private var endObserver: NSObjectProtocol?
    @ObservationIgnored private var loadTask: Task<Void, Never>?

    init(engine: HomefiEngine?) {
        self.engine = engine
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
        timeObserver = player.addPeriodicTimeObserver(forInterval: CMTime(value: 1, timescale: 4), queue: .main) { [weak self] time in
            MainActor.assumeIsolated { self?.tick(time) }
        }
        endObserver = NotificationCenter.default.addObserver(forName: AVPlayerItem.didPlayToEndTimeNotification, object: nil, queue: .main) { [weak self] note in
            MainActor.assumeIsolated {
                guard let self, (note.object as? AVPlayerItem) === self.player.currentItem else { return }
                self.next()
            }
        }
        setUpRemoteCommands()
    }

    // MARK: - queue

    func play(_ items: [PlayItem], at start: Int = 0, shuffle: Bool = false) {
        guard !items.isEmpty else { return }
        queue = shuffle ? items.shuffled() : items
        load(shuffle ? 0 : min(max(start, 0), items.count - 1))
    }

    /// Adds after the current song (`next`) or at the end.
    func enqueue(_ items: [PlayItem], next: Bool) {
        if current == nil { return play(items) }
        queue.insert(contentsOf: items, at: next ? index + 1 : queue.count)
    }

    func skip(to i: Int) {
        guard queue.indices.contains(i) else { return }
        load(i)
    }

    func next() {
        if index + 1 < queue.count { load(index + 1) } else { player.pause(); player.seek(to: .zero); isPlaying = false; updateNowPlaying() }
    }

    func previous() {
        if positionMs > 3000 || index == 0 { seek(to: 0) } else { load(index - 1) }
    }

    func toggle() {
        guard current != nil else { return }
        if isPlaying { player.pause() } else { activate(); player.play() }
        isPlaying.toggle()
        updateNowPlaying()
    }

    func seek(to ms: Double) {
        player.seek(to: CMTime(seconds: ms / 1000, preferredTimescale: 1000))
        positionMs = ms
        updateNowPlaying()
    }

    // MARK: - loading

    private func load(_ i: Int) {
        index = i
        positionMs = 0
        durationMs = Double(queue[i].durationMs ?? 0)
        let item = queue[i]
        loadTask?.cancel()
        isLoading = true
        updateNowPlaying()
        loadTask = Task {
            do {
                let url = try await self.resolve(item)
                guard !Task.isCancelled, self.current == item else { return }
                self.player.replaceCurrentItem(with: AVPlayerItem(url: url))
                self.activate()
                self.player.play()
                self.isPlaying = true
            } catch {
                guard !Task.isCancelled else { return }
                self.error = "\(item.title): \(error.localizedDescription)"
                self.isPlaying = false
            }
            self.isLoading = false
            self.updateNowPlaying()
        }
    }

    private func resolve(_ item: PlayItem) async throws -> URL {
        switch item.source {
        case .file(let path):
            return URL(fileURLWithPath: path)
        case .tidal(let id):
            guard let engine else { throw PlayerError.noCore }
            try FileManager.default.createDirectory(at: Folders.streamCache, withIntermediateDirectories: true)
            let path = try await engine.fetchStream(trackId: id, cacheDir: Folders.streamCache.path, quality: nil)
            // keep the cache bounded (2 GB, like Android), never the song about to play
            _ = trimStreamCache(cacheDir: Folders.streamCache.path, maxBytes: 2 << 30, keep: [path])
            return URL(fileURLWithPath: path)
        }
    }

    private func activate() {
        try? AVAudioSession.sharedInstance().setActive(true)
    }

    private func tick(_ time: CMTime) {
        guard current != nil else { return }
        positionMs = max(0, time.seconds * 1000)
        if let d = player.currentItem?.duration.seconds, d.isFinite, d > 0 { durationMs = d * 1000 }
    }

    // MARK: - lock screen

    private func setUpRemoteCommands() {
        let c = MPRemoteCommandCenter.shared()
        c.playCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { if self?.isPlaying == false { self?.toggle() } }; return .success }
        c.pauseCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { if self?.isPlaying == true { self?.toggle() } }; return .success }
        c.togglePlayPauseCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { self?.toggle() }; return .success }
        c.nextTrackCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { self?.next() }; return .success }
        c.previousTrackCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { self?.previous() }; return .success }
        c.changePlaybackPositionCommand.addTarget { [weak self] e in
            guard let e = e as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            MainActor.assumeIsolated { self?.seek(to: e.positionTime * 1000) }
            return .success
        }
    }

    private func updateNowPlaying() {
        guard let item = current else {
            MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
            return
        }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: item.title,
            MPMediaItemPropertyArtist: item.artist,
            MPMediaItemPropertyAlbumTitle: item.album,
            MPMediaItemPropertyPlaybackDuration: durationMs / 1000,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: positionMs / 1000,
            MPNowPlayingInfoPropertyPlaybackRate: isPlaying ? 1.0 : 0.0,
        ]
        if case .file(let path) = item.source, let image = Covers.local(path) {
            info[MPMediaItemPropertyArtwork] = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
        }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    }
}

enum PlayerError: LocalizedError {
    case noCore
    var errorDescription: String? { String(localized: "The HomeFi core did not start") }
}

/// Embedded covers of local files, read once by the core and kept in memory.
enum Covers {
    private static let cache = NSCache<NSString, UIImage>()

    static func local(_ path: String) -> UIImage? {
        if let hit = cache.object(forKey: path as NSString) { return hit }
        guard let data = readCover(path: path), let image = UIImage(data: data) else { return nil }
        cache.setObject(image, forKey: path as NSString)
        return image
    }
}
