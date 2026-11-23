//! Standard MIDI files (format 0 and 1, ticks per quarter note): a reader that gives the
//! notes, names, tempos and meters, and a writer of format 1. No file access here.
//!
//! A note-off names only its channel and key, so of several notes of one key sounding at
//! once the earliest started ends first, here as in most readers. The writer orders its
//! events so that the reader gives back what was written, but for a note that lies wholly
//! inside another of its key: those two come back with their ends swapped.

use std::fmt;

use meloet_model::TICKS_PER_QUARTER;

/// A note of a file, in the file's ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmfNote {
    pub start: u64,
    /// At `start` or after it.
    pub end: u64,
    /// 0 to 15.
    pub channel: u8,
    pub key: u8,
    /// 1 to 127.
    pub velocity: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SmfTrack {
    pub name: Option<String>,
    /// In order of start, then key.
    pub notes: Vec<SmfNote>,
    /// Notes never ended in the file: they end where the track ends.
    pub hanging: u32,
    /// The tick of the track's last event.
    pub length: u64,
    /// Tick and microseconds a quarter note.
    pub tempos: Vec<(u64, u32)>,
    /// Tick, numerator, and the denominator as a power of two.
    pub meters: Vec<(u64, u8, u8)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Smf {
    pub format: u16,
    /// Ticks a quarter note.
    pub division: u16,
    pub tracks: Vec<SmfTrack>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmfError {
    NotMidi,
    /// Format 2 (patterns that do not play together), or a format that is not known.
    Format(u16),
    /// Time in frames a second instead of ticks a quarter note.
    Smpte,
    NoDivision,
    /// The file ends in the middle of track `track` (from 1).
    Truncated {
        track: usize,
    },
    /// Track `track` (from 1) holds something that is no MIDI event at byte `at` of the file.
    BadEvent {
        track: usize,
        at: usize,
    },
}

impl fmt::Display for SmfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmfError::NotMidi => write!(f, "it is not a standard MIDI file (no MThd header)"),
            SmfError::Format(2) => write!(
                f,
                "it is a format 2 MIDI file (separate patterns); only formats 0 and 1 can be read"
            ),
            SmfError::Format(n) => write!(f, "its MIDI format {n} is not known (0 and 1 are)"),
            SmfError::Smpte => write!(
                f,
                "its time is in SMPTE frames, not in ticks a quarter note; that cannot be read"
            ),
            SmfError::NoDivision => write!(f, "it says 0 ticks a quarter note"),
            SmfError::Truncated { track } => {
                write!(f, "the file ends in the middle of its track {track}")
            }
            SmfError::BadEvent { track, at } => write!(
                f,
                "its track {track} is broken at byte {at} (not a MIDI event)"
            ),
        }
    }
}

/// A tick of a file with `division` ticks a quarter note as a tick of meloet, the nearest
/// (a half goes up).
pub fn scaled(tick: u64, division: u16) -> u64 {
    let d = u128::from(division.max(1));
    let t = (u128::from(tick) * u128::from(TICKS_PER_QUARTER) + d / 2) / d;
    u64::try_from(t).unwrap_or(u64::MAX)
}

/// A tick of meloet as a tick of a file with `division` ticks a quarter note.
pub fn unscaled(tick: u64, division: u16) -> u64 {
    let q = u128::from(TICKS_PER_QUARTER);
    let t = (u128::from(tick) * u128::from(division) + q / 2) / q;
    u64::try_from(t).unwrap_or(u64::MAX)
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    /// Where the chunk being read ends.
    end: usize,
    track: usize,
}

impl<'a> Reader<'a> {
    fn bad(&self) -> SmfError {
        SmfError::BadEvent {
            track: self.track,
            at: self.at,
        }
    }

    fn byte(&mut self) -> Result<u8, SmfError> {
        if self.at >= self.end {
            return Err(self.bad());
        }
        let b = self.data[self.at];
        self.at += 1;
        Ok(b)
    }

    /// A data byte: below 0x80.
    fn data_byte(&mut self) -> Result<u8, SmfError> {
        match self.byte()? {
            b if b < 0x80 => Ok(b),
            _ => {
                self.at -= 1;
                Err(self.bad())
            }
        }
    }

    /// A variable-length quantity: up to 4 bytes of 7 bits.
    fn quantity(&mut self) -> Result<u32, SmfError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let b = self.byte()?;
            value = value << 7 | u32::from(b & 0x7F);
            if b < 0x80 {
                return Ok(value);
            }
        }
        Err(self.bad())
    }

    fn bytes(&mut self, count: usize) -> Result<&'a [u8], SmfError> {
        if count > self.end - self.at {
            return Err(self.bad());
        }
        let bytes = &self.data[self.at..self.at + count];
        self.at += count;
        Ok(bytes)
    }
}

fn name_of(bytes: &[u8]) -> String {
    (String::from_utf8_lossy(bytes).chars())
        .filter(|c| !c.is_control())
        .take(64)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn read_track(r: &mut Reader) -> Result<SmfTrack, SmfError> {
    let mut track = SmfTrack::default();
    // Notes that sound, by channel and key: the start and velocity of each, the earliest first.
    let mut open: Vec<Vec<(u64, u8)>> = vec![Vec::new(); 16 * 128];
    let mut tick = 0u64;
    let mut running = None;
    while r.at < r.end {
        tick = tick.saturating_add(u64::from(r.quantity()?));
        let first = r.byte()?;
        let status = if first < 0x80 {
            // Running status: the event has the status of the one before.
            r.at -= 1;
            running.ok_or_else(|| r.bad())?
        } else {
            first
        };
        match status {
            0x80..=0xEF => {
                running = Some(status);
                let a = r.data_byte()?;
                let b = match status & 0xF0 {
                    0xC0 | 0xD0 => 0,
                    _ => r.data_byte()?,
                };
                let channel = status & 0x0F;
                let sounding = &mut open[usize::from(channel) * 128 + usize::from(a)];
                match status & 0xF0 {
                    0x90 if b > 0 => sounding.push((tick, b)),
                    0x80 | 0x90 if !sounding.is_empty() => {
                        let (start, velocity) = sounding.remove(0);
                        track.notes.push(SmfNote {
                            start,
                            end: tick,
                            channel,
                            key: a,
                            velocity,
                        });
                    }
                    _ => {}
                }
            }
            0xF0 | 0xF7 => {
                let length = r.quantity()? as usize;
                r.bytes(length)?;
            }
            0xFF => {
                let kind = r.byte()?;
                let length = r.quantity()? as usize;
                let data = r.bytes(length)?;
                match (kind, data) {
                    (0x2F, _) => break,
                    (0x03, name) if track.name.is_none() => track.name = Some(name_of(name)),
                    (0x51, &[a, b, c, ..]) => {
                        let us = u32::from(a) << 16 | u32::from(b) << 8 | u32::from(c);
                        track.tempos.push((tick, us));
                    }
                    (0x58, &[num, power, ..]) => track.meters.push((tick, num, power)),
                    _ => {}
                }
            }
            _ => {
                r.at -= 1;
                return Err(r.bad());
            }
        }
    }
    track.length = tick;
    for (i, sounding) in open.iter().enumerate() {
        for &(start, velocity) in sounding {
            track.hanging += 1;
            track.notes.push(SmfNote {
                start,
                end: tick,
                channel: (i / 128) as u8,
                key: (i % 128) as u8,
                velocity,
            });
        }
    }
    track.notes.sort_by_key(|n| (n.start, n.key));
    Ok(track)
}

fn be32(b: &[u8]) -> usize {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
}

pub fn read(data: &[u8]) -> Result<Smf, SmfError> {
    if data.len() < 14 || &data[..4] != b"MThd" {
        return Err(SmfError::NotMidi);
    }
    let header = be32(&data[4..8]);
    if header < 6 || header > data.len() - 8 {
        return Err(SmfError::NotMidi);
    }
    let format = u16::from_be_bytes([data[8], data[9]]);
    let division = u16::from_be_bytes([data[12], data[13]]);
    if format > 1 {
        return Err(SmfError::Format(format));
    }
    if division & 0x8000 != 0 {
        return Err(SmfError::Smpte);
    }
    if division == 0 {
        return Err(SmfError::NoDivision);
    }
    let mut tracks = Vec::new();
    let mut at = 8 + header;
    // Chunks that are no tracks are passed over, as the standard asks.
    while data.len() - at >= 8 {
        let length = be32(&data[at + 4..at + 8]);
        let is_track = &data[at..at + 4] == b"MTrk";
        at += 8;
        if length > data.len() - at {
            return Err(SmfError::Truncated {
                track: tracks.len() + 1,
            });
        }
        if is_track {
            let mut r = Reader {
                data,
                at,
                end: at + length,
                track: tracks.len() + 1,
            };
            tracks.push(read_track(&mut r)?);
        }
        at += length;
    }
    Ok(Smf {
        format,
        division,
        tracks,
    })
}

/// A note to write, in the file's ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutNote {
    pub start: u64,
    /// 1 or more.
    pub length: u64,
    pub key: u8,
    pub velocity: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutTrack {
    pub name: String,
    /// 0 to 15.
    pub channel: u8,
    pub notes: Vec<OutNote>,
}

/// A format 1 file: a first track with the name, the tempos and the meters, then the tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutFile {
    /// Ticks a quarter note, below 0x8000.
    pub division: u16,
    pub name: String,
    /// Tick and microseconds a quarter note, in order of tick.
    pub tempos: Vec<(u64, u32)>,
    /// Tick, numerator and denominator (2, 4, 8, 16 …), in order of tick.
    pub meters: Vec<(u64, u8, u8)>,
    /// Every track ends here, or where its last note ends when that is later.
    pub length: u64,
    pub tracks: Vec<OutTrack>,
}

/// The longest time between two events, and so the longest file, in ticks.
pub const MAX_OUT_TICKS: u64 = 0x0FFF_FFFF;

fn put_quantity(out: &mut Vec<u8>, value: u64) {
    let value = value.min(MAX_OUT_TICKS) as u32;
    for shift in [21, 14, 7] {
        if value >> shift != 0 {
            out.push((value >> shift) as u8 & 0x7F | 0x80);
        }
    }
    out.push(value as u8 & 0x7F);
}

fn put_meta(out: &mut Vec<u8>, kind: u8, data: &[u8]) {
    out.extend([0xFF, kind]);
    put_quantity(out, data.len() as u64);
    out.extend(data);
}

/// One track chunk from events in order: tick and the event's bytes.
fn put_track(out: &mut Vec<u8>, events: &[(u64, Vec<u8>)], length: u64) {
    let mut body = Vec::new();
    let mut tick = 0;
    for (at, bytes) in events {
        put_quantity(&mut body, at - tick);
        body.extend(bytes);
        tick = *at;
    }
    put_quantity(&mut body, length.max(tick) - tick);
    put_meta(&mut body, 0x2F, &[]);
    out.extend(b"MTrk");
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(body);
}

fn name_event(name: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    put_meta(&mut bytes, 0x03, name.as_bytes());
    bytes
}

pub fn write(file: &OutFile) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(b"MThd");
    out.extend(6u32.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend((file.tracks.len() as u16 + 1).to_be_bytes());
    out.extend(file.division.to_be_bytes());

    let mut events = vec![(0, name_event(&file.name))];
    for &(tick, num, den) in &file.meters {
        let power = den.max(1).ilog2() as u8;
        // MIDI clocks a beat (24 a quarter note), and 8 notated 32nds a quarter note.
        let clocks = (96 / u32::from(den.max(1))).max(1) as u8;
        let mut bytes = Vec::new();
        put_meta(&mut bytes, 0x58, &[num, power, clocks, 8]);
        events.push((tick, bytes));
    }
    for &(tick, us) in &file.tempos {
        let us = us.clamp(1, 0xFF_FFFF).to_be_bytes();
        let mut bytes = Vec::new();
        put_meta(&mut bytes, 0x51, &us[1..]);
        events.push((tick, bytes));
    }
    events.sort_by_key(|(tick, _)| *tick);
    put_track(&mut out, &events, file.length);

    for track in &file.tracks {
        let channel = track.channel & 0x0F;
        // Of notes of one key that start together the shorter is written first, so that the
        // reader, which ends the earliest first, gives each its own end.
        let mut notes: Vec<&OutNote> = track.notes.iter().collect();
        notes.sort_by_key(|n| (n.start, n.key, n.length));
        // At one tick the ends come before the starts: a note that follows another of its
        // key at once is not ended by it.
        let mut keyed = Vec::with_capacity(notes.len() * 2);
        for (i, n) in notes.iter().enumerate() {
            let on = vec![0x90 | channel, n.key & 0x7F, n.velocity.clamp(1, 127)];
            let off = vec![0x80 | channel, n.key & 0x7F, 0x40];
            keyed.push((n.start, 1, i, on));
            keyed.push((n.start + n.length.max(1), 0, i, off));
        }
        keyed.sort_by_key(|&(tick, order, i, _)| (tick, order, i));
        let mut events = vec![(0, name_event(&track.name))];
        events.extend(keyed.into_iter().map(|(tick, _, _, bytes)| (tick, bytes)));
        put_track(&mut out, &events, file.length);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = kind.to_vec();
        out.extend((body.len() as u32).to_be_bytes());
        out.extend(body);
        out
    }

    fn file(format: u16, division: u16, tracks: &[&[u8]]) -> Vec<u8> {
        let mut header = format.to_be_bytes().to_vec();
        header.extend((tracks.len() as u16).to_be_bytes());
        header.extend(division.to_be_bytes());
        let mut out = chunk(b"MThd", &header);
        for track in tracks {
            out.extend(chunk(b"MTrk", track));
        }
        out
    }

    fn note(start: u64, end: u64, channel: u8, key: u8, velocity: u8) -> SmfNote {
        SmfNote {
            start,
            end,
            channel,
            key,
            velocity,
        }
    }

    const END: [u8; 4] = [0x00, 0xFF, 0x2F, 0x00];

    #[test]
    fn a_hand_made_file_with_every_kind_of_event() {
        #[rustfmt::skip]
        let track: &[u8] = &[
            0x00, 0xFF, 0x03, 0x04, b'B', b'a', b's', b's',
            0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20,
            0x00, 0xFF, 0x58, 0x04, 0x03, 0x02, 0x18, 0x08,
            // A sysex, a program change (one data byte) and a pitch bend between the notes.
            0x00, 0xF0, 0x03, 0x01, 0x02, 0xF7,
            0x00, 0xC0, 0x05,
            0x00, 0xE0, 0x00, 0x40,
            0x00, 0x90, 0x3C, 0x64,
            // Running status: another note-on, then a note-on of velocity 0, which ends C4.
            0x30, 0x3E, 0x50,
            0x30, 0x3C, 0x00,
            0x00, 0x99, 0x24, 0x7F,
            // A delta of two bytes (192), and a real note-off.
            0x81, 0x40, 0x80, 0x3E, 0x40,
            0x00, 0x90, 0x40, 0x01,
            0x60, 0xFF, 0x2F, 0x00,
        ];
        let read = read(&file(0, 96, &[track])).unwrap();
        assert_eq!((read.format, read.division), (0, 96));
        assert_eq!(
            read.tracks,
            [SmfTrack {
                name: Some("Bass".into()),
                // The drum hit and the last E4 never end: they end with the track.
                notes: vec![
                    note(0, 96, 0, 60, 100),
                    note(48, 288, 0, 62, 80),
                    note(96, 384, 9, 36, 127),
                    note(288, 384, 0, 64, 1),
                ],
                hanging: 2,
                length: 384,
                tempos: vec![(0, 500_000)],
                meters: vec![(0, 3, 2)],
            }]
        );
    }

    #[test]
    fn running_status_goes_on_after_a_meta_event_and_other_chunks_are_passed_over() {
        #[rustfmt::skip]
        let track: &[u8] = &[
            0x00, 0x90, 0x3C, 0x64,
            0x00, 0xFF, 0x01, 0x02, b'h', b'i',
            0x10, 0x3C, 0x00,
            0x00, 0xFF, 0x2F, 0x00,
        ];
        let mut bytes = file(1, 480, &[&END]);
        bytes.extend(chunk(b"XFIH", &[1, 2, 3]));
        bytes.extend(chunk(b"MTrk", track));
        // Less than a chunk's head at the end is not looked at.
        bytes.extend([0, 0, 0]);
        let read = read(&bytes).unwrap();
        assert_eq!(read.tracks.len(), 2);
        assert_eq!(read.tracks[0], SmfTrack::default());
        assert_eq!(read.tracks[1].notes, [note(0, 16, 0, 60, 100)]);
    }

    #[test]
    fn notes_of_one_key_end_in_the_order_they_started() {
        #[rustfmt::skip]
        let track: &[u8] = &[
            0x00, 0x90, 0x3C, 0x10,
            0x0A, 0x90, 0x3C, 0x20,
            0x0A, 0x80, 0x3C, 0x00,
            // An end with no note sounding does nothing.
            0x00, 0x80, 0x3D, 0x00,
            0x0A, 0x80, 0x3C, 0x00,
        ];
        let read = read(&file(0, 96, &[track])).unwrap();
        assert_eq!(
            read.tracks[0].notes,
            [note(0, 20, 0, 60, 0x10), note(10, 30, 0, 60, 0x20)]
        );
        // No end of track: the track ends at its last event.
        assert_eq!((read.tracks[0].length, read.tracks[0].hanging), (30, 0));
    }

    #[test]
    fn what_cannot_be_read_says_why() {
        assert_eq!(read(b"RIFF....RMID"), Err(SmfError::NotMidi));
        assert_eq!(read(&[]), Err(SmfError::NotMidi));
        assert_eq!(read(&file(2, 96, &[&END])), Err(SmfError::Format(2)));
        assert_eq!(read(&file(7, 96, &[&END])), Err(SmfError::Format(7)));
        // 25 frames a second, 40 ticks a frame.
        assert_eq!(read(&file(1, 0xE728, &[&END])), Err(SmfError::Smpte));
        assert_eq!(read(&file(1, 0, &[&END])), Err(SmfError::NoDivision));
        let mut cut = file(1, 96, &[&END, &[0x00, 0x90, 0x3C, 0x64]]);
        cut.truncate(cut.len() - 2);
        assert_eq!(read(&cut), Err(SmfError::Truncated { track: 2 }));
        // A data byte with no status before it; a status where a data byte must be; a
        // status that is no event of a file; an event that runs past its track.
        for (track, at) in [
            (&[0x00, 0x3C, 0x64][..], 23),
            (&[0x00, 0x90, 0x3C, 0x90], 25),
            (&[0x00, 0xF8], 23),
            (&[0x00, 0x90, 0x3C], 25),
            (&[0x00, 0xFF, 0x03, 0x05, b'a'], 26),
            (&[0xFF, 0xFF, 0xFF, 0xFF, 0x00], 26),
        ] {
            assert_eq!(
                read(&file(0, 96, &[track])),
                Err(SmfError::BadEvent { track: 1, at }),
                "{track:02X?}"
            );
        }
        assert!(SmfError::Format(2).to_string().contains("format 2"));
        assert!(SmfError::Smpte.to_string().contains("SMPTE"));
    }

    #[test]
    fn ticks_are_scaled_to_the_nearest() {
        for (division, per_tick) in [(96, 20), (480, 4), (960, 2), (1920, 1), (120, 16)] {
            for tick in [0, 1, 7, 95, 12_345] {
                assert_eq!(scaled(tick, division), tick * per_tick, "{division}");
            }
        }
        // 100 does not divide 1920: 19.2 ticks each.
        let hundred: Vec<u64> = (0..=5).map(|t| scaled(t, 100)).collect();
        assert_eq!(hundred, [0, 19, 38, 58, 77, 96]);
        assert_eq!(scaled(100, 100), 1920);
        assert_eq!(scaled(12_345, 100), 237_024);
        // Finer than meloet: a half goes up.
        assert_eq!([1, 2, 3, 4, 5].map(|t| scaled(t, 3840)), [1, 1, 2, 2, 3]);
        assert_eq!(scaled(u64::MAX, 1), u64::MAX);
        assert_eq!([0, 1, 2, 3, 4].map(|t| unscaled(t, 960)), [0, 1, 1, 2, 2]);
        assert_eq!(
            [0, 1, 2, 3, 4, 5].map(|t| unscaled(t, 480)),
            [0, 0, 1, 1, 1, 1]
        );
        assert_eq!(unscaled(12_345, 1920), 12_345);
    }

    #[test]
    fn quantities_take_one_to_four_bytes() {
        for (value, bytes) in [
            (0u64, vec![0x00]),
            (0x7F, vec![0x7F]),
            (0x80, vec![0x81, 0x00]),
            (0x3FFF, vec![0xFF, 0x7F]),
            (0x4000, vec![0x81, 0x80, 0x00]),
            (0x1F_FFFF, vec![0xFF, 0xFF, 0x7F]),
            (0x20_0000, vec![0x81, 0x80, 0x80, 0x00]),
            (0x0FFF_FFFF, vec![0xFF, 0xFF, 0xFF, 0x7F]),
        ] {
            let mut out = Vec::new();
            put_quantity(&mut out, value);
            assert_eq!(out, bytes, "{value:#X}");
            let mut r = Reader {
                data: &out,
                at: 0,
                end: out.len(),
                track: 1,
            };
            assert_eq!(u64::from(r.quantity().unwrap()), value);
        }
    }

    fn out(start: u64, length: u64, key: u8, velocity: u8) -> OutNote {
        OutNote {
            start,
            length,
            key,
            velocity,
        }
    }

    #[test]
    fn what_is_written_is_read_back() {
        let notes = vec![
            // Two of one key starting together, the longer given first.
            out(0, 300, 60, 90),
            out(0, 100, 60, 50),
            // One right after another of its key, and one crossing it.
            out(300, 100, 60, 70),
            out(350, 100, 60, 71),
            out(120, 1, 0, 1),
            out(0x20_0000, 5, 127, 127),
        ];
        let written = OutFile {
            division: 1920,
            name: "a song ♪".into(),
            tempos: vec![(0, 500_000), (1920, 400_000)],
            meters: vec![(0, 4, 4), (7680, 6, 8)],
            length: 0x30_0000,
            tracks: vec![
                OutTrack {
                    name: "ベース".into(),
                    channel: 0,
                    notes: notes.clone(),
                },
                OutTrack {
                    name: "drums".into(),
                    channel: 9,
                    notes: vec![out(10, 20, 36, 100)],
                },
                OutTrack {
                    name: String::new(),
                    channel: 0,
                    notes: Vec::new(),
                },
            ],
        };
        let bytes = write(&written);
        let read = read(&bytes).unwrap();
        assert_eq!(
            (read.format, read.division, read.tracks.len()),
            (1, 1920, 4)
        );
        let conductor = &read.tracks[0];
        assert_eq!(conductor.name.as_deref(), Some("a song ♪"));
        assert_eq!(conductor.tempos, written.tempos);
        assert_eq!(conductor.meters, [(0, 4, 2), (7680, 6, 3)]);
        assert_eq!(conductor.length, 0x30_0000);
        let bass = &read.tracks[1];
        assert_eq!(bass.name.as_deref(), Some("ベース"));
        let mut wanted: Vec<SmfNote> = (notes.iter())
            .map(|n| note(n.start, n.start + n.length, 0, n.key, n.velocity))
            .collect();
        wanted.sort_by_key(|n| (n.start, n.key, n.end));
        let mut got = bass.notes.clone();
        got.sort_by_key(|n| (n.start, n.key, n.end));
        assert_eq!(got, wanted);
        assert_eq!((bass.hanging, bass.length), (0, 0x30_0000));
        assert_eq!(read.tracks[2].notes, [note(10, 30, 9, 36, 100)]);
        assert_eq!(read.tracks[3].notes, []);
        assert_eq!(read.tracks[3].name.as_deref(), Some(""));
        // A track whose last note ends after the length ends there.
        let long = OutFile {
            length: 10,
            ..written
        };
        assert_eq!(read_len(&write(&long), 1), 0x20_0005);
    }

    fn read_len(bytes: &[u8], track: usize) -> u64 {
        read(bytes).unwrap().tracks[track].length
    }
}
