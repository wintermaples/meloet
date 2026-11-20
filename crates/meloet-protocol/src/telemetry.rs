//! Telemetry binary, version 3, little-endian. The layout table is docs/architecture.md 3-3;
//! this file is the one place that fixes it. `ui/src/bridge/telemetry.ts` reads it.

use std::fmt;

use crate::TransportState;

pub const TELEMETRY_KIND: u8 = 1;
pub const TELEMETRY_VERSION: u8 = 3;
pub const TELEMETRY_HEADER_LEN: usize = 88;

const FLAG_RUNNING: u8 = 1 << 0;
const FLAG_MASTER_CLIPPED: u8 = 1 << 1;
const FLAG_POSITION: u8 = 1 << 2;

/// What the listener hears at the send time. The screen goes on from it with `ticks_per_us`
/// until the next message: `tick + (now_us - sent_us) * ticks_per_us`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransportReading {
    /// Song position in ticks, with the fraction. Negative during a count-in from near 0.
    pub tick: f64,
    /// 0 while stopped.
    pub ticks_per_us: f64,
    pub state: TransportState,
    /// Clicks of the metronome still to come before recording starts, the one sounding
    /// included; 0 outside a count-in.
    pub count_in_beats_left: u16,
    /// Pass of the loop while recording, 0 for the first.
    pub lap: u32,
}

impl TransportState {
    fn to_u8(self) -> u8 {
        match self {
            TransportState::Stopped => 0,
            TransportState::CountIn => 1,
            TransportState::Playing => 2,
            TransportState::Recording => 3,
        }
    }

    fn from_u8(v: u8) -> Option<TransportState> {
        Some(match v {
            0 => TransportState::Stopped,
            1 => TransportState::CountIn,
            2 => TransportState::Playing,
            3 => TransportState::Recording,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Telemetry {
    pub seq: u32,
    /// CLOCK_MONOTONIC, nanoseconds, when Rust sent this.
    pub sent_ns: u64,
    /// Held keys 0..=63, bit n = note n.
    pub keys_low: u64,
    /// Held keys 64..=127, bit n = note 64 + n.
    pub keys_high: u64,
    /// 0 = none yet.
    pub last_velocity: u8,
    pub sustain: u8,
    pub modulation: u8,
    pub running: bool,
    pub master_clipped: bool,
    pub pitch_bend: i16,
    pub load_permille: u16,
    /// Left, right. Largest value since the last send; 1.0 = 0 dBFS.
    pub master_peak: [f32; 2],
    /// `None` before the audio has played anything.
    pub transport: Option<TransportReading>,
    /// Return A, then return B: left, right, as `master_peak`.
    pub return_peaks: [[f32; 2]; 2],
    pub track_peaks: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryError {
    TooShort {
        len: usize,
    },
    WrongKind(u8),
    WrongVersion(u8),
    WrongTransportState(u8),
    /// The length does not match the track count in the header.
    WrongLength {
        len: usize,
        expected: usize,
    },
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => {
                write!(f, "telemetry is {len} bytes, shorter than the header")
            }
            Self::WrongKind(k) => write!(f, "telemetry kind {k}, expected {TELEMETRY_KIND}"),
            Self::WrongVersion(v) => {
                write!(f, "telemetry version {v}, expected {TELEMETRY_VERSION}")
            }
            Self::WrongTransportState(v) => write!(f, "telemetry transport state {v}"),
            Self::WrongLength { len, expected } => {
                write!(f, "telemetry is {len} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for TelemetryError {}

impl Telemetry {
    pub fn encoded_len(&self) -> usize {
        TELEMETRY_HEADER_LEN + 8 * self.track_peaks.len()
    }

    /// Clears `out` and writes the message into it. The caller keeps one Vec and reuses it.
    ///
    /// # Panics
    /// When there are more than `u16::MAX` tracks.
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        let n = u16::try_from(self.track_peaks.len()).expect("track count fits in u16");
        out.clear();
        out.reserve(self.encoded_len());
        out.push(TELEMETRY_KIND);
        out.push(TELEMETRY_VERSION);
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        out.extend_from_slice(&self.sent_ns.to_le_bytes());
        out.extend_from_slice(&self.keys_low.to_le_bytes());
        out.extend_from_slice(&self.keys_high.to_le_bytes());
        out.push(self.last_velocity);
        out.push(self.sustain);
        out.push(self.modulation);
        let mut flags = 0;
        if self.running {
            flags |= FLAG_RUNNING;
        }
        if self.master_clipped {
            flags |= FLAG_MASTER_CLIPPED;
        }
        if self.transport.is_some() {
            flags |= FLAG_POSITION;
        }
        out.push(flags);
        out.extend_from_slice(&self.pitch_bend.to_le_bytes());
        out.extend_from_slice(&self.load_permille.to_le_bytes());
        for v in self.master_peak {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let t = self.transport.unwrap_or_default();
        out.extend_from_slice(&t.tick.to_le_bytes());
        out.extend_from_slice(&t.ticks_per_us.to_le_bytes());
        out.push(t.state.to_u8());
        out.push(0);
        out.extend_from_slice(&t.count_in_beats_left.to_le_bytes());
        out.extend_from_slice(&t.lap.to_le_bytes());
        for pair in self.return_peaks.iter().chain(&self.track_peaks) {
            for v in pair {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Telemetry, TelemetryError> {
        if bytes.len() < TELEMETRY_HEADER_LEN {
            return Err(TelemetryError::TooShort { len: bytes.len() });
        }
        if bytes[0] != TELEMETRY_KIND {
            return Err(TelemetryError::WrongKind(bytes[0]));
        }
        if bytes[1] != TELEMETRY_VERSION {
            return Err(TelemetryError::WrongVersion(bytes[1]));
        }
        let n = usize::from(u16::from_le_bytes(at(bytes, 2)));
        let expected = TELEMETRY_HEADER_LEN + 8 * n;
        if bytes.len() != expected {
            return Err(TelemetryError::WrongLength {
                len: bytes.len(),
                expected,
            });
        }
        let flags = bytes[35];
        let state = TransportState::from_u8(bytes[64])
            .ok_or(TelemetryError::WrongTransportState(bytes[64]))?;
        let pair = |pos: usize| {
            [
                f32::from_le_bytes(at(bytes, pos)),
                f32::from_le_bytes(at(bytes, pos + 4)),
            ]
        };
        Ok(Telemetry {
            seq: u32::from_le_bytes(at(bytes, 4)),
            sent_ns: u64::from_le_bytes(at(bytes, 8)),
            keys_low: u64::from_le_bytes(at(bytes, 16)),
            keys_high: u64::from_le_bytes(at(bytes, 24)),
            last_velocity: bytes[32],
            sustain: bytes[33],
            modulation: bytes[34],
            running: flags & FLAG_RUNNING != 0,
            master_clipped: flags & FLAG_MASTER_CLIPPED != 0,
            pitch_bend: i16::from_le_bytes(at(bytes, 36)),
            load_permille: u16::from_le_bytes(at(bytes, 38)),
            master_peak: pair(40),
            transport: (flags & FLAG_POSITION != 0).then(|| TransportReading {
                tick: f64::from_le_bytes(at(bytes, 48)),
                ticks_per_us: f64::from_le_bytes(at(bytes, 56)),
                state,
                count_in_beats_left: u16::from_le_bytes(at(bytes, 66)),
                lap: u32::from_le_bytes(at(bytes, 68)),
            }),
            return_peaks: [pair(72), pair(80)],
            track_peaks: (0..n).map(|i| pair(TELEMETRY_HEADER_LEN + 8 * i)).collect(),
        })
    }

    /// True when everything but the sequence number and the send time is the same.
    /// The sender skips a message that would repeat the previous one.
    pub fn same_readings(&self, other: &Telemetry) -> bool {
        self.keys_low == other.keys_low
            && self.keys_high == other.keys_high
            && self.last_velocity == other.last_velocity
            && self.sustain == other.sustain
            && self.modulation == other.modulation
            && self.running == other.running
            && self.master_clipped == other.master_clipped
            && self.pitch_bend == other.pitch_bend
            && self.load_permille == other.load_permille
            && self.master_peak == other.master_peak
            && self.transport == other.transport
            && self.return_peaks == other.return_peaks
            && self.track_peaks == other.track_peaks
    }
}

fn at<const N: usize>(bytes: &[u8], pos: usize) -> [u8; N] {
    bytes[pos..pos + N].try_into().expect("length checked")
}
