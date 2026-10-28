import Foundation
import UIKit

/// The Tidal session: device login, favorites and search, all through the core.
@MainActor
@Observable
final class TidalStore {
    private(set) var account: Account?
    private(set) var login: DeviceLogin?
    private(set) var favorites: [SearchHit] = []
    private(set) var loading = false
    var error: String?

    @ObservationIgnored private let engine: HomefiEngine?
    /// Set when the app comes back from Safari, to check the login right away.
    @ObservationIgnored private var wake: CheckedContinuation<Void, Never>?

    init(engine: HomefiEngine?) {
        self.engine = engine
        account = engine?.account()
    }

    /// Opens tidal.com in Safari with the code and waits for the user to approve it.
    func startLogin() async {
        guard let engine else { return }
        do {
            let d = try await engine.startLogin()
            login = d
            if let url = URL(string: d.url.hasPrefix("http") ? d.url : "https://\(d.url)") {
                await UIApplication.shared.open(url)
            }
            let deadline = Date().addingTimeInterval(TimeInterval(d.expiresInSecs))
            while login != nil, Date() < deadline {
                await pause(seconds: Double(d.intervalSecs))
                // a network hiccup (or a request iOS froze while Safari was in front) is not fatal
                guard let state = try? await engine.pollLogin(deviceCode: d.deviceCode) else { continue }
                switch state {
                case .success:
                    login = nil
                    account = engine.account()
                    await loadFavorites()
                    return
                case .expired:
                    login = nil
                case .pending:
                    continue
                }
            }
            login = nil
        } catch {
            login = nil
            self.error = error.localizedDescription
        }
    }

    func cancelLogin() {
        login = nil
        checkNow()
    }

    /// The app is in front again: look at the login now instead of waiting.
    func checkNow() {
        wake?.resume()
        wake = nil
    }

    private func pause(seconds: Double) async {
        await withCheckedContinuation { (c: CheckedContinuation<Void, Never>) in
            wake = c
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(seconds))
                checkNow()
            }
        }
    }

    func logout() async {
        try? await engine?.logout()
        account = nil
        favorites = []
    }

    func loadFavorites() async {
        guard let engine, account != nil else { return }
        loading = true
        do { favorites = try await engine.favoriteTracks() } catch { self.error = error.localizedDescription }
        loading = false
    }

    func search(_ q: String) async throws -> SearchResults? {
        guard let engine, account != nil, !q.trimmingCharacters(in: .whitespaces).isEmpty else { return nil }
        return try await engine.search(query: q)
    }
}
