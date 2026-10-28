import SwiftUI

struct SettingsView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(TidalStore.self) private var tidal

    var body: some View {
        @Bindable var settings = settings
        NavigationStack {
            Form {
                Section("Appearance") {
                    Picker("Theme", selection: $settings.theme) {
                        ForEach(ThemeMode.allCases) { Text($0.label).tag($0) }
                    }
                }

                Section {
                    Picker("Style", selection: $settings.mascotMood) {
                        Text("Auto (from the genre)").tag(Mood?.none)
                        ForEach(Mood.allCases) { Text($0.label).tag(Mood?.some($0)) }
                    }
                    Picker("Dance", selection: $settings.mascotDance) {
                        Text("Auto").tag(DanceId?.none)
                        ForEach(DanceId.allCases) { Text($0.label).tag(DanceId?.some($0)) }
                    }
                    if settings.mascotDance == nil {
                        Picker("Famous dances", selection: $settings.mascotSpecials) {
                            ForEach(Specials.allCases) { Text($0.label).tag($0) }
                        }
                    }
                    Picker("Turn sideways", selection: $settings.mascotTurns) {
                        ForEach(Turns.allCases) { Text($0.label).tag($0) }
                    }
                    Picker("Arms and legs", selection: $settings.mascotLimbs) {
                        ForEach(MascotLimbs.allCases) { Text($0.label).tag($0) }
                    }
                    if settings.mascotOffset != .zero || settings.mascotScale != 1 {
                        Button("Put the mascot back") {
                            settings.mascotOffset = .zero
                            settings.mascotScale = 1
                        }
                    }
                } header: {
                    Text("Mascot")
                } footer: {
                    if let d = settings.mascotDance { Text(d.hint) }
                }

                Section("Tidal") {
                    if let account = tidal.account {
                        LabeledContent("Account", value: account.countryCode.isEmpty ? account.userId : "\(account.userId) · \(account.countryCode)")
                        Button("Sign out", role: .destructive) { Task { await tidal.logout() } }
                    } else {
                        Text("Not signed in").foregroundStyle(.secondary)
                    }
                }

                Section("About") {
                    LabeledContent("HomeFi", value: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "")
                    LabeledContent("Core", value: coreVersion())
                }
            }
            .navigationTitle("Settings")
        }
    }
}
