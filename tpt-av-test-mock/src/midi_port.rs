//! Virtual MIDI input/output for CI testing without physical hardware.
//!
//! [`MockMidiDevice`] is a lossless in-memory MIDI port pair: the test (or
//! the "hardware" side) *injects* messages, and the code under test polls
//! them with `try_recv` — the same shape as a real MIDI input callback. The
//! reverse direction (`send`/`drain_output`) records what the code under
//! test transmitted, so output routing can be asserted too.
//!
//! ```rust
//! use tpt_av_test_mock::midi_port::{MidiMessage, MockMidiDevice};
//!
//! let device = MockMidiDevice::new();
//!
//! // Simulate a performer pressing middle C.
//! device.inject(MidiMessage::note_on(0, 60, 100));
//!
//! // The code under test polls the port.
//! let received = device.try_recv().unwrap().expect("note-on queued");
//! assert_eq!(received.bytes, vec![0x90, 60, 100]);
//!
//! // The port is drained; the next poll reports silence.
//! assert_eq!(device.try_recv().unwrap(), None);
//!
//! // Anything the code under test sends back is recorded.
//! device.send(MidiMessage::control_change(0, 7, 127)).unwrap();
//! assert_eq!(device.drain_output().len(), 1);
//! ```

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

/// One MIDI event: the raw wire bytes plus an optional timestamp in
/// microseconds. Short messages are 1-3 bytes; system exclusive messages are
/// wrapped in `0xF0 ... 0xF7`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiMessage {
    /// Raw MIDI wire bytes.
    pub bytes: Vec<u8>,
    /// Timestamp in microseconds since an arbitrary epoch (0 = "now" for
    /// freshly injected messages).
    pub timestamp_us: u64,
}

impl MidiMessage {
    /// Wraps raw bytes into a message with a zero timestamp.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        MidiMessage {
            bytes: bytes.into(),
            timestamp_us: 0,
        }
    }

    /// Wraps raw bytes into a message stamped at `timestamp_us`.
    pub fn with_timestamp(bytes: impl Into<Vec<u8>>, timestamp_us: u64) -> Self {
        MidiMessage {
            bytes: bytes.into(),
            timestamp_us,
        }
    }

    /// Note-on: `0x9n note velocity`.
    pub fn note_on(channel: u8, note: u8, velocity: u8) -> Self {
        MidiMessage::new([0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F])
    }

    /// Note-off: `0x8n note velocity`.
    pub fn note_off(channel: u8, note: u8, velocity: u8) -> Self {
        MidiMessage::new([0x80 | (channel & 0x0F), note & 0x7F, velocity & 0x7F])
    }

    /// Control change: `0xBn controller value`.
    pub fn control_change(channel: u8, controller: u8, value: u8) -> Self {
        MidiMessage::new([0xB0 | (channel & 0x0F), controller & 0x7F, value & 0x7F])
    }

    /// Program change: `0xCn program`.
    pub fn program_change(channel: u8, program: u8) -> Self {
        MidiMessage::new([0xC0 | (channel & 0x0F), program & 0x7F])
    }

    /// Pitch bend from a 14-bit value (`0..=16383`, 8192 = center):
    /// `0xEn lsb msb`.
    pub fn pitch_bend(channel: u8, value14: u16) -> Self {
        MidiMessage::new([
            0xE0 | (channel & 0x0F),
            (value14 & 0x7F) as u8,
            (value14 >> 7) as u8,
        ])
    }

    /// System exclusive message, wrapped in `0xF0 ... 0xF7`.
    pub fn sysex(data: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(data.len() + 2);
        bytes.push(0xF0);
        bytes.extend_from_slice(data);
        bytes.push(0xF7);
        MidiMessage::new(bytes)
    }

    /// The status byte (first byte), if the message is non-empty.
    pub fn status(&self) -> Option<u8> {
        self.bytes.first().copied()
    }

    /// The channel nibble of a channel voice message (`0x80..=0xEF`).
    pub fn channel(&self) -> Option<u8> {
        match self.status()? {
            status @ 0x80..=0xEF => Some(status & 0x0F),
            _ => None,
        }
    }

    /// Whether this is a note-on with non-zero velocity.
    pub fn is_note_on(&self) -> bool {
        matches!(self.status(), Some(status) if status & 0xF0 == 0x90)
    }
}

/// Errors the virtual port can report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MidiError {
    /// The port has been closed with [`MockMidiDevice::close`].
    PortClosed,
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiError::PortClosed => write!(f, "the virtual MIDI port is closed"),
        }
    }
}

impl std::error::Error for MidiError {}

/// The receive half of a MIDI port: poll events coming from the "device".
pub trait MidiInput {
    /// Polls the next queued message; `Ok(None)` when nothing is pending.
    fn try_recv(&mut self) -> Result<Option<MidiMessage>, MidiError>;
}

/// The transmit half of a MIDI port: emit events toward the "device".
pub trait MidiOutput {
    /// Sends one message to the device.
    fn send(&mut self, message: MidiMessage) -> Result<(), MidiError>;
}

#[derive(Default)]
struct PortState {
    input_queue: VecDeque<MidiMessage>,
    output_queue: VecDeque<MidiMessage>,
    closed: bool,
}

/// A virtual MIDI port pair, shared-clonable so handles can move across
/// threads. Lossless and FIFO: every injected message is delivered exactly
/// once, in order.
///
/// Clones share the same port; use [`MockMidiDevice::split`] when the input
/// and output halves should travel independently.
#[derive(Clone, Default)]
pub struct MockMidiDevice {
    state: Arc<Mutex<PortState>>,
}

impl MockMidiDevice {
    /// Creates an open virtual MIDI port.
    pub fn new() -> Self {
        Self::default()
    }

    /// Injects a message into the port as if the hardware had received it;
    /// the next [`MidiInput::try_recv`] hands it to the code under test.
    pub fn inject(&self, message: MidiMessage) {
        self.state
            .lock()
            .expect("MIDI port poisoned")
            .input_queue
            .push_back(message);
    }

    /// Builds and injects a message from raw bytes.
    pub fn inject_bytes(&self, bytes: &[u8]) {
        self.inject(MidiMessage::new(bytes.to_vec()));
    }

    /// Injects a short run of messages (e.g. a whole gesture) at once.
    pub fn inject_all(&self, messages: impl IntoIterator<Item = MidiMessage>) {
        let mut state = self.state.lock().expect("MIDI port poisoned");
        state.input_queue.extend(messages);
    }

    /// Polls the next queued message; `Ok(None)` when the port is drained.
    /// The inherent counterpart of [`MidiInput::try_recv`].
    pub fn try_recv(&self) -> Result<Option<MidiMessage>, MidiError> {
        Self::try_recv_inner(&self.state)
    }

    /// Transmits a message toward the device. The inherent counterpart of
    /// [`MidiOutput::send`].
    pub fn send(&self, message: MidiMessage) -> Result<(), MidiError> {
        Self::send_inner(&self.state, message)
    }

    /// Marks the port closed: later `try_recv`/`send` calls return
    /// [`MidiError::PortClosed`], like unplugging the cable.
    pub fn close(&self) {
        self.state.lock().expect("MIDI port poisoned").closed = true;
    }

    /// Whether the port has been closed.
    pub fn is_closed(&self) -> bool {
        self.state.lock().expect("MIDI port poisoned").closed
    }

    /// Number of messages waiting to be received.
    pub fn pending_count(&self) -> usize {
        self.state
            .lock()
            .expect("MIDI port poisoned")
            .input_queue
            .len()
    }

    /// Drains everything the code under test has transmitted, oldest first.
    pub fn drain_output(&self) -> Vec<MidiMessage> {
        self.state
            .lock()
            .expect("MIDI port poisoned")
            .output_queue
            .drain(..)
            .collect()
    }

    /// Number of transmitted messages waiting in `drain_output`.
    pub fn output_count(&self) -> usize {
        self.state
            .lock()
            .expect("MIDI port poisoned")
            .output_queue
            .len()
    }

    /// Splits the device into independent input/output handles.
    pub fn split(&self) -> (MockMidiInput, MockMidiOutput) {
        (
            MockMidiInput {
                state: Arc::clone(&self.state),
            },
            MockMidiOutput {
                state: Arc::clone(&self.state),
            },
        )
    }

    fn try_recv_inner(state: &Mutex<PortState>) -> Result<Option<MidiMessage>, MidiError> {
        let mut guard = state.lock().expect("MIDI port poisoned");
        if guard.closed {
            return Err(MidiError::PortClosed);
        }
        Ok(guard.input_queue.pop_front())
    }

    fn send_inner(state: &Mutex<PortState>, message: MidiMessage) -> Result<(), MidiError> {
        let mut guard = state.lock().expect("MIDI port poisoned");
        if guard.closed {
            return Err(MidiError::PortClosed);
        }
        guard.output_queue.push_back(message);
        Ok(())
    }
}

impl MidiInput for MockMidiDevice {
    fn try_recv(&mut self) -> Result<Option<MidiMessage>, MidiError> {
        Self::try_recv_inner(&self.state)
    }
}

impl MidiOutput for MockMidiDevice {
    fn send(&mut self, message: MidiMessage) -> Result<(), MidiError> {
        Self::send_inner(&self.state, message)
    }
}

/// Input-only handle: what a `MidiInput` consumer holds.
#[derive(Clone)]
pub struct MockMidiInput {
    state: Arc<Mutex<PortState>>,
}

impl MidiInput for MockMidiInput {
    fn try_recv(&mut self) -> Result<Option<MidiMessage>, MidiError> {
        MockMidiDevice::try_recv_inner(&self.state)
    }
}

/// Output-only handle: what a `MidiOutput` consumer holds.
#[derive(Clone)]
pub struct MockMidiOutput {
    state: Arc<Mutex<PortState>>,
}

impl MidiOutput for MockMidiOutput {
    fn send(&mut self, message: MidiMessage) -> Result<(), MidiError> {
        MockMidiDevice::send_inner(&self.state, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_builders_produce_wire_bytes() {
        assert_eq!(MidiMessage::note_on(0, 60, 100).bytes, vec![0x90, 60, 100]);
        assert_eq!(MidiMessage::note_on(15, 60, 100).bytes[0], 0x9F);
        assert_eq!(MidiMessage::note_off(2, 48, 64).bytes, vec![0x82, 48, 64]);
        assert_eq!(
            MidiMessage::control_change(1, 7, 127).bytes,
            vec![0xB1, 7, 127]
        );
        assert_eq!(MidiMessage::program_change(3, 42).bytes, vec![0xC3, 42]);
        assert_eq!(
            MidiMessage::pitch_bend(4, 8192).bytes,
            vec![0xE4, 0x00, 0x40]
        );
        assert_eq!(
            MidiMessage::sysex(&[0x7E, 0x09, 0x01]).bytes,
            vec![0xF0, 0x7E, 0x09, 0x01, 0xF7]
        );
    }

    #[test]
    fn message_inspection_helpers() {
        let note = MidiMessage::note_on(5, 60, 100);
        assert!(note.is_note_on());
        assert_eq!(note.channel(), Some(5));
        assert_eq!(MidiMessage::sysex(&[]).channel(), None);
        assert_eq!(MidiMessage::new(Vec::new()).status(), None);
    }

    #[test]
    fn injection_preserves_fifo_order_and_payloads() {
        let device = MockMidiDevice::new();
        device.inject_all([
            MidiMessage::with_timestamp(MidiMessage::note_on(0, 60, 1).bytes, 100),
            MidiMessage::with_timestamp(MidiMessage::note_off(0, 60, 0).bytes, 250),
        ]);
        assert_eq!(device.pending_count(), 2);

        let input = device.clone();
        let first = input.try_recv().unwrap().unwrap();
        assert_eq!(first.timestamp_us, 100);
        assert!(first.is_note_on());
        let second = input.try_recv().unwrap().unwrap();
        assert_eq!(second.timestamp_us, 250);
        assert_eq!(input.try_recv().unwrap(), None);
        assert_eq!(device.pending_count(), 0);
    }

    #[test]
    fn close_unplugs_the_port() {
        let device = MockMidiDevice::new();
        device.inject(MidiMessage::note_on(0, 60, 100));
        assert!(!device.is_closed());
        device.close();
        assert!(device.is_closed());
        let consumer = device;
        assert_eq!(consumer.try_recv(), Err(MidiError::PortClosed));
        assert_eq!(
            consumer.send(MidiMessage::note_off(0, 60, 0)),
            Err(MidiError::PortClosed)
        );
    }

    #[test]
    fn split_halves_route_independently() {
        let device = MockMidiDevice::new();
        let (mut input, mut output) = device.split();
        device.inject(MidiMessage::control_change(0, 1, 2));
        assert_eq!(input.try_recv().unwrap().unwrap().bytes, vec![0xB0, 1, 2]);
        output.send(MidiMessage::program_change(7, 9)).unwrap();
        let drained = device.drain_output();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].bytes, vec![0xC7, 9]);
        assert_eq!(device.output_count(), 0);
    }
}
