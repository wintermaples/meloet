//! Reads a SoundFont 2 file the user brings and turns one of its presets into a
//! [`SampleBank`] (docs/architecture.md 20-7). Never called on the audio thread.
//!
//! What is read: the zones with their generators, the 16-bit samples. What is not: the
//! modulators of the file (PMOD, IMOD), the keynum and velocity generators, the chorus and
//! reverb sends, the 24-bit extension (sm24) and SoundFont 3 (Ogg Vorbis inside), which gets
//! an error of its own.

mod file;
mod zones;

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::sampler::{EnvShape, LfoShape, SampleBank, ZoneSpec};
use file::{ROM_SAMPLE, Tables};
use zones::generator::*;
use zones::{GENERATORS, NAMES, Zone};

/// FluidSynth takes 0.4 of initialAttenuation, and SoundFonts are tuned by ear with it
/// (FluidR3 on Linux above all). 1.0 is what the specification says.
const ATTENUATION_SCALE: f32 = 0.4;
/// Added to every zone, so that a preset is about as loud as the instruments of the packs.
/// The same as FluidSynth's gain setting at 0.63 (its default is 0.2).
pub const GAIN_DB: f32 = -4.0;
/// A preset often plays two to four zones for one key (left and right, layers).
const POLYPHONY: usize = 128;
/// A zone with fewer frames than this is left out.
const MIN_FRAMES: i64 = 4;
const MIN_LOOP_FRAMES: i64 = 4;
/// Frequency of absolute cent 0 (8.1.3).
const CENT_0_HZ: f32 = 8.176;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sf2Error {
    /// The file could not be opened or read; the text is the system's.
    Io(String),
    /// Not a SoundFont 2 file, or one too broken to read; the text says what is wrong.
    NotSf2(String),
    /// A SoundFont 3 file (samples in Ogg Vorbis), which this meloet does not read.
    Sf3,
    /// The file has no preset with this bank and program.
    NoPreset { bank: u16, program: u16 },
    /// The preset is there, but none of its zones can be played.
    NoSound { bank: u16, program: u16 },
}

impl fmt::Display for Sf2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sf2Error::Io(what) => write!(f, "could not read the file: {what}"),
            Sf2Error::NotSf2(what) => write!(f, "not a SoundFont 2 file: {what}"),
            Sf2Error::Sf3 => write!(f, "a SoundFont 3 file (compressed samples) is not read"),
            Sf2Error::NoPreset { bank, program } => {
                write!(f, "no preset {bank}:{program} in the file")
            }
            Sf2Error::NoSound { bank, program } => {
                write!(f, "preset {bank}:{program} has no zone that can be played")
            }
        }
    }
}

impl std::error::Error for Sf2Error {}

/// One preset of the file. Drum kits are in bank 128 by custom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetInfo {
    pub bank: u16,
    pub program: u16,
    pub name: String,
}

/// One zone a preset plays, as the file says it, for tools and tests.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneInfo {
    pub sample: String,
    /// 7.10 sfSampleType: 1 mono, 2 right, 4 left.
    pub sample_type: u16,
    pub sample_link: u16,
    pub instrument: String,
    pub keys: (u8, u8),
    pub velocities: (u8, u8),
    /// Every named generator with its amount, the defaults too.
    pub generators: Vec<(&'static str, i32)>,
}

/// An opened SoundFont: its tables are in memory, its samples are still on disk.
pub struct Sf2File {
    path: PathBuf,
    tables: Tables,
}

impl Sf2File {
    /// Reads the tables of the file. The samples are read by [`Sf2File::load`].
    pub fn open(path: &Path) -> Result<Sf2File, Sf2Error> {
        let mut file = File::open(path).map_err(|e| Sf2Error::Io(e.to_string()))?;
        Ok(Sf2File {
            path: path.to_path_buf(),
            tables: file::read(&mut file)?,
        })
    }

    /// The presets by bank and program. Of two with the same bank and program the first in
    /// the file counts.
    pub fn presets(&self) -> Vec<PresetInfo> {
        let named = &self.tables.presets[..self.tables.presets.len() - 1];
        let mut first: BTreeMap<(u16, u16), &str> = BTreeMap::new();
        for p in named {
            first.entry((p.bank, p.program)).or_insert(&p.name);
        }
        first
            .into_iter()
            .map(|((bank, program), name)| PresetInfo {
                bank,
                program,
                name: name.to_string(),
            })
            .collect()
    }

    fn find(&self, bank: u16, program: u16) -> Result<usize, Sf2Error> {
        let named = &self.tables.presets[..self.tables.presets.len() - 1];
        named
            .iter()
            .position(|p| p.bank == bank && p.program == program)
            .ok_or(Sf2Error::NoPreset { bank, program })
    }

    #[doc(hidden)]
    pub fn zones(&self, bank: u16, program: u16) -> Result<Vec<ZoneInfo>, Sf2Error> {
        let index = self.find(bank, program)?;
        let zones = zones::zones(&self.tables, index);
        Ok(zones
            .iter()
            .map(|z| {
                let sample = &self.tables.samples[z.sample];
                let named = NAMES.iter().zip(&z.amount);
                ZoneInfo {
                    sample: sample.name.clone(),
                    sample_type: sample.kind,
                    sample_link: sample.link,
                    instrument: self.tables.insts[z.inst].name.clone(),
                    keys: (z.key_lo, z.key_hi),
                    velocities: (z.vel_lo, z.vel_hi),
                    generators: named
                        .filter(|(name, _)| !matches!(**name, "" | "instrument" | "sampleID"))
                        .filter(|(name, _)| !name.ends_with("Range"))
                        .map(|(name, amount)| (*name, *amount))
                        .collect(),
                }
            })
            .collect())
    }

    /// The keys the preset has a playable zone on, lowest first.
    pub fn keys(&self, bank: u16, program: u16) -> Result<Vec<u8>, Sf2Error> {
        let index = self.find(bank, program)?;
        let mut has = [false; 128];
        for zone in zones::zones(&self.tables, index) {
            if self.spec(&zone).is_none() {
                continue;
            }
            for key in zone.key_lo..=zone.key_hi.min(127) {
                has[usize::from(key)] = true;
            }
        }
        Ok((0..=127u8).filter(|k| has[usize::from(*k)]).collect())
    }

    /// Each key the preset has a playable zone on, lowest first, with a name made of the
    /// samples that zones of at most [`NAMING_KEYS`] keys play there: a stereo half's mark
    /// ("(L)") taken off, the same name once, at most [`KEY_NAMES`] joined with " / ". `None`
    /// where only wider zones play. An SF2 file has no names for keys; in a drum kit the names
    /// of its samples are the nearest thing.
    pub fn key_names(
        &self,
        bank: u16,
        program: u16,
    ) -> Result<Vec<(u8, Option<String>)>, Sf2Error> {
        let index = self.find(bank, program)?;
        let mut names: Vec<Vec<String>> = vec![Vec::new(); 128];
        let mut has = [false; 128];
        for zone in zones::zones(&self.tables, index) {
            if self.spec(&zone).is_none() {
                continue;
            }
            let (lo, hi) = (zone.key_lo, zone.key_hi.min(127));
            let sample = &self.tables.samples[zone.sample];
            let name = (hi - lo < NAMING_KEYS)
                .then(|| without_half_mark(&sample.name, sample.kind))
                .filter(|name| !name.is_empty());
            for key in lo..=hi {
                has[usize::from(key)] = true;
                let list = &mut names[usize::from(key)];
                if let Some(name) = &name
                    && !list.contains(name)
                    && list.len() < KEY_NAMES
                {
                    list.push(name.clone());
                }
            }
        }
        Ok((0..=127u8)
            .filter(|k| has[usize::from(*k)])
            .map(|k| {
                let list = &names[usize::from(k)];
                (k, (!list.is_empty()).then(|| list.join(" / ")))
            })
            .collect())
    }

    /// Reads the samples one preset uses and builds its bank. A zone whose sample or values
    /// make no sense is left out. The left and right halves of a stereo sample are two zones
    /// with their own pan and play as two voices.
    ///
    /// `_sample_rate` is not needed today: the player turns to the rate it is prepared with.
    pub fn load(&self, bank: u16, program: u16, _sample_rate: f32) -> Result<SampleBank, Sf2Error> {
        let index = self.find(bank, program)?;
        let zones = zones::zones(&self.tables, index);

        // A file used by several zones is read once; the key is its place in the sample data.
        let mut places: BTreeMap<(u64, u64), u32> = BTreeMap::new();
        let mut spans: Vec<(u64, u64, f32)> = Vec::new();
        let mut specs = Vec::with_capacity(zones.len());
        let mut budget = self.tables.smpl_len.saturating_mul(2);
        for zone in &zones {
            let Some((start, end, mut spec)) = self.spec(zone) else {
                continue;
            };
            let rate = self.tables.samples[zone.sample].rate as f32;
            spec.sample = match places.get(&(start, end)) {
                Some(place) => *place,
                None => {
                    // Broken tables could ask for the same data under ever new cuts.
                    if end - start > budget {
                        continue;
                    }
                    budget -= end - start;
                    spans.push((start, end, rate));
                    places.insert((start, end), spans.len() as u32 - 1);
                    spans.len() as u32 - 1
                }
            };
            specs.push(spec);
        }
        if specs.is_empty() {
            return Err(Sf2Error::NoSound { bank, program });
        }

        let mut file = File::open(&self.path).map_err(|e| Sf2Error::Io(e.to_string()))?;
        let mut samples = Vec::with_capacity(spans.len());
        for (start, end, rate) in spans {
            let data = file::read_samples(&mut file, &self.tables, start, end)?;
            samples.push((rate, data));
        }
        let name = self.tables.presets[index].name.clone();
        Ok(SampleBank::from_zones(
            name, POLYPHONY, GAIN_DB, samples, specs,
        ))
    }

    /// The zone in the player's units, and where its sample starts and ends in the sample
    /// data. None when the zone cannot be played.
    fn spec(&self, zone: &Zone) -> Option<(u64, u64, ZoneSpec)> {
        let g = &zone.amount;
        let sample = &self.tables.samples[zone.sample];
        if sample.kind & ROM_SAMPLE != 0 || !(1..=1_000_000).contains(&sample.rate) {
            return None;
        }
        if zone.key_lo > 127 || zone.vel_hi == 0 {
            return None;
        }

        // 8.1.2: the address offsets move the sample's points; the coarse ones by 32768.
        let point = |header: u32, fine: usize, coarse: usize| {
            i64::from(header) + i64::from(g[fine]) + 32_768 * i64::from(g[coarse])
        };
        let start = point(sample.start, START_OFFSET, START_COARSE_OFFSET);
        let end = point(sample.end, END_OFFSET, END_COARSE_OFFSET);
        if start < 0 || end - start < MIN_FRAMES || end as u64 > self.tables.smpl_len {
            return None;
        }
        let loop_start = point(
            sample.loop_start,
            LOOP_START_OFFSET,
            LOOP_START_COARSE_OFFSET,
        );
        let loop_end = point(sample.loop_end, LOOP_END_OFFSET, LOOP_END_COARSE_OFFSET);
        // 8.1.2 sampleModes: 1 loops for ever, 3 loops until the key goes up; 0 and 2 do not.
        let mode = g[SAMPLE_MODES] & 3;
        let loop_fits =
            start <= loop_start && loop_end - loop_start >= MIN_LOOP_FRAMES && loop_end <= end;
        let looped = ((mode == 1 || mode == 3) && loop_fits)
            .then(|| ((loop_start - start) as usize, (loop_end - start) as usize));

        // 7.10: an original pitch of 255 (or anything over 127) is taken as 60.
        let root = match g[ROOT_KEY] {
            key @ 0..=127 => key as u8,
            _ if sample.original_pitch <= 127 => sample.original_pitch,
            _ => 60,
        };
        let tune_cents = g[COARSE_TUNE].clamp(-120, 120) * 100
            + g[FINE_TUNE].clamp(-99, 99)
            + i32::from(sample.correction);
        let attenuation_cb = g[ATTENUATION].clamp(0, 1440) as f32;

        Some((
            start as u64,
            end as u64,
            ZoneSpec {
                sample: 0,
                key_lo: zone.key_lo,
                key_hi: zone.key_hi.min(127),
                vel_lo: zone.vel_lo.max(1),
                vel_hi: zone.vel_hi.min(127),
                root,
                tune_cents: tune_cents as f32,
                scale_cents: g[SCALE_TUNING].clamp(0, 1200) as f32,
                gain_db: -attenuation_cb * ATTENUATION_SCALE / 10.0,
                looped,
                exclusive_class: g[EXCLUSIVE_CLASS].clamp(0, 65_535) as u16,
                // sustainVolEnv is centibels down; decay and release cover 96 dB.
                volume: envelope(g, DELAY_VOL_ENV, |cb| cb.clamp(0, 1440) as f32 / 960.0),
                // sustainModEnv is tenths of a percent down.
                modulation: envelope(g, DELAY_MOD_ENV, |x| x.clamp(0, 1000) as f32 / 1000.0),
                mod_to_pitch: cents(g[MOD_ENV_TO_PITCH]),
                mod_to_filter: cents(g[MOD_ENV_TO_FILTER]),
                vibrato: lfo(g[DELAY_VIB_LFO], g[FREQ_VIB_LFO]),
                vibrato_to_pitch: cents(g[VIB_LFO_TO_PITCH]),
                lfo: lfo(g[DELAY_MOD_LFO], g[FREQ_MOD_LFO]),
                lfo_to_pitch: cents(g[MOD_LFO_TO_PITCH]),
                lfo_to_filter: cents(g[MOD_LFO_TO_FILTER]),
                lfo_to_volume_db: g[MOD_LFO_TO_VOLUME].clamp(-960, 960) as f32 / 10.0,
                filter_cents: g[FILTER_FC].clamp(1500, 13_500) as f32,
                // 8.1.2 initialFilterQ: centibels above the flat response, whose Q is
                // 1/sqrt(2), that is 3.01 dB under 1.
                filter_q: 10.0_f32.powf((g[FILTER_Q].clamp(0, 960) as f32 / 10.0 - 3.01) / 20.0),
                pan: g[PAN].clamp(-500, 500) as f32 / 500.0,
                loop_until_release: mode == 3,
            },
        ))
    }
}

/// Zones of at most this many keys name them ([`Sf2File::key_names`]): a drum kit plays a
/// sample on one key, or on two or three that are the same drum.
pub const NAMING_KEYS: u8 = 3;
/// Different sample names kept for one key.
pub const KEY_NAMES: usize = 3;

/// 7.10 sfSampleType of the two halves of a stereo sample.
const RIGHT_SAMPLE: u16 = 2;
const LEFT_SAMPLE: u16 = 4;

/// A sample's name without the mark of the stereo half it is: "Std Kick(L)" is "Std Kick".
fn without_half_mark(name: &str, kind: u16) -> String {
    let name = name.trim();
    if !matches!(kind & !ROM_SAMPLE, LEFT_SAMPLE | RIGHT_SAMPLE) {
        return name.to_owned();
    }
    let marks = [
        "(L)", "(R)", "[L]", "[R]", "_L", "_R", "-L", "-R", ".L", ".R", " L", " R",
    ];
    (marks.iter())
        .find_map(|mark| name.strip_suffix(mark))
        .map(str::trim_end)
        .filter(|rest| !rest.is_empty())
        .unwrap_or(name)
        .to_owned()
}

fn cents(amount: i32) -> f32 {
    amount.clamp(-12_000, 12_000) as f32
}

/// 8.1.3: times are in timecents, seconds = 2^(timecents / 1200).
fn seconds(timecents: i32, longest: i32) -> f32 {
    (timecents.clamp(-12_000, longest) as f32 / 1200.0).exp2()
}

/// As [`seconds`], for a delay or a hold: the shortest time is none at all.
fn wait(timecents: i32) -> f32 {
    if timecents <= -12_000 {
        0.0
    } else {
        seconds(timecents, 5000)
    }
}

/// An envelope whose six generators start at `delay`: delay, attack, hold, decay, sustain,
/// release.
fn envelope(g: &[i32; GENERATORS], delay: usize, sustain: impl Fn(i32) -> f32) -> EnvShape {
    let keys = if delay == DELAY_VOL_ENV {
        (KEY_TO_VOL_ENV_HOLD, KEY_TO_VOL_ENV_DECAY)
    } else {
        (KEY_TO_MOD_ENV_HOLD, KEY_TO_MOD_ENV_DECAY)
    };
    EnvShape {
        delay_s: wait(g[delay]),
        attack_s: seconds(g[delay + 1], 8000),
        hold_s: wait(g[delay + 2]),
        decay_s: seconds(g[delay + 3], 8000),
        sustain: sustain(g[delay + 4]).min(1.0),
        release_s: seconds(g[delay + 5], 8000),
        key_hold: g[keys.0].clamp(-1200, 1200) as f32,
        key_decay: g[keys.1].clamp(-1200, 1200) as f32,
    }
}

fn lfo(delay: i32, frequency: i32) -> LfoShape {
    LfoShape {
        delay_s: wait(delay),
        hz: CENT_0_HZ * (frequency.clamp(-16_000, 4500) as f32 / 1200.0).exp2(),
    }
}
