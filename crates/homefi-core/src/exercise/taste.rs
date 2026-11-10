//! Artists to suggest for the setup's "artists you like": the ones the rider owns the most songs of in the
//! genres picked. Cheap (one pass over the library index the app already has); no history, no network.

use std::collections::HashMap;

use crate::playlist::artist_key;
use crate::text::fold;

/// Artists the setup suggests at most.
pub const MAX_ARTIST_SUGGESTIONS: usize = 6;
/// An artist needs this many songs in the library to be suggested (one stray song says little).
const MIN_SONGS_TO_SUGGEST: usize = 2;

/// The artists with the most library songs whose genre tag names one of `genres` (any genre when empty),
/// most songs first, then by name; each once, as the library spells it. `songs` are `(artist, genre tag)`
/// pairs, borrowed from whatever the app keeps its library in.
pub fn artist_suggestions<'a>(songs: impl IntoIterator<Item = (&'a str, &'a str)>, genres: &[String]) -> Vec<String> {
    let wanted: Vec<String> = genres.iter().map(|g| fold(g)).filter(|g| !g.is_empty()).collect();
    // artist key -> (songs, the name as first seen)
    let mut counts: HashMap<String, (usize, &str)> = HashMap::new();
    for (artist, genre) in songs {
        if !wanted.is_empty() {
            let tag = fold(genre);
            if !wanted.iter().any(|g| tag.contains(g.as_str())) {
                continue;
            }
        }
        let key = artist_key(artist);
        if key.is_empty() {
            continue;
        }
        counts.entry(key).or_insert((0, artist.trim())).0 += 1;
    }
    let mut ranked: Vec<(usize, &str)> = counts.into_values().filter(|(n, _)| *n >= MIN_SONGS_TO_SUGGEST).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    ranked.into_iter().take(MAX_ARTIST_SUGGESTIONS).map(|(_, name)| name.to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_owned_artists_of_the_genre_come_first() {
        let library = [
            ("Metallica", "Thrash Metal"),
            ("Metallica", "Metal"),
            ("metallica", "Heavy Metal"),
            ("Iron Maiden", "Heavy Metal"),
            ("Iron Maiden", "Metal"),
            ("Stray", "Metal"),
            ("Adele", "Pop"),
            ("Adele", "Pop"),
            ("", "Metal"),
        ];
        assert_eq!(artist_suggestions(library, &["metal".into()]), ["Metallica", "Iron Maiden"]);
        assert_eq!(artist_suggestions(library, &["pop".into()]), ["Adele"]);
        assert_eq!(artist_suggestions(library, &[]), ["Metallica", "Adele", "Iron Maiden"]);
        assert!(artist_suggestions([], &["metal".into()]).is_empty());
    }
}
