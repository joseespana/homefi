package com.perseusoft.homefi.ui.player

import com.perseusoft.homefi.R
import com.perseusoft.homefi.player.Listening

// How the mascot dances: from the genre tag, then words in the title/album,
// then what the music sounds like (tempo, loudness). BOP is the standard nod.
// Port of the desktop's `lib/mood.ts`: keep both in sync.

enum class Mood(val label: Int, val genres: Int) {
    BOP(R.string.mood_bop, R.string.mood_bop_hint),
    MOSH(R.string.mood_mosh, R.string.mood_mosh_hint),
    POGO(R.string.mood_pogo, R.string.mood_pogo_hint),
    CUTE(R.string.mood_cute, R.string.mood_cute_hint),
    PARTY(R.string.mood_party, R.string.mood_party_hint),
    SALSA(R.string.mood_salsa, R.string.mood_salsa_hint),
    GROOVE(R.string.mood_groove, R.string.mood_groove_hint),
    CHILL(R.string.mood_chill, R.string.mood_chill_hint),
}

// first match wins, so specific words come before generic ones
private val RULES: List<Pair<Regex, Mood>> = listOf(
    Regex("punk|ska|emo") to Mood.POGO,
    Regex("metal|rock|grunge|hardcore|core\\b|thrash|doom|alternative") to Mood.MOSH,
    Regex("salsa|bachata|cumbia|merengue|tropical|son cubano|mambo|latin(?!.*(pop|urban|trap))") to Mood.SALSA,
    Regex("hip.?hop|rap|trap|r&b|rnb|funk|soul|reggaeton|urbano|latin urban|dancehall|drill|grime") to Mood.GROOVE,
    Regex("electro|edm|house|techno|trance|dance|disco|dubstep|drum.?n.?bass|dnb|club|garage|synthwave") to Mood.PARTY,
    Regex("jazz|blues|classical|orchestra|piano|ambient|lo.?fi|chill|acoustic|folk|bossa|instrumental|soundtrack|new age|country") to Mood.CHILL,
    Regex("pop|k-?pop|j-?pop|idol|indie") to Mood.CUTE,
)

// words that describe a version of a song, whatever the genre
private val DESCRIPTION: List<Pair<Regex, Mood>> = listOf(
    Regex("\\b(remix|club mix|extended mix|edit|vip|bootleg)\\b") to Mood.PARTY,
    Regex("\\b(acoustic|unplugged|piano version|lullaby|orchestral|strings)\\b") to Mood.CHILL,
)

fun moodFor(genre: String?, description: String = "", sound: Listening? = null): Mood {
    val g = genre.orEmpty().lowercase()
    for ((re, mood) in RULES) if (re.containsMatchIn(g)) return mood
    val d = description.lowercase()
    for ((re, mood) in DESCRIPTION) if (re.containsMatchIn(d)) return mood
    if (sound != null && sound.heard > 8) {
        if (sound.bpm >= 118 && sound.energy > 0.42f) return Mood.PARTY
        if (sound.energy < 0.16f || (sound.bpm < 90 && sound.energy < 0.28f)) return Mood.CHILL
    }
    return Mood.BOP
}
