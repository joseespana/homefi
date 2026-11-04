//! String keys shared by album grouping and the playlist resolver: two
//! spellings of the same name or title fold to the same key.

use unicode_normalization::UnicodeNormalization;

/// Words that make a trailing ` - <suffix>` a version of the song, not part of its name.
const VERSION_WORDS: [&str; 12] = [
    "live",
    "remaster",
    "remastered",
    "version",
    "edit",
    "mix",
    "demo",
    "acoustic",
    "mono",
    "stereo",
    "single",
    "radio",
];

/// Separator between a title and its version suffix ("Blind - Live").
const SUFFIX_SEPARATOR: &str = " - ";

/// Lower case, no accents, letters and digits only.
pub fn fold(s: &str) -> String {
    s.nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// `(head, inner)` of a title ending in `(...)` or `[...]`.
pub fn trailing_bracket(t: &str) -> Option<(&str, &str)> {
    let close = t.chars().last()?;
    let open = match close {
        ')' => '(',
        ']' => '[',
        _ => return None,
    };
    let mut depth = 0;
    for (i, c) in t.char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return Some((&t[..i], &t[i + 1..t.len() - 1]));
            }
        }
    }
    None
}

/// A song title's matching key: trailing `(...)` / `[...]` groups and a final
/// ` - <version words>` suffix removed, then [`fold`]. Falls back to
/// `fold(title)` when that leaves nothing.
pub fn song_key(title: &str) -> String {
    let mut base = title.trim();
    loop {
        if let Some((head, _)) = trailing_bracket(base) {
            base = head.trim_end();
            continue;
        }
        if let Some(i) = base.rfind(SUFFIX_SEPARATOR) {
            let suffix = &base[i + SUFFIX_SEPARATOR.len()..];
            let versionish = suffix
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty())
                .any(|w| VERSION_WORDS.contains(&w.to_lowercase().as_str()));
            if versionish {
                base = base[..i].trim_end();
                continue;
            }
        }
        break;
    }
    let key = fold(base);
    if key.is_empty() {
        fold(title)
    } else {
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_drops_accents_case_and_punctuation() {
        assert_eq!(fold("Beyoncé – Déjà Vu!"), "beyoncedejavu");
        assert_eq!(fold("Freak On A Leash"), "freakonaleash");
    }

    #[test]
    fn song_key_ignores_versions() {
        assert_eq!(song_key("Freak On A Leash"), "freakonaleash");
        assert_eq!(song_key("Blind (Remastered 2009)"), song_key("Blind"));
        assert_eq!(song_key("Blind - Live"), song_key("Blind"));
        assert_eq!(song_key("Blind [Live] (Remastered)"), song_key("Blind"));
        assert_eq!(song_key("Béyond"), song_key("beyond"));
    }

    #[test]
    fn song_key_keeps_real_subtitles() {
        assert_eq!(song_key("Part 1 - The Beginning"), "part1thebeginning");
    }

    #[test]
    fn song_key_falls_back_when_nothing_is_left() {
        assert_eq!(song_key("(Live)"), "live");
        assert_eq!(song_key("Live - Live"), "live");
    }
}
