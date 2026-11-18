//! One instrument in memory: its samples as i16 and its regions, checked and ready to play.

use std::ops::Range;

use serde::Deserialize;

pub(super) const INSTRUMENT_FORMAT: &str = "meloet-instrument";
pub(super) const INSTRUMENT_VERSION: u32 = 1;
const DEFAULT_POLYPHONY: u32 = 64;
const MAX_POLYPHONY: u32 = 256;
/// Used when a region does not say how long its released note rings.
const DEFAULT_RELEASE_MS: f32 = 200.0;
const MAX_GAIN_DB: f32 = 48.0;
/// Choke groups are kept as the bits of a u64.
const MAX_CHOKE_GROUPS: u32 = 64;
/// Four-point interpolation reads one frame before and two after the playing position.
const MIN_LOOP_FRAMES: u64 = 8;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstrumentFile {
    pub format: String,
    pub version: u32,
    pub kind: String,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default = "default_polyphony")]
    pub polyphony: u32,
    #[serde(default)]
    pub sustain_pedal: bool,
    pub regions: Vec<RegionFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RegionFile {
    pub sample: String,
    pub key_lo: u8,
    pub key_hi: u8,
    pub root: u8,
    #[serde(default)]
    pub tune_cents: f32,
    #[serde(default = "one")]
    pub vel_lo: u8,
    #[serde(default = "top")]
    pub vel_hi: u8,
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default = "hundred")]
    pub amp_veltrack: f32,
    #[serde(default = "hundred")]
    pub pitch_keytrack: f32,
    #[serde(default)]
    pub attack_ms: f32,
    #[serde(default)]
    pub release_ms: Option<f32>,
    #[serde(default)]
    pub rt_decay_db_per_s: f32,
    #[serde(default)]
    pub loop_start: Option<u64>,
    #[serde(default)]
    pub loop_end: Option<u64>,
    #[serde(default)]
    pub loop_decay_db_per_s: f32,
    #[serde(default)]
    pub one_shot: bool,
    #[serde(default)]
    pub rr_index: Option<u32>,
    #[serde(default)]
    pub rr_count: Option<u32>,
    #[serde(default)]
    pub choke_group: Option<u32>,
    #[serde(default)]
    pub choked_by: Vec<u32>,
    #[serde(default)]
    pub pan: f32,
}

fn default_polyphony() -> u32 {
    DEFAULT_POLYPHONY
}
fn one() -> u8 {
    1
}
fn top() -> u8 {
    127
}
fn hundred() -> f32 {
    100.0
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) enum Trigger {
    #[default]
    Attack,
    Release,
    PedalDown,
    PedalUp,
}

/// One decoded file. Stereo is interleaved.
pub(super) struct SampleData {
    pub rate: f32,
    pub channels: usize,
    pub frames: usize,
    pub data: Vec<i16>,
}

pub(super) struct RoundRobin {
    /// Counted from 0 here; `rrIndex` in the file counts from 1.
    pub index: u32,
    pub count: u32,
    /// Regions that take turns with each other share a counter.
    pub group: u32,
}

pub(super) struct Region {
    pub sample: u32,
    pub vel_lo: u8,
    pub vel_hi: u8,
    pub root: u8,
    pub tune_semitones: f32,
    /// Semitones of pitch per key.
    pub keytrack: f32,
    /// Region and instrument gain, and the step from i16 to -1..1.
    pub gain: f32,
    pub pan_left: f32,
    pub pan_right: f32,
    /// -1..1
    pub veltrack: f32,
    pub attack_s: f32,
    pub release_s: f32,
    pub rt_decay_db_per_s: f32,
    /// Frames `start..end`.
    pub looped: Option<(usize, usize)>,
    pub loop_decay_db_per_s: f32,
    pub one_shot: bool,
    pub round_robin: Option<RoundRobin>,
    /// The bit of the region's own choke group, or 0.
    pub choke_group: u64,
    /// The bits of the groups that stop this region's voices.
    pub choked_by: u64,
}

/// (trigger, keys, velocities, count) of a set of regions that take turns.
type RoundRobinId = (Trigger, u8, u8, u8, u8, u32);

/// A sampled instrument, read whole into memory. Shared by every track that plays it.
pub struct SampleBank {
    kind: String,
    polyphony: usize,
    sustain_pedal: bool,
    pub(super) samples: Vec<SampleData>,
    pub(super) regions: Vec<Region>,
    /// Region numbers, grouped by trigger and key; the tables below point into it.
    order: Vec<u32>,
    attack: Vec<Range<u32>>,
    release: Vec<Range<u32>>,
    pedal_down: Range<u32>,
    pedal_up: Range<u32>,
    pub(super) round_robin_groups: usize,
}

impl SampleBank {
    /// The instrument's name in the pack (`grandPiano`, `studioKit`, ...).
    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn polyphony(&self) -> usize {
        self.polyphony
    }

    pub fn sustain_pedal(&self) -> bool {
        self.sustain_pedal
    }

    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Bytes the samples take in memory.
    pub fn sample_bytes(&self) -> usize {
        self.samples
            .iter()
            .map(|s| s.data.len() * size_of::<i16>())
            .sum()
    }

    /// Seconds of sound over all files.
    pub fn sample_seconds(&self) -> f64 {
        self.samples
            .iter()
            .map(|s| s.frames as f64 / f64::from(s.rate))
            .sum()
    }

    /// The regions a trigger on a key may start. Pedal triggers do not look at the key.
    pub(super) fn candidates(&self, trigger: Trigger, key: u8) -> &[u32] {
        let range = match trigger {
            Trigger::Attack => &self.attack[usize::from(key & 127)],
            Trigger::Release => &self.release[usize::from(key & 127)],
            Trigger::PedalDown => &self.pedal_down,
            Trigger::PedalUp => &self.pedal_up,
        };
        &self.order[range.start as usize..range.end as usize]
    }

    /// Checks every value and builds the bank. `sample_of[i]` is the place in `samples` of
    /// region i's file. The error says which region (counted from 0) and what is wrong.
    pub(super) fn build(
        file: InstrumentFile,
        samples: Vec<SampleData>,
        sample_of: &[u32],
    ) -> Result<SampleBank, String> {
        check((1..=MAX_POLYPHONY).contains(&file.polyphony), || {
            format!(
                "polyphony is {}, must be 1 to {MAX_POLYPHONY}",
                file.polyphony
            )
        })?;
        check_gain("gainDb", file.gain_db)?;
        let instrument_gain = db_to_gain(file.gain_db);

        let mut regions = Vec::with_capacity(file.regions.len());
        let mut groups: Vec<RoundRobinId> = Vec::new();
        for (index, r) in file.regions.iter().enumerate() {
            let sample = sample_of[index];
            let region = region(r, &samples[sample as usize], sample, &mut groups)
                .map_err(|what| format!("regions[{index}] ({}): {what}", r.sample))?;
            regions.push(Region {
                gain: region.gain * instrument_gain,
                release_s: r.release_ms.unwrap_or(DEFAULT_RELEASE_MS) / 1000.0,
                ..region
            });
        }

        let mut order = Vec::new();
        let mut table = |trigger: Trigger, key: Option<u8>| {
            let start = order.len() as u32;
            for (index, r) in file.regions.iter().enumerate() {
                if r.trigger == trigger && key.is_none_or(|k| (r.key_lo..=r.key_hi).contains(&k)) {
                    order.push(index as u32);
                }
            }
            start..order.len() as u32
        };
        let attack = (0..128).map(|k| table(Trigger::Attack, Some(k))).collect();
        let release = (0..128).map(|k| table(Trigger::Release, Some(k))).collect();
        let pedal_down = table(Trigger::PedalDown, None);
        let pedal_up = table(Trigger::PedalUp, None);

        Ok(SampleBank {
            kind: file.kind,
            polyphony: file.polyphony as usize,
            sustain_pedal: file.sustain_pedal,
            samples,
            regions,
            order,
            attack,
            release,
            pedal_down,
            pedal_up,
            round_robin_groups: groups.len(),
        })
    }
}

fn region(
    r: &RegionFile,
    sample: &SampleData,
    sample_index: u32,
    groups: &mut Vec<RoundRobinId>,
) -> Result<Region, String> {
    check(r.key_lo <= r.key_hi && r.key_hi <= 127, || {
        format!(
            "keys {} to {} are not a range inside 0 to 127",
            r.key_lo, r.key_hi
        )
    })?;
    check(r.root <= 127, || {
        format!("root is {}, must be 0 to 127", r.root)
    })?;
    check(
        1 <= r.vel_lo && r.vel_lo <= r.vel_hi && r.vel_hi <= 127,
        || {
            format!(
                "velocities {} to {} are not a range inside 1 to 127",
                r.vel_lo, r.vel_hi
            )
        },
    )?;
    check((-1200.0..=1200.0).contains(&r.tune_cents), || {
        format!("tuneCents is {}, must be -1200 to 1200", r.tune_cents)
    })?;
    check_gain("gainDb", r.gain_db)?;
    check((-100.0..=100.0).contains(&r.amp_veltrack), || {
        format!("ampVeltrack is {}, must be -100 to 100", r.amp_veltrack)
    })?;
    check((-1200.0..=1200.0).contains(&r.pitch_keytrack), || {
        format!(
            "pitchKeytrack is {}, must be -1200 to 1200",
            r.pitch_keytrack
        )
    })?;
    check_ms("attackMs", r.attack_ms)?;
    if let Some(ms) = r.release_ms {
        check_ms("releaseMs", ms)?;
    }
    check_rate("rtDecayDbPerS", r.rt_decay_db_per_s)?;
    check_rate("loopDecayDbPerS", r.loop_decay_db_per_s)?;
    check((-1.0..=1.0).contains(&r.pan), || {
        format!("pan is {}, must be -1 to 1", r.pan)
    })?;

    let looped = match (r.loop_start, r.loop_end) {
        (None, None) => None,
        (Some(start), Some(end)) => {
            check(
                start < end && end <= sample.frames as u64 && end - start >= MIN_LOOP_FRAMES,
                || {
                    format!(
                        "loop {start} to {end} does not fit: the file has {} frames, loopEnd is \
                         the frame after the last one played, and a loop is at least \
                         {MIN_LOOP_FRAMES} frames",
                        sample.frames
                    )
                },
            )?;
            Some((start as usize, end as usize))
        }
        _ => return Err("loopStart and loopEnd come together".to_string()),
    };

    let round_robin = match (r.rr_index, r.rr_count) {
        (None, None) => None,
        (Some(index), Some(count)) => {
            check((1..=count).contains(&index), || {
                format!("rrIndex is {index} of rrCount {count}; rrIndex counts from 1")
            })?;
            let id = (r.trigger, r.key_lo, r.key_hi, r.vel_lo, r.vel_hi, count);
            let group = groups.iter().position(|g| *g == id).unwrap_or_else(|| {
                groups.push(id);
                groups.len() - 1
            });
            Some(RoundRobin {
                index: index - 1,
                count,
                group: group as u32,
            })
        }
        _ => return Err("rrIndex and rrCount come together".to_string()),
    };

    // Equal power, and no change at the centre: a file panned to one side is 3 dB louder there.
    let angle = (r.pan + 1.0) * std::f32::consts::FRAC_PI_4;
    Ok(Region {
        sample: sample_index,
        vel_lo: r.vel_lo,
        vel_hi: r.vel_hi,
        root: r.root,
        tune_semitones: r.tune_cents / 100.0,
        keytrack: r.pitch_keytrack / 100.0,
        gain: db_to_gain(r.gain_db) / 32768.0,
        pan_left: std::f32::consts::SQRT_2 * angle.cos(),
        pan_right: std::f32::consts::SQRT_2 * angle.sin(),
        veltrack: r.amp_veltrack / 100.0,
        attack_s: r.attack_ms / 1000.0,
        release_s: 0.0,
        rt_decay_db_per_s: r.rt_decay_db_per_s,
        looped,
        loop_decay_db_per_s: r.loop_decay_db_per_s,
        one_shot: r.one_shot,
        round_robin,
        choke_group: choke_bits("chokeGroup", r.choke_group.as_slice())?,
        choked_by: choke_bits("chokedBy", &r.choked_by)?,
    })
}

fn check(ok: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what()) }
}

fn choke_bits(name: &str, groups: &[u32]) -> Result<u64, String> {
    let mut bits = 0;
    for &group in groups {
        check(group < MAX_CHOKE_GROUPS, || {
            format!("{name} is {group}, must be under {MAX_CHOKE_GROUPS}")
        })?;
        bits |= 1 << group;
    }
    Ok(bits)
}

fn check_gain(name: &str, db: f32) -> Result<(), String> {
    check(db.is_finite() && db <= MAX_GAIN_DB, || {
        format!("{name} is {db}, must be a number up to {MAX_GAIN_DB}")
    })
}

fn check_ms(name: &str, ms: f32) -> Result<(), String> {
    check((0.0..=60_000.0).contains(&ms), || {
        format!("{name} is {ms}, must be 0 to 60000")
    })
}

fn check_rate(name: &str, db_per_s: f32) -> Result<(), String> {
    check((0.0..=10_000.0).contains(&db_per_s), || {
        format!("{name} is {db_per_s}, must be 0 to 10000")
    })
}

fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
