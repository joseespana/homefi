//! The guest side: connect to a hub, send [`GuestMessage`]s, receive
//! [`GuestEvent`]s.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use super::permissions::Permissions;
use super::protocol::{GuestMessage, HubMessage, State};
use super::sync::{expected_position, ClockSync};
use crate::{Error, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The hub pings every 15 s; silence for this long means it is gone (e.g. the
/// guest walked out of Wi-Fi range without a TCP close).
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
/// Clock samples: a quick burst after joining, then one now and then to follow drift.
const CLOCK_BURST: u32 = 5;
const CLOCK_BURST_EVERY: Duration = Duration::from_millis(150);
const CLOCK_EVERY: Duration = Duration::from_secs(3);

/// What the hub said when the guest joined.
#[derive(Debug, Clone, PartialEq)]
pub struct Welcome {
    pub guest_id: String,
    pub session_name: String,
    pub permissions: Permissions,
    pub state: State,
}

// Each event is moved once through a channel; boxing would only make
// matching on `Message(HubMessage::State(..))` clumsier.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum GuestEvent {
    /// `state`, `results`, `page`, `notice`, `denied`, ...
    Message(HubMessage),
    /// The connection ended; `reason` is the hub's text when it closed it
    /// (kicked, hub stopped). Always the last event.
    Disconnected { reason: Option<String> },
}

enum Outbound {
    Message(GuestMessage),
    Close,
}

/// A connection to a hub. Dropping it disconnects.
pub struct GuestClient {
    tx: mpsc::UnboundedSender<Outbound>,
    base_url: String,
    guest_id: String,
    task: JoinHandle<()>,
    clock: Arc<Mutex<ClockSync>>,
    /// Zero of this client's local clock (what the clock samples use).
    epoch: Instant,
}

impl GuestClient {
    /// Joins the hub at `host:port`. Fails with the hub's reason when it refuses
    /// (`Error::Share("Wrong join code")`, ...). Returns the client, the welcome
    /// and the stream of everything that follows.
    pub async fn connect(
        host: &str,
        port: u16,
        name: &str,
        color: &str,
        code: &str,
    ) -> Result<(Self, Welcome, mpsc::UnboundedReceiver<GuestEvent>)> {
        // IPv6 literals need brackets in URLs.
        let authority = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        let (mut ws, _) =
            tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(format!("ws://{authority}/ws")))
                .await
                .map_err(|_| Error::Share("Could not reach the hub (timed out)".into()))?
                .map_err(|e| Error::Share(format!("Could not reach the hub: {e}")))?;

        let hello = GuestMessage::Hello {
            name: name.to_owned(),
            color: color.to_owned(),
            code: code.to_owned(),
            app_version: crate::VERSION.to_owned(),
        };
        ws.send(Message::Text(serde_json::to_string(&hello)?.into()))
            .await
            .map_err(|e| Error::Share(format!("Could not reach the hub: {e}")))?;

        let welcome = tokio::time::timeout(CONNECT_TIMEOUT, async {
            while let Some(msg) = ws.next().await {
                match msg {
                    Ok(Message::Text(t)) => match serde_json::from_str::<HubMessage>(t.as_str()) {
                        Ok(HubMessage::Welcome { guest_id, session_name, permissions, state }) => {
                            return Ok(Welcome { guest_id, session_name, permissions, state });
                        }
                        Ok(HubMessage::Denied { reason }) => return Err(Error::Share(reason)),
                        _ => {}
                    },
                    Ok(Message::Close(frame)) => {
                        let reason = frame.map(|f| f.reason.as_str().to_owned()).filter(|r| !r.is_empty());
                        return Err(Error::Share(reason.unwrap_or_else(|| "The hub closed the connection".into())));
                    }
                    Ok(_) => {}
                    Err(e) => return Err(Error::Share(format!("Connection to the hub failed: {e}"))),
                }
            }
            Err(Error::Share("The hub closed the connection".into()))
        })
        .await
        .map_err(|_| Error::Share("The hub did not answer".into()))??;

        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Outbound>();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let clock = Arc::new(Mutex::new(ClockSync::default()));
        let epoch = Instant::now();
        let samples = clock.clone();
        let local_ms = move || epoch.elapsed().as_millis() as u64;
        let task = tokio::spawn(async move {
            let (mut sink, mut stream) = ws.split();
            let mut reason = None;
            let mut sent = 0u32;
            let mut ticker = tokio::time::interval(CLOCK_BURST_EVERY);
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        sent += 1;
                        if sent == CLOCK_BURST {
                            ticker = tokio::time::interval_at(tokio::time::Instant::now() + CLOCK_EVERY, CLOCK_EVERY);
                        }
                        let Ok(text) = serde_json::to_string(&GuestMessage::Clock { t0: local_ms() }) else { continue };
                        if sink.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    out = out_rx.recv() => match out {
                        Some(Outbound::Message(msg)) => {
                            let Ok(text) = serde_json::to_string(&msg) else { continue };
                            if sink.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                        Some(Outbound::Close) | None => {
                            let _ = sink.send(Message::Close(None)).await;
                            break;
                        }
                    },
                    next = tokio::time::timeout(IDLE_TIMEOUT, stream.next()) => match next {
                        Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<HubMessage>(t.as_str()) {
                            Ok(HubMessage::Unknown) => {}
                            Ok(HubMessage::Clock { t0, hub_ms }) => {
                                let t3 = local_ms();
                                samples.lock().unwrap_or_else(|e| e.into_inner()).add(t0, hub_ms, t3);
                            }
                            Ok(msg) => {
                                let _ = event_tx.send(GuestEvent::Message(msg));
                            }
                            Err(e) => tracing::debug!("share: bad message from the hub: {e}"),
                        },
                        Ok(Some(Ok(Message::Close(frame)))) => {
                            reason = frame.map(|f| f.reason.as_str().to_owned()).filter(|r| !r.is_empty());
                            break;
                        }
                        Ok(Some(Ok(_))) => {}
                        Ok(Some(Err(_))) | Ok(None) => break,
                        Err(_) => {
                            reason = Some("The hub stopped answering".into());
                            break;
                        }
                    },
                }
            }
            let _ = event_tx.send(GuestEvent::Disconnected { reason });
        });

        let client = Self {
            tx: out_tx,
            base_url: format!("http://{authority}"),
            guest_id: welcome.guest_id.clone(),
            task,
            clock,
            epoch,
        };
        Ok((client, welcome, event_rx))
    }

    pub fn guest_id(&self) -> &str {
        &self.guest_id
    }

    /// Queues `msg` for the hub. Fails once disconnected.
    pub fn send(&self, msg: GuestMessage) -> Result<()> {
        self.tx.send(Outbound::Message(msg)).map_err(|_| Error::Share("Not connected to the hub".into()))
    }

    /// Cover URLs may be paths on the hub (`/cover/<id>`): makes them absolute.
    pub fn resolve_url(&self, url: &str) -> String {
        if url.starts_with('/') {
            format!("{}{url}", self.base_url)
        } else {
            url.to_owned()
        }
    }

    /// `http://host:port` of the hub.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The hub's clock now (milliseconds), once the first clock answer came back.
    pub fn hub_now_ms(&self) -> Option<u64> {
        let local = self.epoch.elapsed().as_millis() as u64;
        self.clock.lock().unwrap_or_else(|e| e.into_inner()).hub_now(local)
    }

    /// Round trip of the clock sample in use: the sync is about this precise / 2.
    pub fn clock_round_trip_ms(&self) -> Option<u64> {
        self.clock.lock().unwrap_or_else(|e| e.into_inner()).round_trip()
    }

    /// Where the hub's song is right now, from a `state` (party mode). Before
    /// the clock is known it is the state's own position.
    pub fn expected_position_ms(&self, state: &State) -> u64 {
        match self.hub_now_ms() {
            Some(now) => expected_position(state.position_ms, state.clock_ms, state.playing, now),
            None => state.position_ms,
        }
    }

    pub fn is_connected(&self) -> bool {
        !self.task.is_finished()
    }

    /// Leaves the session; a `Disconnected` event follows.
    pub fn close(&self) {
        let _ = self.tx.send(Outbound::Close);
    }
}

impl Drop for GuestClient {
    fn drop(&mut self) {
        // Let the task send the close frame rather than aborting it.
        let _ = self.tx.send(Outbound::Close);
    }
}
