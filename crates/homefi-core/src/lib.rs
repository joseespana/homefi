//! HomeFi core library.
//!
//! Shared by the `homefi` CLI and the Android app (through `homefi-ffi`):
//! Tidal authentication, the API client, stream parsing, path templating,
//! audio tagging and the concurrent download engine.

pub mod api;
pub mod artwork;
pub mod auth;
pub mod catalog;
pub mod config;
pub mod download;
pub mod error;
pub mod ffmpeg;
pub mod flac_tags;
pub mod humane;
pub mod library;
pub mod lyrics;
pub mod m3u;
pub mod metadata;
pub mod models;
pub mod paths;
pub mod quality;
pub mod remux;
pub mod resource;
pub mod session;
pub mod share;
pub mod stream;
pub mod stream_cache;
pub mod template;

pub use error::{Error, Result};

/// Version of the core crate (shared by the whole workspace).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
