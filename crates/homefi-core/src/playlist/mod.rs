//! HomeFi's own playlists: a model saved as JSON in the app folder, their
//! settings, the resolver that turns song names into Tidal tracks and library
//! files, and saving a copy into the user's TIDAL account (only when asked:
//! the button, or the "save to TIDAL automatically" setting).

mod model;
mod resolve;
mod settings;
mod store;
mod tidal_sync;

pub(crate) use model::now_ms;
pub use model::{
    new_id, ChatOrigin, ConcertOrigin, EntryTag, HomePlaylist, PartyOrigin, PlaylistEntry, PlaylistOrigin, RideOrigin,
    SetlistSource, SongCandidate, SourceLink, MAX_SOURCES, PLAYLISTS_DIR, PLAYLIST_FORMAT_VERSION,
};
pub(crate) use resolve::{artist_key, entry_from_library};
pub use resolve::{
    entry_from_track, score, LibraryIndex, LibraryRef, Resolved, Resolver, SeenEntries, MIN_CONFIDENCE,
    RESOLVE_PARALLEL, RESOLVE_TIMEOUT,
};
pub use settings::{
    lenient_playlist_settings, PlaylistSettings, DEFAULT_SEARCH_BUDGET, MAX_SEARCH_BUDGET, MIN_SEARCH_BUDGET,
};
pub use store::PlaylistStore;
pub use tidal_sync::{delete_from_tidal, save_stored_to_tidal, save_to_tidal, TidalSaveReport, TIDAL_ITEMS_PER_CALL};
