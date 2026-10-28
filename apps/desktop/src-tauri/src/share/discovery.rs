//! Finding each other on the local network: the hub announces itself with
//! mDNS / DNS-SD (`_homefi._tcp`, the same service and TXT keys as Android's
//! NSD: `v`, `name`, `port`; never the join code) and guests browse for it.
//! A typed address is checked with the hub's public `GET /hello`.
//!
//! The desktop has no Wi-Fi Direct or Bluetooth: everything goes over the
//! network the computer is on (home Wi-Fi, Ethernet, a phone's hotspot).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;

/// DNS-SD service type (Android's `_homefi._tcp`, plus the mDNS domain).
pub const SERVICE_TYPE: &str = "_homefi._tcp.local.";
/// TXT `v`: the protocol version guests expect.
const VERSION: &str = "1";
const HELLO_TIMEOUT: Duration = Duration::from_millis(1500);

/// A hub found on the network (or answering `/hello` at a typed address).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Nearby {
    /// The session name ("Música de José").
    pub name: String,
    pub host: String,
    pub port: u16,
    /// Streams its music (party mode), when known.
    pub party: Option<bool>,
    /// The mDNS instance, to forget it when it goes away.
    #[serde(skip)]
    pub fullname: String,
}

/// `text` cut to at most `max` UTF-8 bytes on a character boundary (DNS labels
/// and TXT strings are byte-limited).
pub fn fit_bytes(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if out.len() + c.len_utf8() > max {
            break;
        }
        out.push(c);
    }
    out
}

/// The DNS-SD instance name: dots would read as labels, 63 bytes at most.
pub fn instance_name(session_name: &str) -> String {
    let name = session_name.replace('.', " ");
    let name = name.trim();
    fit_bytes(if name.is_empty() { "HomeFi" } else { name }, 63)
}

/// TXT records of the hub, as Android announces them.
pub fn txt_records(session_name: &str, port: u16) -> HashMap<String, String> {
    HashMap::from([
        ("v".to_owned(), VERSION.to_owned()),
        ("name".to_owned(), fit_bytes(session_name, 200)),
        ("port".to_owned(), port.to_string()),
    ])
}

/// mDNS host name for this computer (`<name>.local.`): letters, digits and dashes.
fn host_label(name: &str) -> String {
    let label: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let label = fit_bytes(&label, 50);
    format!("{}.local.", if label.is_empty() { "homefi" } else { &label })
}

/// The instance part of `<instance>._homefi._tcp.local.`, unescaped.
fn instance_of(fullname: &str) -> String {
    let instance = fullname.strip_suffix(SERVICE_TYPE).unwrap_or(fullname).trim_end_matches('.');
    instance.replace("\\.", ".").replace("\\\\", "\\")
}

fn nearby_of(service: &ResolvedService) -> Option<Nearby> {
    let addresses: Vec<IpAddr> = service.get_addresses().iter().map(|a| a.to_ip_addr()).collect();
    // IPv4 first: link-local IPv6 needs a scope that URLs can't carry everywhere
    let host = addresses.iter().find(|a| a.is_ipv4()).or_else(|| addresses.first())?;
    let name = service
        .get_property_val_str("name")
        .map(str::to_owned)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| instance_of(service.get_fullname()));
    let port = match service.get_port() {
        0 => service.get_property_val_str("port").and_then(|p| p.parse().ok())?,
        p => p,
    };
    Some(Nearby { name, host: host.to_string(), port, party: None, fullname: service.get_fullname().to_owned() })
}

type OnChange = Arc<dyn Fn(Vec<Nearby>) + Send + Sync>;

/// One mDNS daemon for the app: announces our hub, browses for the others.
#[derive(Default)]
pub struct Discovery {
    daemon: Mutex<Option<ServiceDaemon>>,
    announced: Mutex<Option<String>>,
    browsing: Mutex<bool>,
    found: Arc<Mutex<Vec<Nearby>>>,
}

impl Discovery {
    fn daemon(&self) -> Option<ServiceDaemon> {
        let mut slot = self.daemon.lock().unwrap();
        if slot.is_none() {
            match ServiceDaemon::new() {
                Ok(d) => *slot = Some(d),
                Err(e) => tracing::warn!("share: mDNS unavailable: {e}"),
            }
        }
        slot.clone()
    }

    /// Announces the hub on the network until [`Self::unannounce`].
    pub fn announce(&self, session_name: &str, host_name: &str, port: u16) {
        self.unannounce();
        let Some(daemon) = self.daemon() else { return };
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name(session_name),
            &host_label(host_name),
            (),
            port,
            txt_records(session_name, port),
        )
        .map(ServiceInfo::enable_addr_auto);
        match info {
            Ok(info) => {
                let fullname = info.get_fullname().to_owned();
                match daemon.register(info) {
                    Ok(()) => *self.announced.lock().unwrap() = Some(fullname),
                    Err(e) => tracing::warn!("share: could not announce the session: {e}"),
                }
            }
            Err(e) => tracing::warn!("share: bad mDNS record: {e}"),
        }
    }

    pub fn unannounce(&self) {
        let Some(fullname) = self.announced.lock().unwrap().take() else { return };
        if let Some(daemon) = self.daemon.lock().unwrap().as_ref() {
            let _ = daemon.unregister(&fullname);
        }
    }

    /// Hubs found so far (while browsing).
    pub fn found(&self) -> Vec<Nearby> {
        self.found.lock().unwrap().clone()
    }

    /// Starts looking for hubs; `on_change` gets the whole list on every change.
    pub fn browse(&self, on_change: OnChange) {
        let mut browsing = self.browsing.lock().unwrap();
        if *browsing {
            return;
        }
        let Some(daemon) = self.daemon() else { return };
        let rx = match daemon.browse(SERVICE_TYPE) {
            Ok(rx) => rx,
            Err(e) => {
                tracing::warn!("share: could not look for sessions: {e}");
                return;
            }
        };
        *browsing = true;
        let found = self.found.clone();
        std::thread::Builder::new()
            .name("homefi-mdns".into())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    let changed = {
                        let mut list = found.lock().unwrap();
                        match event {
                            ServiceEvent::ServiceResolved(service) => match nearby_of(&service) {
                                Some(hub) => {
                                    let before = list.clone();
                                    // the same hub heard again (new address, renamed) is one entry
                                    list.retain(|h| {
                                        h.fullname != hub.fullname && (&h.host, h.port) != (&hub.host, hub.port)
                                    });
                                    list.push(hub);
                                    *list != before
                                }
                                None => false,
                            },
                            ServiceEvent::ServiceRemoved(_, fullname) => {
                                let n = list.len();
                                list.retain(|h| h.fullname != fullname);
                                list.len() != n
                            }
                            ServiceEvent::SearchStopped(_) => break,
                            _ => false,
                        }
                    };
                    if changed {
                        on_change(found.lock().unwrap().clone());
                    }
                }
            })
            .expect("spawn mDNS browser");
    }

    pub fn stop_browse(&self) {
        let mut browsing = self.browsing.lock().unwrap();
        if !*browsing {
            return;
        }
        *browsing = false;
        if let Some(daemon) = self.daemon.lock().unwrap().as_ref() {
            let _ = daemon.stop_browse(SERVICE_TYPE);
        }
        self.found.lock().unwrap().clear();
    }

    pub fn shutdown(&self) {
        self.unannounce();
        if let Some(daemon) = self.daemon.lock().unwrap().take() {
            let _ = daemon.shutdown();
        }
    }
}

/// Is there a HomeFi hub at `host:port`? (`GET /hello`, never the code.)
pub async fn hello(host: &str, port: u16) -> Option<Nearby> {
    let authority = if host.contains(':') && !host.starts_with('[') { format!("[{host}]") } else { host.to_owned() };
    let client = reqwest::Client::builder().timeout(HELLO_TIMEOUT).build().ok()?;
    let json: serde_json::Value =
        client.get(format!("http://{authority}:{port}/hello")).send().await.ok()?.json().await.ok()?;
    if json.get("app").and_then(|a| a.as_str()) != Some("homefi") {
        return None;
    }
    let name = json.get("name").and_then(|n| n.as_str()).filter(|n| !n.trim().is_empty()).unwrap_or(host);
    Some(Nearby {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
        party: json.get("party").and_then(|p| p.as_bool()),
        fullname: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn txt_records_match_android() {
        let txt = txt_records("Música de José", 47800);
        assert_eq!(txt.len(), 3);
        assert_eq!(txt["v"], "1");
        assert_eq!(txt["name"], "Música de José");
        assert_eq!(txt["port"], "47800");
        assert!(!txt.contains_key("code"));
        let long = "é".repeat(150);
        assert_eq!(txt_records(&long, 1)["name"].len(), 200);
    }

    #[test]
    fn names_fit_dns_limits() {
        assert_eq!(fit_bytes("abc", 2), "ab");
        // never half a character
        assert_eq!(fit_bytes("aé", 2), "a");
        assert_eq!(instance_name("  "), "HomeFi");
        assert_eq!(instance_name("Mr. José"), "Mr  José");
        assert!(instance_name(&"x".repeat(100)).len() <= 63);
        assert_eq!(host_label("José's MacBook Pro"), "jos-s-macbook-pro.local.");
        assert_eq!(host_label("..."), "homefi.local.");
    }

    #[test]
    fn instance_names_come_back_unescaped() {
        assert_eq!(instance_of("Mr\\. X._homefi._tcp.local."), "Mr. X");
        assert_eq!(instance_of("Party._homefi._tcp.local."), "Party");
    }
}
