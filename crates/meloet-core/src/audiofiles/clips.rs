//! What 「ノーマライズ」 and 「逆にする」 read and write, on the thread that reads audio files
//! in (round 12, A9): the loudest sample of the frames a clip plays, and a new file of those
//! frames backwards in the work place, with its peaks. The file a clip plays is only read.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{SystemTime, UNIX_EPOCH};

use meloet_model::{NewAudio, PlayedFrames};
use sha2::{Digest, Sha256};

use super::decode::Decoder;
use super::wav::{CacheWav, WavPcm};
use super::{AudioStore, sync_dir};
use crate::pcm::PcmFile;
use crate::peaks::PeakBuilder;

/// Frames read at a time.
const STEP: usize = 1 << 15;

const NOT_THE_SONGS: &str = "the file is not what the song says it is";

/// The file of the frames, as it is on the disk: a plain wav is read where it is, anything
/// else is decoded from its head.
enum Opened {
    Wav(WavPcm),
    Decoded(Decoder),
}

fn open(store: &AudioStore, played: &PlayedFrames) -> Result<Opened, String> {
    let path = (store.find(&played.source.file)).ok_or("the file is not found")?;
    match WavPcm::open(&path) {
        Ok(wav) if wav.channels() == played.source.channels => Ok(Opened::Wav(wav)),
        Ok(_) => Err(NOT_THE_SONGS.into()),
        Err(_) => Decoder::open(&path)
            .map(Opened::Decoded)
            .map_err(String::from),
    }
}

/// The samples of the frames `range` of a file being decoded, in order, a block at a time.
fn decoded(
    decoder: &mut Decoder,
    channels: u8,
    range: Range<u64>,
    each: &mut dyn FnMut(&[f32]) -> Result<(), String>,
) -> Result<(), String> {
    let width = usize::from(channels);
    let mut at = 0u64;
    while at < range.end {
        let Some((got, block)) = decoder.next_block()? else {
            break;
        };
        if got != channels {
            return Err(NOT_THE_SONGS.into());
        }
        let frames = (block.len() / width) as u64;
        let (a, b) = (range.start.max(at), range.end.min(at + frames));
        if a < b {
            each(&block[(a - at) as usize * width..(b - at) as usize * width])?;
        }
        at += frames;
    }
    Ok(())
}

/// The largest sample of the frames the clip sounds, either way; 0 for silence.
pub fn peak(store: &AudioStore, played: &PlayedFrames) -> Result<f32, String> {
    let range = played.sounding();
    if range.is_empty() {
        return Ok(0.0);
    }
    let mut peak = 0.0f32;
    let mut take = |samples: &[f32]| {
        for s in samples {
            // A sample that is not a number is passed over.
            if s.abs() > peak {
                peak = s.abs();
            }
        }
        Ok(())
    };
    match open(store, played)? {
        Opened::Wav(mut wav) => {
            let width = usize::from(wav.channels());
            let mut block = vec![0.0f32; STEP * width];
            let (mut at, end) = (range.start, range.end.min(wav.frames()));
            while at < end {
                let want = ((end - at) as usize).min(STEP);
                let got = (wav.read(at, &mut block[..want * width])).map_err(|e| e.to_string())?;
                if got == 0 {
                    break;
                }
                take(&block[..got * width])?;
                at += got as u64;
            }
        }
        Opened::Decoded(mut decoder) => {
            decoded(&mut decoder, played.source.channels, range, &mut take)?;
        }
    }
    Ok(peak)
}

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn temp_name(what: &str) -> String {
    format!(
        ".{what}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Relaxed)
    )
}

/// Writes the frames the clip plays, backwards, into a new file in the work place (see
/// `meloet_model::Reversed`), with its peaks beside the others, and tells the store. The
/// file is named `wanted`, or `wanted` with " 2", " 3" … when a file of the song's places or
/// a name of `taken` has that name. A plain wav keeps the kind of its samples; anything else
/// is written as 32-bit float.
pub fn reverse(
    store: &AudioStore,
    played: &PlayedFrames,
    wanted: &str,
    taken: &[String],
) -> Result<NewAudio, String> {
    let work = store.work_dir().ok_or("no work place")?;
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let sounding = played.sounding();
    if sounding.is_empty() {
        return Err("the clip is silent".into());
    }
    // A wav the frames can be read from in any order, and the frame of the file its frame
    // 0 is.
    let (mut wav, first, decoded_to) = match open(store, played)? {
        Opened::Wav(wav) => (wav, 0, None),
        Opened::Decoded(mut decoder) => {
            if decoder.sample_rate() != played.source.sample_rate {
                return Err(NOT_THE_SONGS.into());
            }
            let temp = work.join(format!("{}.wav", temp_name("decoded")));
            let mut out = (CacheWav::create(&temp, decoder.sample_rate(), played.source.channels))
                .map_err(|e| e.to_string())?;
            let mut write = |samples: &[f32]| out.write(samples).map_err(|e| e.to_string());
            if let Err(e) = decoded(
                &mut decoder,
                played.source.channels,
                sounding.clone(),
                &mut write,
            ) {
                out.abandon();
                return Err(e);
            }
            out.finish().map_err(|e| e.to_string())?;
            let wav = WavPcm::open(&temp).map_err(|e| e.to_string());
            if wav.is_err() {
                let _ = fs::remove_file(&temp);
            }
            (wav?, sounding.start, Some(temp))
        }
    };
    let written = write_backwards(store, &work, &mut wav, first, played, wanted, taken);
    drop(wav);
    if let Some(temp) = decoded_to {
        let _ = fs::remove_file(temp);
    }
    written
}

fn write_backwards(
    store: &AudioStore,
    work: &Path,
    wav: &mut WavPcm,
    first: u64,
    played: &PlayedFrames,
    wanted: &str,
    taken: &[String],
) -> Result<NewAudio, String> {
    let frames = played.backwards_frames();
    let sounding = played.sounding();
    // What the file does not have is silent as well.
    let end = sounding.end.min(first + wav.frames()).max(sounding.start);
    let temp = work.join(format!("{}.part", temp_name("backwards")));
    let file =
        (File::options().write(true).create_new(true).open(&temp)).map_err(|e| e.to_string())?;
    let mut peaks = PeakBuilder::new(wav.channels());
    let written = (|| -> io::Result<()> {
        let mut out = BufWriter::with_capacity(1 << 16, file);
        out.write_all(&wav.head_for(frames))?;
        let size = wav.frame_bytes();
        let mut samples = Vec::new();
        let mut put = |out: &mut BufWriter<File>, wav: &WavPcm, bytes: &[u8]| -> io::Result<()> {
            out.write_all(bytes)?;
            wav.samples_of(bytes, &mut samples);
            peaks.push(&samples);
            Ok(())
        };
        // The clip's end is the new file's head: silence where the take had ended.
        let silence = vec![wav.silent_byte(); STEP * size];
        let mut left = played.to as u64 - end;
        while left > 0 {
            let n = left.min(STEP as u64) as usize;
            put(&mut out, wav, &silence[..n * size])?;
            left -= n as u64;
        }
        let (mut raw, mut back) = (vec![0u8; STEP * size], vec![0u8; STEP * size]);
        let mut at = end;
        while at > sounding.start {
            let n = ((at - sounding.start) as usize).min(STEP);
            let from = at - n as u64;
            let got = wav.read_raw(from - first, &mut raw[..n * size])?;
            if got != n {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the file got shorter",
                ));
            }
            let frames = raw[..n * size].chunks_exact(size).rev();
            for (to, frame) in back.chunks_exact_mut(size).zip(frames) {
                to.copy_from_slice(frame);
            }
            put(&mut out, wav, &back[..n * size])?;
            at = from;
        }
        out.into_inner().map_err(|e| e.into_error())?.sync_all()
    })();
    let named = written.and_then(|()| put_in_place(store, work, &temp, wanted, taken));
    let file = match named {
        Ok(name) => name,
        Err(e) => {
            let _ = fs::remove_file(&temp);
            return Err(e.to_string());
        }
    };
    let uid = fresh_uid(played);
    if let Some(path) = store.peaks_path(&uid)
        && let Err(e) = peaks.write(&path)
    {
        eprintln!("meloet: the peaks of {file} were not kept: {e}");
    }
    store.file_arrived();
    Ok(NewAudio {
        uid,
        file,
        frames,
        sample_rate: played.source.sample_rate,
        channels: played.source.channels,
        bits: Some(wav.bits()),
    })
}

/// Gives the whole file at `temp` the first name of `wanted`, `wanted` with " 2", " 3" …,
/// that no file of the song's places has and `taken` does not name. A file that gets the
/// same name at the same moment (a recording) keeps it.
fn put_in_place(
    store: &AudioStore,
    work: &Path,
    temp: &Path,
    wanted: &str,
    taken: &[String],
) -> io::Result<String> {
    let (stem, ending) = match wanted.rfind('.').filter(|&dot| dot > 0) {
        Some(dot) => (&wanted[..dot], &wanted[dot..]),
        None => (wanted, ""),
    };
    for n in 1u32.. {
        let name = match n {
            1 => wanted.to_owned(),
            _ => format!("{stem} {n}{ending}"),
        };
        if taken.contains(&name) || store.find(&name).is_some() {
            continue;
        }
        let to = work.join(&name);
        match fs::hard_link(temp, &to) {
            Ok(()) => {
                let _ = fs::remove_file(temp);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            // A file system without links.
            Err(_) => fs::rename(temp, &to)?,
        }
        sync_dir(work)?;
        return Ok(name);
    }
    unreachable!("u32 names run out")
}

/// A number no other file gets: what is kept beside the file (peaks, the cache) goes by it.
fn fresh_uid(played: &PlayedFrames) -> [u8; 16] {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hash = Sha256::new();
    hash.update(b"meloet backwards audio");
    hash.update(played.source.uid);
    hash.update(played.from.to_le_bytes());
    hash.update(played.to.to_le_bytes());
    hash.update(std::process::id().to_le_bytes());
    hash.update(now.as_nanos().to_le_bytes());
    hash.update(SERIAL.fetch_add(1, Relaxed).to_le_bytes());
    let mut uid = [0u8; 16];
    uid.copy_from_slice(&hash.finalize()[..16]);
    uid
}

#[cfg(test)]
mod tests;
