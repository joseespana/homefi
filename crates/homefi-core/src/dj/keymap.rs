//! Optional keyboard shortcuts for DJ mode. Everything works with the mouse; these are extras
//! the user can rebind (`DjSettings::key_overrides`) or switch off (`keys_enabled`).
//!
//! An action id is `deck_a.play`, `sampler.3`, `crossfader.left`, ... A key is written
//! `[Alt+][Ctrl+][Shift+]<key>`: single letters lower case, other keys by their `KeyboardEvent.key`
//! name (`Enter`, `[`, `1`).

use std::collections::BTreeMap;

use serde::Serialize;

use super::{HOT_CUES, PAD_COUNT};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Binding {
    pub action: String,
    pub key: String,
}

/// Actions that exist once per deck and their default key on deck A and deck B.
const DECK_ACTIONS: [(&str, &str, &str); 9] = [
    ("play", "q", "p"),
    ("cue", "a", "l"),
    ("sync", "g", ";"),
    ("loop_1", "w", "o"),
    ("loop_2", "e", "i"),
    ("loop_4", "r", "u"),
    ("loop_8", "t", "y"),
    ("loop_exit", "s", "k"),
    ("bend_back", "d", "j"),
];
/// Hot cue keys: deck A 1..4 (Shift = 5..8), deck B 7,8,9,0 (Shift = 5..8).
const HOT_CUE_KEYS_A: [&str; 4] = ["1", "2", "3", "4"];
const HOT_CUE_KEYS_B: [&str; 4] = ["7", "8", "9", "0"];
/// Other actions with one key.
const GLOBAL_ACTIONS: [(&str, &str); 5] = [
    ("deck_a.bend_forward", "f"),
    ("deck_b.bend_forward", "h"),
    ("crossfader.left", "["),
    ("crossfader.right", "]"),
    ("crossfader.center", "\\"),
];
const AUTO_MIX_KEYS: [(&str, &str); 2] = [("auto_mix.to_a", "c"), ("auto_mix.to_b", "v")];

/// Canonical form of a key: modifiers in the order Alt, Ctrl, Shift; letters lower case; empty
/// when there is nothing usable.
pub fn normalize(key: &str) -> String {
    let (mut alt, mut ctrl, mut shift) = (false, false, false);
    let mut rest = key.trim();
    // a lone "+" is a key itself
    while let Some((head, tail)) = rest.split_once('+') {
        if tail.is_empty() {
            break;
        }
        match head.trim().to_ascii_lowercase().as_str() {
            "alt" | "option" => alt = true,
            "ctrl" | "control" | "cmd" | "meta" | "command" => ctrl = true,
            "shift" => shift = true,
            _ => break,
        }
        rest = tail.trim();
    }
    if rest.is_empty() {
        return String::new();
    }
    let base = if rest.chars().count() == 1 { rest.to_lowercase() } else { rest.to_owned() };
    let mut out = String::new();
    for (on, name) in [(alt, "Alt+"), (ctrl, "Ctrl+"), (shift, "Shift+")] {
        if on {
            out.push_str(name);
        }
    }
    out + &base
}

/// The shortcuts HomeFi ships with.
pub fn default_bindings() -> Vec<Binding> {
    let mut v = Vec::new();
    let mut add = |action: String, key: &str| v.push(Binding { action, key: key.to_owned() });
    for (name, a, b) in DECK_ACTIONS {
        add(format!("deck_a.{name}"), a);
        add(format!("deck_b.{name}"), b);
    }
    for (deck, keys) in [("deck_a", HOT_CUE_KEYS_A), ("deck_b", HOT_CUE_KEYS_B)] {
        for (i, key) in keys.iter().enumerate() {
            add(format!("{deck}.hot_cue_{}", i + 1), key);
            add(format!("{deck}.hot_cue_{}", i + 1 + keys.len()), &format!("Shift+{key}"));
        }
    }
    for (action, key) in GLOBAL_ACTIONS.into_iter().chain(AUTO_MIX_KEYS) {
        add(action.to_owned(), key);
    }
    for pad in 1..=PAD_COUNT {
        add(format!("sampler.{pad}"), &format!("Alt+{pad}"));
    }
    debug_assert_eq!(HOT_CUE_KEYS_A.len() * 2, HOT_CUES);
    v
}

/// The defaults with `overrides` (action to key) applied; an empty key unbinds the action and
/// an override of an unknown action is ignored.
pub fn resolve(overrides: &BTreeMap<String, String>) -> Vec<Binding> {
    default_bindings()
        .into_iter()
        .filter_map(|mut b| {
            if let Some(key) = overrides.get(&b.action) {
                b.key = key.clone();
            }
            b.key = normalize(&b.key);
            (!b.key.is_empty()).then_some(b)
        })
        .collect()
}

/// Keys used by more than one action, with those actions.
pub fn conflicts(bindings: &[Binding]) -> Vec<(String, Vec<String>)> {
    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for b in bindings {
        by_key.entry(normalize(&b.key)).or_default().push(b.action.clone());
    }
    by_key.into_iter().filter(|(k, actions)| !k.is_empty() && actions.len() > 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_conflict_free_and_complete() {
        let d = default_bindings();
        assert!(conflicts(&d).is_empty(), "{:?}", conflicts(&d));
        let actions: Vec<&str> = d.iter().map(|b| b.action.as_str()).collect();
        for needed in
            ["deck_a.play", "deck_b.cue", "deck_a.hot_cue_8", "deck_b.hot_cue_5", "sampler.8", "auto_mix.to_b"]
        {
            assert!(actions.contains(&needed), "{needed}");
        }
        // every action once
        let mut sorted = actions.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), actions.len());
        // keys are already canonical
        assert!(d.iter().all(|b| normalize(&b.key) == b.key), "keys written canonically");
    }

    #[test]
    fn normalizing_keys() {
        assert_eq!(normalize("Q"), "q");
        assert_eq!(normalize("shift+ctrl+alt+Z"), "Alt+Ctrl+Shift+z");
        assert_eq!(normalize(" Shift + 1 "), "Shift+1");
        assert_eq!(normalize("Enter"), "Enter");
        assert_eq!(normalize("+"), "+");
        assert_eq!(normalize("Shift++"), "Shift++");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn overrides_rebind_unbind_and_conflicts_are_found() {
        let mut o = BTreeMap::new();
        o.insert("deck_a.play".to_owned(), "Z".to_owned());
        o.insert("deck_b.play".to_owned(), String::new());
        o.insert("nonsense.action".to_owned(), "x".to_owned());
        let r = resolve(&o);
        assert_eq!(r.iter().find(|b| b.action == "deck_a.play").unwrap().key, "z");
        assert!(r.iter().all(|b| b.action != "deck_b.play" && b.action != "nonsense.action"));
        assert!(conflicts(&r).is_empty());

        // binding a taken key is reported
        o.insert("deck_a.cue".to_owned(), "Z".to_owned());
        let c = conflicts(&resolve(&o));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, "z");
        assert_eq!(c[0].1.len(), 2);
    }
}
