//! One instrument in memory: its samples as i16 and its regions, checked and ready to play.

use std::ops::Range;

use serde::Deserialize;

use super::shape::{EnvShape, LfoShape, Shape};

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
    /// Envelopes, LFOs and filter of a SoundFont zone. With it the attack, release and decay
    /// fields above are not used.
    pub shape: Option<Box<Shape>>,
}

/// One zone of a SoundFont preset in the player's own units. The files are mono.
pub(crate) struct ZoneSpec {
    /// The place of the zone's file in the list given with it.
    pub sample: u32,
    pub key_lo: u8,
    pub key_hi: u8,
    pub vel_lo: u8,
    pub vel_hi: u8,
    pub root: u8,
    pub tune_cents: f32,
    /// Cents of pitch per key.
    pub scale_cents: f32,
    pub gain_db: f32,
    /// Frames `start..end` of the file.
    pub looped: Option<(usize, usize)>,
    /// Voices of the same class stop each other; 0 is no class.
    pub exclusive_class: u16,
    pub volume: EnvShape,
    pub modulation: EnvShape,
    pub mod_to_pitch: f32,
    pub mod_to_filter: f32,
    pub vibrato: LfoShape,
    pub vibrato_to_pitch: f32,
    pub lfo: LfoShape,
    pub lfo_to_pitch: f32,
    pub lfo_to_filter: f32,
    pub lfo_to_volume_db: f32,
    pub filter_cents: f32,
    pub filter_q: f32,
    pub pan: f32,
    pub loop_until_release: bool,
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
    /// Some region has a [`Shape`].
    pub(super) shaped: bool,
}

/// Region numbers grouped by trigger and key, and the tables that point into them.
struct Tables {
    order: Vec<u32>,
    attack: Vec<Range<u32>>,
    release: Vec<Range<u32>>,
    pedal_down: Range<u32>,
    pedal_up: Range<u32>,
}

/// `spans[i]` is the trigger and the keys of region i.
fn tables(spans: &[(Trigger, u8, u8)]) -> Tables {
    let mut order = Vec::new();
    let mut table = |trigger: Trigger, key: Option<u8>| {
        let start = order.len() as u32;
        for (index, &(its, lo, hi)) in spans.iter().enumerate() {
            if its == trigger && key.is_none_or(|k| (lo..=hi).contains(&k)) {
                order.push(index as u32);
            }
        }
        start..order.len() as u32
    };
    let attack = (0..128).map(|k| table(Trigger::Attack, Some(k))).collect();
    let release = (0..128).map(|k| table(Trigger::Release, Some(k))).collect();
    let pedal_down = table(Trigger::PedalDown, None);
    let pedal_up = table(Trigger::PedalUp, None);
    Tables {
        order,
        attack,
        release,
        pedal_down,
        pedal_up,
    }
}

impl Drop for SampleBank {
    fn drop(&mut self) {
        crate::memory::note_freed(self.sample_bytes());
    }
}

impl SampleBank {
    /// The instrument's name in the pack (`grandPiano`, `studioKit`, ...), or the name of a
    /// SoundFont's preset.
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

    /// The keys a note starts a region on, lowest first.
    pub fn keys(&self) -> Vec<u8> {
        (0..=127u8)
            .filter(|&key| !self.attack[usize::from(key)].is_empty())
            .collect()
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

        let spans: Vec<_> = file
            .regions
            .iter()
            .map(|r| (r.trigger, r.key_lo, r.key_hi))
            .collect();
        let Tables {
            order,
            attack,
            release,
            pedal_down,
            pedal_up,
        } = tables(&spans);

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
            shaped: false,
        })
    }

    /// A bank from the zones of one SoundFont preset. `samples` are mono files as
    /// (frames a second, frames); `gain_db` is added to every zone. The caller has checked
    /// that every zone's file and loop are there.
    pub(crate) fn from_zones(
        name: String,
        polyphony: usize,
        gain_db: f32,
        samples: Vec<(f32, Vec<i16>)>,
        zones: Vec<ZoneSpec>,
    ) -> SampleBank {
        let samples: Vec<SampleData> = samples
            .into_iter()
            .map(|(rate, data)| SampleData {
                rate,
                channels: 1,
                frames: data.len(),
                data,
            })
            .collect();
        let mut classes: Vec<u16> = Vec::new();
        let mut spans = Vec::with_capacity(zones.len());
        let mut regions = Vec::with_capacity(zones.len());
        for z in zones {
            // Only the first 64 classes of a preset stop their voices.
            let class = match z.exclusive_class {
                0 => 0,
                class => {
                    let place = classes.iter().position(|c| *c == class).unwrap_or_else(|| {
                        classes.push(class);
                        classes.len() - 1
                    });
                    1_u64.checked_shl(place as u32).unwrap_or(0)
                }
            };
            spans.push((Trigger::Attack, z.key_lo, z.key_hi));
            regions.push(Region {
                sample: z.sample,
                vel_lo: z.vel_lo,
                vel_hi: z.vel_hi,
                root: z.root,
                tune_semitones: z.tune_cents / 100.0,
                keytrack: z.scale_cents / 100.0,
                // FluidSynth takes half of the resonance off the loudness of every voice,
                // whether its filter is heard or not: 1/sqrt(Q), which is 1.5 dB up for the
                // flat response.
                gain: db_to_gain(z.gain_db + gain_db) / z.filter_q.sqrt() / 32768.0,
                pan_left: 1.0,
                pan_right: 1.0,
                veltrack: 1.0,
                attack_s: 0.0,
                release_s: 0.0,
                rt_decay_db_per_s: 0.0,
                looped: z.looped,
                loop_decay_db_per_s: 0.0,
                one_shot: false,
                round_robin: None,
                choke_group: class,
                choked_by: class,
                shape: Some(Box::new(Shape {
                    volume: z.volume,
                    modulation: z.modulation,
                    mod_to_pitch: z.mod_to_pitch,
                    mod_to_filter: z.mod_to_filter,
                    vibrato: z.vibrato,
                    vibrato_to_pitch: z.vibrato_to_pitch,
                    lfo: z.lfo,
                    lfo_to_pitch: z.lfo_to_pitch,
                    lfo_to_filter: z.lfo_to_filter,
                    lfo_to_volume_db: z.lfo_to_volume_db,
                    filter_cents: z.filter_cents,
                    filter_q: z.filter_q,
                    pan: z.pan,
                    loop_until_release: z.loop_until_release,
                })),
            });
        }
        let Tables {
            order,
            attack,
            release,
            pedal_down,
            pedal_up,
        } = tables(&spans);
        SampleBank {
            kind: name,
            polyphony: polyphony.clamp(1, MAX_POLYPHONY as usize),
            sustain_pedal: true,
            samples,
            regions,
            order,
            attack,
            release,
            pedal_down,
            pedal_up,
            round_robin_groups: 0,
            shaped: true,
        }
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
        shape: None,
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
