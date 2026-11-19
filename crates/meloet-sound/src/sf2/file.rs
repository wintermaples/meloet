//! The parts of a SoundFont 2 file (RIFF `sfbk`) that are read when it is opened: the version
//! and the tables of `pdta`. The samples stay on disk. Section numbers are those of the
//! SoundFont 2.01 specification.
//!
//! The file comes from the user, so nothing in it is trusted: every size and place is checked
//! against the file's real length before it is used.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use super::Sf2Error;

/// A file with more chunks than this in one list is not read further.
const MAX_CHUNKS: usize = 4096;
/// The tables of the biggest SoundFonts are a few MiB.
const MAX_PDTA_BYTES: u64 = 256 * 1024 * 1024;
const MAX_INFO_BYTES: u64 = 1024 * 1024;

/// 7.10 sfSampleType
pub(super) const ROM_SAMPLE: u16 = 0x8000;
/// SoundFont 3 (not part of the specification): the sample is Ogg Vorbis.
const COMPRESSED_SAMPLE: u16 = 0x10;

pub(super) struct PresetHeader {
    pub name: String,
    pub program: u16,
    pub bank: u16,
    pub bag: u16,
}

pub(super) struct InstHeader {
    pub name: String,
    pub bag: u16,
}

pub(super) struct SampleHeader {
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub loop_start: u32,
    pub loop_end: u32,
    pub rate: u32,
    pub original_pitch: u8,
    pub correction: i8,
    pub link: u16,
    pub kind: u16,
}

pub(super) struct Tables {
    /// With the terminal record, whose `bag` ends the last preset.
    pub presets: Vec<PresetHeader>,
    /// The first generator of each zone.
    pub pbag: Vec<u16>,
    /// (generator, amount)
    pub pgen: Vec<(u16, u16)>,
    /// With the terminal record.
    pub insts: Vec<InstHeader>,
    pub ibag: Vec<u16>,
    pub igen: Vec<(u16, u16)>,
    /// Without the terminal record.
    pub samples: Vec<SampleHeader>,
    /// Where the 16-bit samples start in the file, in bytes.
    pub smpl_at: u64,
    /// How many samples are really there.
    pub smpl_len: u64,
}

struct Chunk {
    id: [u8; 4],
    body: u64,
    size: u64,
}

/// The chunks in `start..end` of the file. A chunk that says it goes past `end` is cut there.
fn chunks(file: &mut File, start: u64, end: u64) -> Result<Vec<Chunk>, Sf2Error> {
    let mut found = Vec::new();
    let mut pos = start;
    while pos.saturating_add(8) <= end && found.len() < MAX_CHUNKS {
        let mut head = [0_u8; 8];
        read_at(file, pos, &mut head)?;
        let size = u64::from(u32::from_le_bytes([head[4], head[5], head[6], head[7]]));
        let body = pos + 8;
        found.push(Chunk {
            id: [head[0], head[1], head[2], head[3]],
            body,
            size: size.min(end - body),
        });
        pos = body.saturating_add(size).saturating_add(size & 1);
    }
    Ok(found)
}

fn read_at(file: &mut File, at: u64, into: &mut [u8]) -> Result<(), Sf2Error> {
    file.seek(SeekFrom::Start(at))
        .and_then(|_| file.read_exact(into))
        .map_err(|e| Sf2Error::Io(e.to_string()))
}

fn read_chunk(file: &mut File, chunk: &Chunk) -> Result<Vec<u8>, Sf2Error> {
    let mut bytes = vec![0_u8; chunk.size as usize];
    read_at(file, chunk.body, &mut bytes)?;
    Ok(bytes)
}

/// A name of 20 bytes ends at its first zero. The bytes are taken as Latin-1.
fn name(bytes: &[u8]) -> String {
    let text: String = bytes
        .iter()
        .take_while(|b| **b != 0)
        .map(|b| {
            if b.is_ascii_control() {
                ' '
            } else {
                char::from(*b)
            }
        })
        .collect();
    text.trim().to_string()
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn broken(what: &str) -> Sf2Error {
    Sf2Error::NotSf2(what.to_string())
}

pub(super) fn read(file: &mut File) -> Result<Tables, Sf2Error> {
    let length = file
        .metadata()
        .map_err(|e| Sf2Error::Io(e.to_string()))?
        .len();
    if length < 12 {
        return Err(broken("the file is too short"));
    }
    let mut head = [0_u8; 12];
    read_at(file, 0, &mut head)?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"sfbk" {
        return Err(broken("not a RIFF file of the kind sfbk"));
    }
    let end = length.min(8 + u64::from(u32_at(&head, 4)));

    let (mut info, mut sdta, mut pdta) = (None, None, None);
    for chunk in chunks(file, 12, end)? {
        if &chunk.id != b"LIST" || chunk.size < 4 {
            continue;
        }
        let mut kind = [0_u8; 4];
        read_at(file, chunk.body, &mut kind)?;
        let inside = (chunk.body + 4, chunk.body + chunk.size);
        match &kind {
            b"INFO" => info = info.or(Some(inside)),
            b"sdta" => sdta = sdta.or(Some(inside)),
            b"pdta" => pdta = pdta.or(Some(inside)),
            _ => {}
        }
    }

    // 5.1 ifil: the version of the format.
    if let Some((start, end)) = info {
        for chunk in chunks(file, start, end)? {
            if &chunk.id == b"ifil" && (4..=MAX_INFO_BYTES).contains(&chunk.size) {
                let bytes = read_chunk(file, &chunk)?;
                match (u16_at(&bytes, 0), u16_at(&bytes, 2)) {
                    (2, _) => {}
                    (3, _) => return Err(Sf2Error::Sf3),
                    (major, minor) => {
                        return Err(Sf2Error::NotSf2(format!(
                            "the file says it is version {major}.{minor}; this meloet reads version 2"
                        )));
                    }
                }
            }
        }
    }

    // 6.1 smpl: the 16-bit samples. sm24 (the low byte of 24-bit samples) is not read.
    let (start, end) = sdta.ok_or_else(|| broken("no sample data (sdta)"))?;
    let smpl = chunks(file, start, end)?
        .into_iter()
        .find(|c| &c.id == b"smpl")
        .ok_or_else(|| broken("no sample data (smpl)"))?;
    if smpl.size >= 4 {
        let mut first = [0_u8; 4];
        read_at(file, smpl.body, &mut first)?;
        if &first == b"OggS" {
            return Err(Sf2Error::Sf3);
        }
    }

    // 7.1 the tables of pdta, records of a fixed size.
    let (start, end) = pdta.ok_or_else(|| broken("no preset data (pdta)"))?;
    if end - start > MAX_PDTA_BYTES {
        return Err(broken("the preset data is too big"));
    }
    let parts = chunks(file, start, end)?;
    let mut table = |id: &[u8; 4]| -> Result<Vec<u8>, Sf2Error> {
        let chunk = parts
            .iter()
            .find(|c| &c.id == id)
            .ok_or_else(|| Sf2Error::NotSf2(format!("no {} table", String::from_utf8_lossy(id))))?;
        read_chunk(file, chunk)
    };
    let presets: Vec<PresetHeader> = table(b"phdr")?
        .as_chunks::<38>()
        .0
        .iter()
        .map(|r| PresetHeader {
            name: name(&r[..20]),
            program: u16_at(r, 20),
            bank: u16_at(r, 22),
            bag: u16_at(r, 24),
        })
        .collect();
    let bags = |bytes: Vec<u8>| {
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|r| u16_at(r, 0))
            .collect()
    };
    let gens = |bytes: Vec<u8>| {
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|r| (u16_at(r, 0), u16_at(r, 2)))
            .collect()
    };
    let pbag = bags(table(b"pbag")?);
    let pgen = gens(table(b"pgen")?);
    let insts = table(b"inst")?
        .as_chunks::<22>()
        .0
        .iter()
        .map(|r| InstHeader {
            name: name(&r[..20]),
            bag: u16_at(r, 20),
        })
        .collect();
    let ibag = bags(table(b"ibag")?);
    let igen = gens(table(b"igen")?);
    let mut samples: Vec<SampleHeader> = table(b"shdr")?
        .as_chunks::<46>()
        .0
        .iter()
        .map(|r| SampleHeader {
            name: name(&r[..20]),
            start: u32_at(r, 20),
            end: u32_at(r, 24),
            loop_start: u32_at(r, 28),
            loop_end: u32_at(r, 32),
            rate: u32_at(r, 36),
            original_pitch: r[40],
            correction: r[41] as i8,
            link: u16_at(r, 42),
            kind: u16_at(r, 44),
        })
        .collect();
    // 7.10: the last record is the terminal one.
    samples.pop();
    if samples.iter().any(|s| s.kind & COMPRESSED_SAMPLE != 0) {
        return Err(Sf2Error::Sf3);
    }
    if presets.len() < 2 {
        return Err(broken("no preset in the file"));
    }

    Ok(Tables {
        presets,
        pbag,
        pgen,
        insts,
        ibag,
        igen,
        samples,
        smpl_at: smpl.body,
        smpl_len: smpl.size / 2,
    })
}

/// Samples `start..end` of the file's sample data.
pub(super) fn read_samples(
    file: &mut File,
    tables: &Tables,
    start: u64,
    end: u64,
) -> Result<Vec<i16>, Sf2Error> {
    if !(start <= end && end <= tables.smpl_len) {
        return Err(broken("a sample lies outside the sample data"));
    }
    let mut bytes = vec![0_u8; ((end - start) * 2) as usize];
    read_at(file, tables.smpl_at + start * 2, &mut bytes)?;
    Ok(bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect())
}
