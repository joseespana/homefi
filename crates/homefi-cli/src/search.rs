//! Interactive `download search`.

use std::io::Write;

use anyhow::Result;
use console::style;
use homefi_core::api::TidalApi;
use homefi_core::models::{Album, Search, Track, Video};
use homefi_core::resource::{Resource, ResourceKind};

pub struct Hit {
    pub kind: ResourceKind,
    pub id: String,
    pub title: String,
}

fn by(artist: Option<&homefi_core::models::ArtistRef>, artists: &[homefi_core::models::ArtistRef]) -> String {
    artist.or(artists.first()).map(|a| a.name.clone()).unwrap_or_default()
}

fn track_hit(t: &Track) -> Hit {
    let modes = if t.audio_modes.is_empty() { String::new() } else { format!(" [{}]", t.audio_modes.join(", ")) };
    Hit {
        kind: ResourceKind::Track,
        id: t.id.to_string(),
        title: format!("{} - {}{modes}", by(t.artist.as_ref(), &t.artists), t.title),
    }
}

fn album_hit(a: &Album) -> Hit {
    let modes = if a.audio_modes.is_empty() { String::new() } else { format!(" [{}]", a.audio_modes.join(", ")) };
    Hit {
        kind: ResourceKind::Album,
        id: a.id.to_string(),
        title: format!("{} - {}{modes}", by(a.artist.as_ref(), &a.artists), a.title),
    }
}

fn video_hit(v: &Video) -> Hit {
    Hit {
        kind: ResourceKind::Video,
        id: v.id.to_string(),
        title: format!("{} - {}", by(v.artist.as_ref(), &v.artists), v.title),
    }
}

fn top_hit(results: &Search) -> Option<Hit> {
    let top = results.top_hit.as_ref()?;
    let value = top.value.clone();
    let kind: ResourceKind = top.kind.parse().ok()?;
    Some(match kind {
        ResourceKind::Track => track_hit(&serde_json_from(value)?),
        ResourceKind::Album => album_hit(&serde_json_from(value)?),
        ResourceKind::Video => video_hit(&serde_json_from(value)?),
        ResourceKind::Artist => {
            let a: homefi_core::models::Artist = serde_json_from(value)?;
            Hit { kind, id: a.id.to_string(), title: a.name }
        }
        ResourceKind::Playlist => {
            let p: homefi_core::models::Playlist = serde_json_from(value)?;
            Hit { kind, id: p.uuid, title: p.title }
        }
        ResourceKind::Mix => return None,
    })
}

fn serde_json_from<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Option<T> {
    serde_json::from_value(value).ok()
}

/// Collects the hits shown for `results`, top hit first.
pub fn hits(results: &Search, kinds: &[ResourceKind], per_kind: usize) -> (Option<Hit>, Vec<Hit>) {
    let top = top_hit(results).filter(|h| kinds.contains(&h.kind));
    let mut hits = Vec::new();
    for kind in kinds {
        match kind {
            ResourceKind::Artist => hits.extend(results.artists.items.iter().take(per_kind).map(|a| Hit {
                kind: *kind,
                id: a.id.to_string(),
                title: a.name.clone(),
            })),
            ResourceKind::Album => hits.extend(results.albums.items.iter().take(per_kind).map(album_hit)),
            ResourceKind::Playlist => hits.extend(results.playlists.items.iter().take(per_kind).map(|p| Hit {
                kind: *kind,
                id: p.uuid.clone(),
                title: p.title.clone(),
            })),
            ResourceKind::Track => hits.extend(results.tracks.items.iter().take(per_kind).map(track_hit)),
            ResourceKind::Video => hits.extend(results.videos.items.iter().take(per_kind).map(video_hit)),
            ResourceKind::Mix => {}
        }
    }
    (top, hits)
}

pub async fn run(
    api: &TidalApi,
    query: &str,
    kinds: &[ResourceKind],
    per_kind: usize,
    pick_top: bool,
) -> Result<Vec<Resource>> {
    let results = api.search(query).await?;
    let (top, rest) = hits(&results, kinds, per_kind);

    if pick_top {
        return Ok(match top {
            Some(hit) => {
                eprintln!("{} top hit: {} {}", style("Added").green(), hit.kind, style(&hit.title).bold());
                vec![Resource::new(hit.kind, hit.id)]
            }
            None => {
                eprintln!("{}", style("No top hit for the selected types.").yellow());
                Vec::new()
            }
        });
    }

    let all: Vec<Hit> = top.into_iter().chain(rest).collect();
    if all.is_empty() {
        eprintln!("{}", style("No results.").yellow());
        return Ok(Vec::new());
    }

    eprintln!("{}", style(format!("Results for \"{query}\"")).bold());
    for (i, hit) in all.iter().enumerate() {
        eprintln!(
            "{:>3}  {:<9} {}  {}",
            style(i + 1).yellow(),
            style(hit.kind.as_str()).cyan(),
            style(&hit.title).green(),
            style(&hit.id).magenta().dim()
        );
    }

    eprint!("{}", style("Numbers to download (comma separated, empty = quit): ").green().bold());
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;

    let mut picked = Vec::new();
    for part in line.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        if part.eq_ignore_ascii_case("q") {
            return Ok(Vec::new());
        }
        match part.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).and_then(|i| all.get(i)) {
            Some(hit) => {
                eprintln!("{} {} {}", style("Added").green(), hit.kind, hit.title);
                picked.push(Resource::new(hit.kind, hit.id.clone()));
            }
            None => eprintln!("{} {part}", style("Invalid selection:").red()),
        }
    }
    Ok(picked)
}
