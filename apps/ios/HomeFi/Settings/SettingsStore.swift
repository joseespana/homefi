import SwiftUI

enum ThemeMode: String, CaseIterable, Identifiable {
    case system = "SYSTEM", light = "LIGHT", dark = "DARK"
    var id: String { rawValue }
    var label: String {
        switch self {
        case .system: String(localized: "System")
        case .light: String(localized: "Light")
        case .dark: String(localized: "Dark")
        }
    }
    var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}

/// What the player shows in its big area (same order as Android's stage).
enum Stage: Int, CaseIterable, Identifiable {
    case cover, mascot
    var id: Int { rawValue }
}

/// Every preference, saved in UserDefaults under the same keys the Android app uses.
@MainActor
@Observable
final class SettingsStore {
    private let defaults = UserDefaults.standard

    var theme: ThemeMode { didSet { save("theme", theme.rawValue) } }
    var stage: Stage { didSet { defaults.set(stage.rawValue, forKey: "stage") } }
    /// nil follows the genre of each song.
    var mascotMood: Mood? { didSet { save("mascot_mood", mascotMood?.rawValue) } }
    /// nil: the genre's groove with famous dances now and then.
    var mascotDance: DanceId? { didSet { save("mascot_dance", mascotDance?.rawValue) } }
    var mascotSpecials: Specials { didSet { save("mascot_specials", mascotSpecials.rawValue) } }
    var mascotTurns: Turns { didSet { save("mascot_turns", mascotTurns.rawValue) } }
    var mascotLimbs: MascotLimbs { didSet { save("mascot_limbs", mascotLimbs.rawValue) } }
    /// Where the user dragged the mascot (fraction of its stage) and its zoom.
    var mascotOffset: CGSize {
        didSet {
            defaults.set(Double(mascotOffset.width), forKey: "mascot_x")
            defaults.set(Double(mascotOffset.height), forKey: "mascot_y")
        }
    }
    var mascotScale: CGFloat { didSet { defaults.set(Double(mascotScale), forKey: "mascot_scale") } }

    init() {
        let d = UserDefaults.standard
        func read<T: RawRepresentable>(_ key: String, _ fallback: T) -> T where T.RawValue == String {
            d.string(forKey: key).flatMap(T.init(rawValue:)) ?? fallback
        }
        theme = read("theme", ThemeMode.system)
        stage = Stage(rawValue: d.integer(forKey: "stage")) ?? .cover
        mascotMood = d.string(forKey: "mascot_mood").flatMap(Mood.init(rawValue:))
        mascotDance = d.string(forKey: "mascot_dance").flatMap(DanceId.init(rawValue:))
        mascotSpecials = read("mascot_specials", Specials.sometimes)
        mascotTurns = read("mascot_turns", Turns.all)
        mascotLimbs = read("mascot_limbs", MascotLimbs.white)
        mascotOffset = CGSize(width: d.double(forKey: "mascot_x"), height: d.double(forKey: "mascot_y"))
        mascotScale = d.object(forKey: "mascot_scale") == nil ? 1 : CGFloat(d.double(forKey: "mascot_scale"))
    }

    private func save(_ key: String, _ value: String?) {
        if let value { defaults.set(value, forKey: key) } else { defaults.removeObject(forKey: key) }
    }
}
