//! Mock WebRTC/WebSocket transport: endpoints connected by a fully
//! deterministic simulated link.
//!
//! Delivery runs on a *virtual* clock: `send` queues a message with a
//! delivery deadline (`now + latency`), and [`MockNetworkTransport::advance`]
//! moves simulated time forward, delivering due messages into recipients'
//! inboxes (broadcast: every endpoint except the sender). No sleeps, no
//! flakiness — the same scenario always produces the same delivery order,
//! drops, and reordering, which is exactly what a reproducible CI test for
//! `tpt-av-sync` needs.
//!
//! ```rust
//! use core::time::Duration;
//! use tpt_av_test_mock::network::MockNetworkTransport;
//!
//! let transport = MockNetworkTransport::new();
//! let (alice, bob) = transport.pair();
//!
//! alice.send(b"hello").unwrap();
//! assert_eq!(bob.try_recv(), None, "latency has not elapsed yet");
//!
//! transport.advance(Duration::from_millis(50));
//! assert_eq!(bob.try_recv().unwrap().payload, b"hello".to_vec());
//! assert_eq!(transport.stats().delivered, 1);
//! ```

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

/// Identifies one end of a simulated link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EndpointId(pub usize);

/// A message on the simulated link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkMessage {
    /// Payload bytes exactly as sent.
    pub payload: Vec<u8>,
    /// Sender of the message.
    pub from: EndpointId,
}

/// Errors the mock transport can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkError {
    /// The link is down (`MockNetworkTransport::set_connected(false)`).
    Disconnected,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::Disconnected => write!(f, "the simulated link is disconnected"),
        }
    }
}

impl std::error::Error for NetworkError {}

/// Delivery statistics of the simulated link.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// Messages accepted by `send`.
    pub sent: u64,
    /// Messages delivered into a receiver's inbox.
    pub delivered: u64,
    /// Messages destroyed by the drop rate before delivery.
    pub dropped: u64,
    /// Messages currently queued in simulated flight.
    pub in_flight: u64,
}

struct InFlight {
    message: NetworkMessage,
    deliver_at: std::time::Duration,
}

struct Mailbox {
    id: EndpointId,
    inbox: Arc<Mutex<VecDeque<NetworkMessage>>>,
}

struct LinkState {
    virtual_now: std::time::Duration,
    latency: std::time::Duration,
    drop_rate: f64,
    reorder_rate: f64,
    connected: bool,
    queue: VecDeque<InFlight>,
    mailboxes: Vec<Mailbox>,
    stats: LinkStats,
    rng: RefCell<SplitMix64>,
}

impl LinkState {
    /// Queues a message from `from`, applying drop/reorder/latency.
    fn transmit(&mut self, from: EndpointId, payload: Vec<u8>) -> Result<(), NetworkError> {
        if !self.connected {
            return Err(NetworkError::Disconnected);
        }
        self.stats.sent += 1;

        let mut rng = self.rng.borrow_mut();
        if rng.next_unit() < self.drop_rate {
            self.stats.dropped += 1;
            return Ok(()); // silently eaten, like a real lossy link
        }
        let reorder = rng.next_unit() < self.reorder_rate;
        drop(rng);

        let deliver_at = self.virtual_now + self.latency;
        let mut position = self.queue.len();
        if reorder && position > 0 {
            position -= 1; // swap one slot forward: overtakes its neighbour
        }
        self.queue.insert(
            position,
            InFlight {
                message: NetworkMessage { payload, from },
                deliver_at,
            },
        );
        Ok(())
    }

    /// Delivers every due message into all other endpoints' inboxes.
    /// Delivery order: send order (the queue), and within one message the
    /// mailbox registration order — fully deterministic.
    fn deliver_due(&mut self, now: std::time::Duration) -> usize {
        let mut due = Vec::new();
        let mut survivors = VecDeque::new();
        for in_flight in self.queue.drain(..) {
            if in_flight.deliver_at <= now {
                due.push(in_flight.message);
            } else {
                survivors.push_back(in_flight);
            }
        }
        self.queue = survivors;
        let count = due.len();
        for message in due {
            for mailbox in &self.mailboxes {
                if mailbox.id != message.from {
                    mailbox
                        .inbox
                        .lock()
                        .expect("inbox poisoned")
                        .push_back(message.clone());
                }
            }
        }
        self.stats.delivered += count as u64;
        self.stats.in_flight = self.queue.len() as u64;
        count
    }
}

/// Splitmix64: a tiny deterministic RNG so loss/reordering never needs a
/// random dependency or a real clock.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The controller side of a mock transport: owns the virtual clock, the
/// impairment knobs (latency, drop, reorder), and the statistics. Cheap to
/// clone; all clones steer the same link.
#[derive(Clone)]
pub struct MockNetworkTransport {
    link: Arc<Mutex<LinkState>>,
    next_endpoint: Arc<std::sync::atomic::AtomicUsize>,
}

impl MockNetworkTransport {
    /// A lossless, ordered, zero-latency, connected link with a fixed seed,
    /// so scenarios replay identically everywhere.
    pub fn new() -> Self {
        Self::with_seed(0x1A2B_3C4D_5E6F_7081)
    }

    /// Like [`MockNetworkTransport::new`] but with an explicit RNG seed for
    /// drop/reorder decisions.
    pub fn with_seed(seed: u64) -> Self {
        MockNetworkTransport {
            link: Arc::new(Mutex::new(LinkState {
                virtual_now: std::time::Duration::ZERO,
                latency: std::time::Duration::ZERO,
                drop_rate: 0.0,
                reorder_rate: 0.0,
                connected: true,
                queue: VecDeque::new(),
                mailboxes: Vec::new(),
                stats: LinkStats::default(),
                rng: RefCell::new(SplitMix64(seed)),
            })),
            next_endpoint: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Connects two fresh endpoints through this transport. Any number of
    /// pairs can share one transport; they all share its clock and stats,
    /// and every send is broadcast to all *other* endpoints.
    pub fn pair(&self) -> (Endpoint, Endpoint) {
        let a = self
            .next_endpoint
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let b = self
            .next_endpoint
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut link = self.link.lock().expect("link poisoned");
        let mut mailbox_for = |id: usize| {
            let inbox = Arc::new(Mutex::new(VecDeque::new()));
            link.mailboxes.push(Mailbox {
                id: EndpointId(id),
                inbox: Arc::clone(&inbox),
            });
            inbox
        };
        let a_inbox = mailbox_for(a);
        let b_inbox = mailbox_for(b);
        drop(link);
        (
            Endpoint {
                id: EndpointId(a),
                link: Arc::clone(&self.link),
                inbox: a_inbox,
            },
            Endpoint {
                id: EndpointId(b),
                link: Arc::clone(&self.link),
                inbox: b_inbox,
            },
        )
    }

    /// Simulated link latency, applied to every future `send`.
    pub fn set_latency(&self, latency: std::time::Duration) {
        self.link.lock().expect("link poisoned").latency = latency;
    }

    /// Fraction `0.0..=1.0` of messages destroyed in flight.
    ///
    /// # Panics
    /// If the rate is outside `0.0..=1.0`.
    pub fn set_drop_rate(&self, rate: f64) {
        assert!(
            (0.0..=1.0).contains(&rate),
            "drop rate must be within 0.0..=1.0"
        );
        self.link.lock().expect("link poisoned").drop_rate = rate;
    }

    /// Fraction `0.0..=1.0` of surviving messages delivered out of order
    /// (each swaps one position forward on the queue).
    ///
    /// # Panics
    /// If the rate is outside `0.0..=1.0`.
    pub fn set_reorder_rate(&self, rate: f64) {
        assert!(
            (0.0..=1.0).contains(&rate),
            "reorder rate must be within 0.0..=1.0"
        );
        self.link.lock().expect("link poisoned").reorder_rate = rate;
    }

    /// Raising or cutting the link: `false` makes [`Endpoint::send`] fail
    /// with [`NetworkError::Disconnected`] until reconnected. Messages
    /// already in flight still arrive.
    pub fn set_connected(&self, connected: bool) {
        self.link.lock().expect("link poisoned").connected = connected;
    }

    /// Moves the virtual clock forward by `by`, delivering every message
    /// whose deadline has passed into recipients' inboxes.
    pub fn advance(&self, by: std::time::Duration) {
        let mut link = self.link.lock().expect("link poisoned");
        link.virtual_now += by;
        let now = link.virtual_now;
        link.deliver_due(now);
    }

    /// Advances the virtual clock in latency-sized steps until nothing is in
    /// flight (or `max_rounds` rounds have passed).
    pub fn flush(&self, max_rounds: usize) {
        for _ in 0..max_rounds {
            let (in_flight, latency) = {
                let link = self.link.lock().expect("link poisoned");
                (link.queue.len(), link.latency)
            };
            if in_flight == 0 {
                break;
            }
            self.advance(latency.max(std::time::Duration::from_nanos(1)));
        }
    }

    /// Current link statistics.
    pub fn stats(&self) -> LinkStats {
        let mut link = self.link.lock().expect("link poisoned");
        link.stats.in_flight = link.queue.len() as u64;
        link.stats
    }

    /// The virtual clock reading.
    pub fn virtual_now(&self) -> std::time::Duration {
        self.link.lock().expect("link poisoned").virtual_now
    }
}

impl Default for MockNetworkTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MockNetworkTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let link = self.link.lock().expect("link poisoned");
        f.debug_struct("MockNetworkTransport")
            .field("virtual_now", &link.virtual_now)
            .field("latency", &link.latency)
            .field("drop_rate", &link.drop_rate)
            .field("reorder_rate", &link.reorder_rate)
            .field("connected", &link.connected)
            .field("stats", &link.stats)
            .finish()
    }
}

/// One end of a simulated link. Cloneable; all clones share the inbox.
#[derive(Clone)]
pub struct Endpoint {
    id: EndpointId,
    link: Arc<Mutex<LinkState>>,
    inbox: Arc<Mutex<VecDeque<NetworkMessage>>>,
}

impl Endpoint {
    /// This endpoint's identifier on the link.
    pub fn id(&self) -> EndpointId {
        self.id
    }

    /// Broadcasts payload bytes to every other endpoint on the transport.
    pub fn send(&self, payload: impl Into<Vec<u8>>) -> Result<(), NetworkError> {
        let mut link = self.link.lock().expect("link poisoned");
        link.transmit(self.id, payload.into())
    }

    /// Polls the next delivered message, oldest first.
    pub fn try_recv(&self) -> Option<NetworkMessage> {
        self.inbox.lock().expect("inbox poisoned").pop_front()
    }

    /// Number of delivered-but-unread messages.
    pub fn inbox_len(&self) -> usize {
        self.inbox.lock().expect("inbox poisoned").len()
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Endpoint")
            .field("id", &self.id)
            .field("inbox_len", &self.inbox_len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn zero_latency_delivers_on_advance_only() {
        let transport = MockNetworkTransport::new();
        let (alice, bob) = transport.pair();
        alice.send(b"ping").unwrap();
        assert_eq!(transport.stats().sent, 1);
        assert_eq!(transport.stats().in_flight, 1);
        assert_eq!(bob.try_recv(), None, "time has not advanced yet");

        transport.advance(Duration::ZERO);
        let message = bob.try_recv().expect("delivered after advance");
        assert_eq!(message.payload, b"ping");
        assert_eq!(message.from, alice.id());
        assert_eq!(alice.try_recv(), None, "sender never sees its own message");
    }

    #[test]
    fn latency_holds_messages_until_the_deadline() {
        let transport = MockNetworkTransport::new();
        transport.set_latency(Duration::from_millis(100));
        let (alice, bob) = transport.pair();

        alice.send(b"late").unwrap();
        transport.advance(Duration::from_millis(99));
        assert_eq!(bob.try_recv(), None, "99 ms < 100 ms latency");

        transport.advance(Duration::from_millis(1));
        assert_eq!(bob.try_recv().unwrap().payload, b"late");
        assert_eq!(transport.virtual_now(), Duration::from_millis(100));
    }

    #[test]
    fn flush_delivers_everything_queued() {
        let transport = MockNetworkTransport::new();
        transport.set_latency(Duration::from_millis(10));
        let (alice, bob) = transport.pair();
        for index in 0..5u8 {
            alice.send([index]).unwrap();
        }
        transport.flush(8);
        assert_eq!(bob.inbox_len(), 5);
        let payload: Vec<u8> = (0..5).map(|_| bob.try_recv().unwrap().payload[0]).collect();
        assert_eq!(payload, vec![0, 1, 2, 3, 4], "FIFO order preserved");
        assert_eq!(transport.stats().in_flight, 0);
    }

    #[test]
    fn disconnect_fails_new_sends_but_not_in_flight_ones() {
        let transport = MockNetworkTransport::new();
        let (alice, bob) = transport.pair();
        alice.send(b"before").unwrap();
        transport.set_connected(false);
        assert_eq!(alice.send(b"during"), Err(NetworkError::Disconnected));
        transport.advance(Duration::ZERO);
        assert_eq!(bob.try_recv().unwrap().payload, b"before");
    }

    #[test]
    fn full_drop_rate_eats_every_message_deterministically() {
        let transport = MockNetworkTransport::with_seed(42);
        transport.set_drop_rate(1.0);
        let (alice, bob) = transport.pair();
        for index in 0..10u8 {
            alice.send([index]).unwrap();
        }
        transport.flush(4);
        assert_eq!(bob.inbox_len(), 0);
        let stats = transport.stats();
        assert_eq!(stats.sent, 10);
        assert_eq!(stats.dropped, 10);
        assert_eq!(stats.delivered, 0);
    }

    #[test]
    fn seeded_links_replay_identically() {
        let run = || {
            let transport = MockNetworkTransport::with_seed(7);
            transport.set_drop_rate(0.3);
            transport.set_reorder_rate(0.5);
            transport.set_latency(Duration::from_millis(5));
            let (alice, bob) = transport.pair();
            for index in 0..32u8 {
                alice.send([index]).unwrap();
            }
            transport.flush(8);
            let received: Vec<u8> = std::iter::from_fn(|| bob.try_recv())
                .map(|m| m.payload[0])
                .collect();
            (received, transport.stats())
        };
        let first = run();
        let second = run();
        assert_eq!(first, second, "same seed must replay the same impairments");
        assert!(first.1.dropped > 0, "seed 7 must produce some drops");
    }

    #[test]
    fn broadcast_reaches_every_peer_once() {
        let transport = MockNetworkTransport::new();
        let (alice, bob) = transport.pair();
        let (carol, dave) = transport.pair();
        alice.send(b"all").unwrap();
        transport.advance(Duration::ZERO);
        assert_eq!(bob.try_recv().unwrap().payload, b"all");
        assert_eq!(carol.try_recv().unwrap().payload, b"all");
        assert_eq!(dave.try_recv().unwrap().payload, b"all");
        assert_eq!(alice.inbox_len(), 0);
        // Delivered count is per message, not per recipient.
        assert_eq!(transport.stats().delivered, 1);
    }

    #[test]
    #[should_panic(expected = "drop rate must be within 0.0..=1.0")]
    fn invalid_drop_rate_is_rejected() {
        MockNetworkTransport::new().set_drop_rate(1.5);
    }
}
