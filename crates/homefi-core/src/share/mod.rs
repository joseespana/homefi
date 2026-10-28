//! Shared control ("car mode"): one device is the **hub** (plays the music and
//! owns the shared queue), the others are **guests** that search, add songs
//! and, when allowed, skip, pause or reorder. Design: `docs/shared-control.md`.
//!
//! - [`protocol`]: the JSON messages (always compiled, also used by the web page).
//! - [`permissions`] and [`queue`]: the hub's rules, the fair queue and skip
//!   votes, as pure logic.
//! - [`sync`]: the shared clock and how guests play along (party mode).
//! - `Hub` / `GuestClient` (feature `share`): the WebSocket + HTTP server and
//!   the guest connection. The hub never plays audio or searches by itself;
//!   the host app plugs in through [`HubHost`] and pushes what its player does.

pub mod permissions;
pub mod protocol;
pub mod queue;
pub mod sync;

#[cfg(feature = "share")]
mod guest;
#[cfg(feature = "share")]
mod hub;

#[cfg(feature = "share")]
pub use guest::{GuestClient, GuestEvent, Welcome, EVENT_QUEUE as GUEST_EVENT_QUEUE};
#[cfg(feature = "share")]
pub use hub::{CoverImage, HostCommand, Hub, HubConfig, HubEvent, HubHost};

pub use permissions::{AddMode, EditMode, Permissions, QueueOrder, SkipMode};
pub use protocol::{
    AudioStream, BrowseKind, Collection, GuestMessage, HubMessage, Member, Page, QueueItem, SearchResults, ShareTrack,
    Source, State, HUB_ID,
};

use url::Url;

/// Port the hub listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 47800;

/// What a guest needs to join: shown as a QR code by the hub and returned by
/// discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinInfo {
    pub host: String,
    pub port: u16,
    pub code: String,
    pub session_name: String,
}

impl JoinInfo {
    /// `homefi://join?host=..&port=..&code=..&name=..` (the QR payload).
    pub fn to_uri(&self) -> String {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("host", &self.host)
            .append_pair("port", &self.port.to_string())
            .append_pair("code", &self.code)
            .append_pair("name", &self.session_name)
            .finish();
        format!("homefi://join?{query}")
    }

    /// Parses [`Self::to_uri`]'s output; `None` for anything else.
    pub fn parse(uri: &str) -> Option<Self> {
        let url = Url::parse(uri.trim()).ok()?;
        if url.scheme() != "homefi" || url.host_str() != Some("join") {
            return None;
        }
        let mut info =
            JoinInfo { host: String::new(), port: DEFAULT_PORT, code: String::new(), session_name: String::new() };
        for (k, v) in url.query_pairs() {
            match &*k {
                "host" => info.host = v.into_owned(),
                "port" => info.port = v.parse().ok()?,
                "code" => info.code = v.into_owned(),
                "name" => info.session_name = v.into_owned(),
                _ => {}
            }
        }
        (!info.host.is_empty()).then_some(info)
    }
}

/// Best guess at this device's LAN address, for the QR code when the app does
/// not know a better one (it should pass its Wi-Fi Direct / hotspot address:
/// on a phone the default route is often mobile data). No packet is sent: a
/// UDP "connect" only picks the outgoing interface.
pub fn local_ip_guess() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("192.0.2.1", 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}

/// MIME type of cover bytes served by the hub (hosts hand over raw bytes).
pub fn image_mime(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_uri_round_trips() {
        let info = JoinInfo {
            host: "192.168.49.1".into(),
            port: 47800,
            code: "0427".into(),
            session_name: "Música de José & co".into(),
        };
        let uri = info.to_uri();
        assert!(uri.starts_with("homefi://join?host=192.168.49.1&port=47800&code=0427&name="));
        assert_eq!(JoinInfo::parse(&uri), Some(info));
        assert_eq!(JoinInfo::parse("https://join?host=x"), None);
        assert_eq!(JoinInfo::parse("homefi://join?port=1"), None);
        assert_eq!(JoinInfo::parse("homefi://join?host=h&port=nope"), None);
    }

    #[test]
    fn sniffs_cover_types() {
        assert_eq!(image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
        assert_eq!(image_mime(b"\x89PNG\r\n"), "image/png");
        assert_eq!(image_mime(b"RIFF\0\0\0\0WEBPVP8 "), "image/webp");
        assert_eq!(image_mime(b""), "application/octet-stream");
    }
}
