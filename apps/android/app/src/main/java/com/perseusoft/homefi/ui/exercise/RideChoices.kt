package com.perseusoft.homefi.ui.exercise

import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.Popularity

/** Genres of the setup chips: the core and Tidal search by the token (English), the chip shows the label. */
internal enum class Genre(val token: String, val label: Int) {
    ROCK("rock", R.string.ex_genre_rock),
    METAL("metal", R.string.ex_genre_metal),
    POP("pop", R.string.ex_genre_pop),
    DANCE("dance", R.string.ex_genre_dance),
    HIP_HOP("hip-hop", R.string.ex_genre_hip_hop),
    LATIN("latin", R.string.ex_genre_latin),
    FUNK("funk", R.string.ex_genre_funk),
    EIGHTIES("80s", R.string.ex_genre_80s),
    INDIE("indie", R.string.ex_genre_indie),
    PUNK("punk", R.string.ex_genre_punk);

    companion object {
        fun ofToken(token: String): Genre? = entries.firstOrNull { it.token == token }
    }
}

internal fun Popularity.labelRes(): Int = when (this) {
    Popularity.HITS -> R.string.ex_pop_hits
    Popularity.MIX -> R.string.ex_pop_mix
    Popularity.DISCOVER -> R.string.ex_pop_discover
}

/** First year of the decades written in full ("2000s"): "00s" or "20s" would be unclear. */
private const val FULL_DECADE_FROM = 2000
private const val YEARS_PER_CENTURY = 100

/** "80s", "2000s". */
internal fun decadeLabel(decade: UShort): String {
    val year = decade.toInt()
    return if (year < FULL_DECADE_FROM) "${year % YEARS_PER_CENTURY}s" else "${year}s"
}

/** [genres] with [token] switched on or off (the phone's and the TV's genre chips). */
internal fun toggleGenre(genres: Set<String>, token: String): Set<String> =
    if (token in genres) genres - token else genres + token

/** [eras] with [decade] switched on or off, kept in order; empty means any era. */
internal fun toggleDecade(eras: List<UShort>, decade: UShort): List<UShort> =
    if (decade in eras) eras - decade else (eras + decade).sorted()
