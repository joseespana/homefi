import Foundation

// How the mascot dances: from the genre tag, then words in the title/album,
// then what the music sounds like (tempo, loudness). BOP is the standard nod.
// Port of the Android `ui/player/Mood.kt` and the desktop's `lib/mood.ts`: keep all in sync.

enum Mood: String, CaseIterable, Identifiable {
    case bop = "BOP"
    case mosh = "MOSH"
    case pogo = "POGO"
    case cute = "CUTE"
    case party = "PARTY"
    case salsa = "SALSA"
    case groove = "GROOVE"
    case chill = "CHILL"

    var id: String { rawValue }

    var label: String {
        switch self {
        case .bop: String(localized: "mood.bop", defaultValue: "Bop")
        case .mosh: String(localized: "mood.mosh", defaultValue: "Mosh")
        case .pogo: String(localized: "mood.pogo", defaultValue: "Pogo")
        case .cute: String(localized: "mood.cute", defaultValue: "Cute")
        case .party: String(localized: "mood.party", defaultValue: "Party")
        case .salsa: String(localized: "mood.salsa", defaultValue: "Salsa")
        case .groove: String(localized: "mood.groove", defaultValue: "Groove")
        case .chill: String(localized: "mood.chill", defaultValue: "Chill")
        }
    }

    /** The genres that pick this mood (shown as a hint in Settings). */
    var genres: String {
        switch self {
        case .bop: String(localized: "mood.bop.hint", defaultValue: "Standard nod on the beat")
        case .mosh: String(localized: "mood.mosh.hint", defaultValue: "Rock, metal, nu metal, hardcore")
        case .pogo: String(localized: "mood.pogo.hint", defaultValue: "Punk, ska, pop punk")
        case .cute: String(localized: "mood.cute.hint", defaultValue: "Pop, K-pop, indie pop")
        case .party: String(localized: "mood.party.hint", defaultValue: "Electronic, house, techno, EDM, disco")
        case .salsa: String(localized: "mood.salsa.hint", defaultValue: "Salsa, bachata, cumbia, merengue, latin")
        case .groove: String(localized: "mood.groove.hint", defaultValue: "Hip-hop, rap, R&B, funk, reggaeton")
        case .chill: String(localized: "mood.chill.hint", defaultValue: "Jazz, blues, classical, ambient, lo-fi")
        }
    }
}

/// What the music sounds like so far (Android's `player.Listening`): loudness
/// 0...1, tempo, and how many seconds have been heard.
struct ListeningHint {
    var energy: Double = 0
    var bpm: Double = 120
    var heard: Double = 0
}

private func rx(_ pattern: String) -> NSRegularExpression {
    // the patterns are constants: a typo is a programming error
    try! NSRegularExpression(pattern: pattern)
}

// first match wins, so specific words come before generic ones
private let rules: [(NSRegularExpression, Mood)] = [
    (rx("punk|ska|emo"), .pogo),
    (rx("metal|rock|grunge|hardcore|core\\b|thrash|doom|alternative"), .mosh),
    (rx("salsa|bachata|cumbia|merengue|tropical|son cubano|mambo|latin(?!.*(pop|urban|trap))"), .salsa),
    (rx("hip.?hop|rap|trap|r&b|rnb|funk|soul|reggaeton|urbano|latin urban|dancehall|drill|grime"), .groove),
    (rx("electro|edm|house|techno|trance|dance|disco|dubstep|drum.?n.?bass|dnb|club|garage|synthwave"), .party),
    (rx("jazz|blues|classical|orchestra|piano|ambient|lo.?fi|chill|acoustic|folk|bossa|instrumental|soundtrack|new age|country"), .chill),
    (rx("pop|k-?pop|j-?pop|idol|indie"), .cute),
]

// words that describe a version of a song, whatever the genre
private let descriptionRules: [(NSRegularExpression, Mood)] = [
    (rx("\\b(remix|club mix|extended mix|edit|vip|bootleg)\\b"), .party),
    (rx("\\b(acoustic|unplugged|piano version|lullaby|orchestral|strings)\\b"), .chill),
]

private func matches(_ re: NSRegularExpression, _ s: String) -> Bool {
    re.firstMatch(in: s, range: NSRange(s.startIndex..., in: s)) != nil
}

func moodFor(genre: String?, description: String = "", listening: ListeningHint? = nil) -> Mood {
    let g = (genre ?? "").lowercased()
    for (re, mood) in rules where matches(re, g) { return mood }
    let d = description.lowercased()
    for (re, mood) in descriptionRules where matches(re, d) { return mood }
    if let sound = listening, sound.heard > 8 {
        if sound.bpm >= 118 && sound.energy > 0.42 { return .party }
        if sound.energy < 0.16 || (sound.bpm < 90 && sound.energy < 0.28) { return .chill }
    }
    return .bop
}
