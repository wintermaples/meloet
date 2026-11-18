//! Reads a pack and its instruments from disk. Never called on the audio thread.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

use serde::Deserialize;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::TrackType;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;

use super::bank::{INSTRUMENT_FORMAT, INSTRUMENT_VERSION, InstrumentFile, SampleBank, SampleData};

const PACK_FORMAT: &str = "meloet-sounds";
const PACK_VERSION: u32 = 1;

/// Which file could not be used, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub path: PathBuf,
    pub what: String,
}

impl LoadError {
    fn new(path: &Path, what: impl fmt::Display) -> LoadError {
        LoadError {
            path: path.to_path_buf(),
            what: what.to_string(),
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.what)
    }
}

impl std::error::Error for LoadError {}

/// What `pack.json` lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    /// The pack's own version, for example `2026.11.0`.
    pub pack: String,
    /// Instrument name and the full path of its `instrument.json`, in name order.
    pub instruments: Vec<(String, PathBuf)>,
}

#[derive(Deserialize)]
struct PackFile {
    format: String,
    version: u32,
    pack: String,
    instruments: BTreeMap<String, String>,
}

/// Reads `pack.json` in a pack's folder. The instruments themselves are not read.
pub fn read_pack(dir: &Path) -> Result<Pack, LoadError> {
    let path = dir.join("pack.json");
    let file: PackFile = read_json(&path)?;
    check_format(&path, &file.format, file.version, PACK_FORMAT, PACK_VERSION)?;
    let mut instruments = Vec::with_capacity(file.instruments.len());
    for (name, relative) in file.instruments {
        let inside = inside(Path::new(&relative))
            .map_err(|what| LoadError::new(&path, format!("instruments.{name}: {what}")))?;
        instruments.push((name, dir.join(inside)));
    }
    Ok(Pack {
        pack: file.pack,
        instruments,
    })
}

/// Reads an `instrument.json` and every sample it names into memory, decoding on `threads`
/// threads that live only for this call. Blocks until done.
pub fn load_instrument(json: &Path, threads: usize) -> Result<Arc<SampleBank>, LoadError> {
    let file: InstrumentFile = read_json(json)?;
    check_format(
        json,
        &file.format,
        file.version,
        INSTRUMENT_FORMAT,
        INSTRUMENT_VERSION,
    )?;
    let dir = json.parent().unwrap_or(Path::new(""));

    // A file used by several regions is read once.
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut places: BTreeMap<&str, u32> = BTreeMap::new();
    let mut sample_of = Vec::with_capacity(file.regions.len());
    for (index, region) in file.regions.iter().enumerate() {
        let place = match places.get(region.sample.as_str()) {
            Some(place) => *place,
            None => {
                let relative = inside(Path::new(&region.sample)).map_err(|what| {
                    LoadError::new(json, format!("regions[{index}].sample: {what}"))
                })?;
                paths.push(dir.join(relative));
                places.insert(&region.sample, paths.len() as u32 - 1);
                paths.len() as u32 - 1
            }
        };
        sample_of.push(place);
    }
    drop(places);

    let samples = decode_all(&paths, threads)?;
    let bank =
        SampleBank::build(file, samples, &sample_of).map_err(|what| LoadError::new(json, what))?;
    Ok(Arc::new(bank))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|e| LoadError::new(path, e))?;
    serde_json::from_str(&text).map_err(|e| LoadError::new(path, e))
}

fn check_format(
    path: &Path,
    format: &str,
    version: u32,
    want_format: &str,
    want_version: u32,
) -> Result<(), LoadError> {
    if format != want_format {
        return Err(LoadError::new(
            path,
            format!("format is \"{format}\", must be \"{want_format}\""),
        ));
    }
    if version != want_version {
        return Err(LoadError::new(
            path,
            format!("version is {version}, this meloet reads version {want_version}"),
        ));
    }
    Ok(())
}

/// A path in a pack may only point further into the pack.
fn inside(relative: &Path) -> Result<&Path, String> {
    let stays = relative
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if stays && relative.file_name().is_some() {
        Ok(relative)
    } else {
        Err(format!(
            "\"{}\" is not a path inside the pack",
            relative.display()
        ))
    }
}

fn decode_all(paths: &[PathBuf], threads: usize) -> Result<Vec<SampleData>, LoadError> {
    let threads = threads.clamp(1, paths.len().max(1));
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let work = || {
        let mut done = Vec::new();
        while !failed.load(Relaxed) {
            let index = next.fetch_add(1, Relaxed);
            let Some(path) = paths.get(index) else { break };
            let decoded = decode(path);
            if decoded.is_err() {
                failed.store(true, Relaxed);
            }
            done.push((index, decoded));
        }
        done
    };
    let mut all = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads).map(|_| scope.spawn(work)).collect();
        let mut all = Vec::with_capacity(paths.len());
        for worker in workers {
            match worker.join() {
                Ok(done) => all.extend(done),
                Err(_) => {
                    let lost = PathBuf::from("(a decoding thread)");
                    all.push((usize::MAX, Err(LoadError::new(&lost, "stopped early"))));
                }
            }
        }
        all
    });
    all.sort_by_key(|(index, _)| *index);
    all.into_iter().map(|(_, decoded)| decoded).collect()
}

fn decode(path: &Path) -> Result<SampleData, LoadError> {
    let fail = |what: &str, e: &dyn fmt::Display| LoadError::new(path, format!("{what}: {e}"));
    let file = File::open(path).map_err(|e| LoadError::new(path, e))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, stream, Default::default(), Default::default())
        .map_err(|e| fail("not a sound file this meloet reads", &e))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| LoadError::new(path, "no sound in the file"))?;
    let track_id = track.id;
    let expected_frames = track.num_frames;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| LoadError::new(path, "no sound in the file"))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| fail("no decoder", &e))?;

    let (mut rate, mut channels) = (0_u32, 0_usize);
    let mut data: Vec<i16> = Vec::new();
    while let Some(packet) = format.next_packet().map_err(|e| fail("read", &e))? {
        if packet.track_id != track_id {
            continue;
        }
        let buffer = decoder.decode(&packet).map_err(|e| fail("decode", &e))?;
        if channels == 0 {
            rate = buffer.spec().rate();
            channels = buffer.spec().channels().count();
            if !(1..=2).contains(&channels) {
                let what = format!("{channels} channels; a sample is mono or stereo");
                return Err(LoadError::new(path, what));
            }
            if let Some(frames) = expected_frames {
                data.reserve_exact(frames as usize * channels);
            }
        }
        let at = data.len();
        data.resize(at + buffer.samples_interleaved(), 0);
        buffer.copy_to_slice_interleaved(&mut data[at..]);
    }
    if data.is_empty() || rate == 0 {
        return Err(LoadError::new(path, "the file has no sound in it"));
    }
    data.shrink_to_fit();
    Ok(SampleData {
        rate: rate as f32,
        channels,
        frames: data.len() / channels,
        data,
    })
}
