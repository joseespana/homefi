//! HomeFi's own playlists: a model saved as JSON in the app folder (nothing is
//! ever written to the Tidal account), their settings and the resolver that
//! turns song names into Tidal tracks and library files.

mod model;
mod resolve;
mod settings;
mod store;

pub(crate) use model::now_ms;
pub use model::{
    new_id, ChatOrigin, ConcertOrigin, EntryTag, HomePlaylist, PartyOrigin, PlaylistEntry, PlaylistOrigin,
    SetlistSource, SongCandidate, SourceLink, MAX_SOURCES, PLAYLISTS_DIR, PLAYLIST_FORMAT_VERSION,
};
pub(crate) use resolve::artist_key;
pub use resolve::{
    entry_from_track, score, LibraryIndex, LibraryRef, Resolved, Resolver, SeenEntries, MIN_CONFIDENCE,
    RESOLVE_PARALLEL, RESOLVE_TIMEOUT,
};
pub use settings::{
    lenient_playlist_settings, PlaylistSettings, DEFAULT_SEARCH_BUDGET, MAX_SEARCH_BUDGET, MIN_SEARCH_BUDGET,
};
pub use store::PlaylistStore;
