//! The notes of one take, binary, version 2, little-endian: the reply of the `take_notes` invoke.
//! A take never changes once made, so the screen keeps it by its id. The layout table is
//! docs/architecture.md 3-4; `ui/src/bridge/take.ts` reads it.

use std::fmt;

pub const TAKE_KIND: u8 = 2;
pub const TAKE_VERSION: u8 = 2;
pub const TAKE_HEADER_LEN: usize = 24;
pub const TAKE_NOTE_LEN: usize = 16;
pub const TAKE_CONTROL_LEN: usize = 8;

const CONTROL_CC: u8 = 0;
const CONTROL_BEND: u8 = 1;
/// `origin` in the binary when there is none. Take ids start at 1.
const NO_ORIGIN: u32 = 0;
/// `from` in the binary when there is none.
pub const TAKE_NO_FROM: u32 = u32::MAX;

/// Positions count from the take's tick 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakeNote {
    pub start: u32,
    pub length: u32,
    pub key: u8,
    pub velocity: u8,
    /// The index of the note of the origin take this one was made from; `None` for a note
    /// added by hand, or in a take with no origin.
    pub from: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeControlValue {
    Cc {
        number: u8,
        value: u8,
    },
    /// -8192 to 8191.
    PitchBend(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakeControl {
    pub tick: u32,
    pub value: TakeControlValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TakeNotes {
    pub id: u32,
    pub length: u32,
    /// The recorded take a quantized take was made from: the piano roll draws its notes as
    /// "そろえる前の位置". Never `Some(0)`.
    pub origin: Option<u32>,
    /// In order of start.
    pub notes: Vec<TakeNote>,
    /// In order of tick.
    pub controls: Vec<TakeControl>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeError {
    TooShort {
        len: usize,
    },
    WrongKind(u8),
    WrongVersion(u8),
    WrongControlKind(u8),
    /// The length does not match the counts in the header.
    WrongLength {
        len: usize,
        expected: usize,
    },
}

impl fmt::Display for TakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(f, "take is {len} bytes, shorter than the header"),
            Self::WrongKind(k) => write!(f, "take kind {k}, expected {TAKE_KIND}"),
            Self::WrongVersion(v) => write!(f, "take version {v}, expected {TAKE_VERSION}"),
            Self::WrongControlKind(k) => write!(f, "take control kind {k}"),
            Self::WrongLength { len, expected } => {
                write!(f, "take is {len} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for TakeError {}

impl TakeNotes {
    pub fn encoded_len(&self) -> usize {
        TAKE_HEADER_LEN + TAKE_NOTE_LEN * self.notes.len() + TAKE_CONTROL_LEN * self.controls.len()
    }

    /// # Panics
    /// When there are more than `u32::MAX` notes or controls.
    pub fn encode(&self) -> Vec<u8> {
        let count = |n: usize| u32::try_from(n).expect("count fits in u32");
        debug_assert_ne!(self.origin, Some(NO_ORIGIN), "take ids start at 1");
        let mut out = Vec::with_capacity(self.encoded_len());
        out.push(TAKE_KIND);
        out.push(TAKE_VERSION);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.length.to_le_bytes());
        out.extend_from_slice(&count(self.notes.len()).to_le_bytes());
        out.extend_from_slice(&count(self.controls.len()).to_le_bytes());
        out.extend_from_slice(&self.origin.unwrap_or(NO_ORIGIN).to_le_bytes());
        for n in &self.notes {
            out.extend_from_slice(&n.start.to_le_bytes());
            out.extend_from_slice(&n.length.to_le_bytes());
            out.extend_from_slice(&[n.key, n.velocity, 0, 0]);
            out.extend_from_slice(&n.from.unwrap_or(TAKE_NO_FROM).to_le_bytes());
        }
        for c in &self.controls {
            out.extend_from_slice(&c.tick.to_le_bytes());
            let (kind, number, value) = match c.value {
                TakeControlValue::Cc { number, value } => (CONTROL_CC, number, i16::from(value)),
                TakeControlValue::PitchBend(v) => (CONTROL_BEND, 0, v),
            };
            out.extend_from_slice(&[kind, number]);
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<TakeNotes, TakeError> {
        if bytes.len() < TAKE_HEADER_LEN {
            return Err(TakeError::TooShort { len: bytes.len() });
        }
        if bytes[0] != TAKE_KIND {
            return Err(TakeError::WrongKind(bytes[0]));
        }
        if bytes[1] != TAKE_VERSION {
            return Err(TakeError::WrongVersion(bytes[1]));
        }
        let u32_at = |pos: usize| u32::from_le_bytes(bytes[pos..pos + 4].try_into().expect("4"));
        let notes = u32_at(12) as usize;
        let controls = u32_at(16) as usize;
        let expected = notes
            .checked_mul(TAKE_NOTE_LEN)
            .and_then(|n| controls.checked_mul(TAKE_CONTROL_LEN)?.checked_add(n))
            .and_then(|n| n.checked_add(TAKE_HEADER_LEN))
            .unwrap_or(usize::MAX);
        if bytes.len() != expected {
            return Err(TakeError::WrongLength {
                len: bytes.len(),
                expected,
            });
        }
        let notes_at = TAKE_HEADER_LEN;
        let controls_at = notes_at + TAKE_NOTE_LEN * notes;
        Ok(TakeNotes {
            id: u32_at(4),
            length: u32_at(8),
            origin: Some(u32_at(20)).filter(|&o| o != NO_ORIGIN),
            notes: (0..notes)
                .map(|i| {
                    let p = notes_at + TAKE_NOTE_LEN * i;
                    TakeNote {
                        start: u32_at(p),
                        length: u32_at(p + 4),
                        key: bytes[p + 8],
                        velocity: bytes[p + 9],
                        from: Some(u32_at(p + 12)).filter(|&f| f != TAKE_NO_FROM),
                    }
                })
                .collect(),
            controls: (0..controls)
                .map(|i| {
                    let p = controls_at + TAKE_CONTROL_LEN * i;
                    let value = i16::from_le_bytes([bytes[p + 6], bytes[p + 7]]);
                    let value = match bytes[p + 4] {
                        CONTROL_CC => TakeControlValue::Cc {
                            number: bytes[p + 5],
                            value: value.clamp(0, 127) as u8,
                        },
                        CONTROL_BEND => TakeControlValue::PitchBend(value),
                        k => return Err(TakeError::WrongControlKind(k)),
                    };
                    Ok(TakeControl {
                        tick: u32_at(p),
                        value,
                    })
                })
                .collect::<Result<_, _>>()?,
        })
    }
}
