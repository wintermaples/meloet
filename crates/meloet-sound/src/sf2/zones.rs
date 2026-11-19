//! The zones a preset plays: every preset zone with every zone of its instrument, after the
//! rules of the SoundFont 2.01 specification (8.5 and 9.4).

use super::file::Tables;

pub(super) const GENERATORS: usize = 61;

/// 8.1.2: the generators this reader knows, by number.
pub(super) mod generator {
    pub const START_OFFSET: usize = 0;
    pub const END_OFFSET: usize = 1;
    pub const LOOP_START_OFFSET: usize = 2;
    pub const LOOP_END_OFFSET: usize = 3;
    pub const START_COARSE_OFFSET: usize = 4;
    pub const MOD_LFO_TO_PITCH: usize = 5;
    pub const VIB_LFO_TO_PITCH: usize = 6;
    pub const MOD_ENV_TO_PITCH: usize = 7;
    pub const FILTER_FC: usize = 8;
    pub const FILTER_Q: usize = 9;
    pub const MOD_LFO_TO_FILTER: usize = 10;
    pub const MOD_ENV_TO_FILTER: usize = 11;
    pub const END_COARSE_OFFSET: usize = 12;
    pub const MOD_LFO_TO_VOLUME: usize = 13;
    pub const PAN: usize = 17;
    pub const DELAY_MOD_LFO: usize = 21;
    pub const FREQ_MOD_LFO: usize = 22;
    pub const DELAY_VIB_LFO: usize = 23;
    pub const FREQ_VIB_LFO: usize = 24;
    pub const DELAY_MOD_ENV: usize = 25;
    pub const KEY_TO_MOD_ENV_HOLD: usize = 31;
    pub const KEY_TO_MOD_ENV_DECAY: usize = 32;
    pub const DELAY_VOL_ENV: usize = 33;
    pub const KEY_TO_VOL_ENV_HOLD: usize = 39;
    pub const KEY_TO_VOL_ENV_DECAY: usize = 40;
    pub const INSTRUMENT: usize = 41;
    pub const KEY_RANGE: usize = 43;
    pub const VEL_RANGE: usize = 44;
    pub const LOOP_START_COARSE_OFFSET: usize = 45;
    pub const ATTENUATION: usize = 48;
    pub const LOOP_END_COARSE_OFFSET: usize = 50;
    pub const COARSE_TUNE: usize = 51;
    pub const FINE_TUNE: usize = 52;
    pub const SAMPLE_ID: usize = 53;
    pub const SAMPLE_MODES: usize = 54;
    pub const SCALE_TUNING: usize = 56;
    pub const EXCLUSIVE_CLASS: usize = 57;
    pub const ROOT_KEY: usize = 58;
}
use generator::*;

/// The names the specification gives the generators, in the order of their numbers; the ones
/// without a name here are unused or reserved.
pub(super) const NAMES: [&str; GENERATORS] = [
    "startAddrsOffset",
    "endAddrsOffset",
    "startloopAddrsOffset",
    "endloopAddrsOffset",
    "startAddrsCoarseOffset",
    "modLfoToPitch",
    "vibLfoToPitch",
    "modEnvToPitch",
    "initialFilterFc",
    "initialFilterQ",
    "modLfoToFilterFc",
    "modEnvToFilterFc",
    "endAddrsCoarseOffset",
    "modLfoToVolume",
    "",
    "chorusEffectsSend",
    "reverbEffectsSend",
    "pan",
    "",
    "",
    "",
    "delayModLFO",
    "freqModLFO",
    "delayVibLFO",
    "freqVibLFO",
    "delayModEnv",
    "attackModEnv",
    "holdModEnv",
    "decayModEnv",
    "sustainModEnv",
    "releaseModEnv",
    "keynumToModEnvHold",
    "keynumToModEnvDecay",
    "delayVolEnv",
    "attackVolEnv",
    "holdVolEnv",
    "decayVolEnv",
    "sustainVolEnv",
    "releaseVolEnv",
    "keynumToVolEnvHold",
    "keynumToVolEnvDecay",
    "instrument",
    "",
    "keyRange",
    "velRange",
    "startloopAddrsCoarseOffset",
    "keynum",
    "velocity",
    "initialAttenuation",
    "",
    "endloopAddrsCoarseOffset",
    "coarseTune",
    "fineTune",
    "sampleID",
    "sampleModes",
    "",
    "scaleTuning",
    "exclusiveClass",
    "overridingRootKey",
    "",
    "",
];

/// 8.1.3: what a generator is when no zone sets it.
fn default_of(generator: usize) -> i32 {
    match generator {
        FILTER_FC => 13_500,
        // the delays, attacks, holds, decays and releases of the LFOs and envelopes
        DELAY_MOD_LFO | DELAY_VIB_LFO | 25..=28 | 30 | 33..=36 | 38 => -12_000,
        // keynum, velocity
        46 | 47 => -1,
        SCALE_TUNING => 100,
        ROOT_KEY => -1,
        _ => 0,
    }
}

/// 8.5: generators that mean nothing in a preset zone (sample addresses, keynum, velocity,
/// sampleModes, exclusiveClass, overridingRootKey).
fn not_in_preset(generator: usize) -> bool {
    matches!(
        generator,
        0..=4 | 12 | 45 | 46 | 47 | 50 | SAMPLE_MODES | EXCLUSIVE_CLASS | ROOT_KEY
    )
}

/// A preset whose tables are broken could name the same zones again and again.
const MAX_ZONES: usize = 16_384;

/// The generators one zone of the file sets.
#[derive(Clone, Copy)]
struct Gens {
    set: u64,
    amount: [u16; GENERATORS],
    /// The last generator of the zone, known to this reader or not.
    last: Option<u16>,
}

impl Gens {
    const NONE: Gens = Gens {
        set: 0,
        amount: [0; GENERATORS],
        last: None,
    };

    fn has(&self, generator: usize) -> bool {
        self.set & (1 << generator) != 0
    }

    /// 8.1.1: a signed 16-bit amount.
    fn value(&self, generator: usize) -> Option<i32> {
        self.has(generator)
            .then(|| i32::from(self.amount[generator] as i16))
    }

    /// A key or velocity range is two bytes, low then high. Without one the zone takes all.
    fn range(&self, generator: usize) -> (u8, u8) {
        if self.has(generator) {
            let [lo, hi] = self.amount[generator].to_le_bytes();
            (lo, hi)
        } else {
            (0, 127)
        }
    }

    /// What `over` sets replaces what `self` sets.
    fn under(&self, over: &Gens) -> Gens {
        let mut both = *self;
        for g in 0..GENERATORS {
            if over.has(g) {
                both.amount[g] = over.amount[g];
            }
        }
        both.set |= over.set;
        both
    }
}

/// The global zone and the local zones of one preset or instrument, whose bags are
/// `first..last` (7.3 and 7.7).
fn zones_of(
    bags: &[u16],
    gens: &[(u16, u16)],
    first: usize,
    last: usize,
    terminal: usize,
) -> (Option<Gens>, Vec<Gens>) {
    // The bag after a zone's own says where its generators end.
    if first >= last || last >= bags.len() || last - first > MAX_ZONES {
        return (None, Vec::new());
    }
    let mut zones = Vec::with_capacity(last - first);
    for bag in first..last {
        let end = usize::from(bags[bag + 1]).min(gens.len());
        let start = usize::from(bags[bag]).min(end);
        let mut zone = Gens::NONE;
        for &(generator, amount) in &gens[start..end] {
            if let Some(slot) = zone.amount.get_mut(usize::from(generator)) {
                *slot = amount;
                zone.set |= 1 << generator;
            }
            zone.last = Some(generator);
        }
        zones.push(zone);
    }
    // The first of several zones is the global one if it does not end with the terminal
    // generator (instrument, sampleID).
    let global =
        (zones.len() > 1 && zones[0].last != Some(terminal as u16)).then(|| zones.remove(0));
    // A local zone without its terminal generator is ignored.
    zones.retain(|z| z.has(terminal));
    (global, zones)
}

/// One zone a preset plays, with every generator worked out.
pub(super) struct Zone {
    /// The place of the sample's header in the file's table.
    pub sample: usize,
    pub inst: usize,
    pub key_lo: u8,
    pub key_hi: u8,
    pub vel_lo: u8,
    pub vel_hi: u8,
    /// By generator number. Ranges, instrument and sampleID are not in here.
    pub amount: [i32; GENERATORS],
}

/// Every (preset zone x instrument zone) of the preset at `index` of the header table.
/// - Instrument level: a local generator replaces the instrument's global one; one that
///   neither sets has its default.
/// - Preset level: a local generator replaces the preset's global one; the result is added to
///   the instrument's value, except the key and velocity ranges, which narrow it.
///
/// A zone that points outside the tables is left out.
pub(super) fn zones(tables: &Tables, index: usize) -> Vec<Zone> {
    let mut out = Vec::new();
    let (Some(preset), Some(next)) = (tables.presets.get(index), tables.presets.get(index + 1))
    else {
        return out;
    };
    let (preset_global, preset_local) = zones_of(
        &tables.pbag,
        &tables.pgen,
        usize::from(preset.bag),
        usize::from(next.bag),
        INSTRUMENT,
    );
    for local in &preset_local {
        let pg = preset_global.map_or(*local, |global| global.under(local));
        let inst = usize::from(pg.amount[INSTRUMENT]);
        let (Some(header), Some(next)) = (tables.insts.get(inst), tables.insts.get(inst + 1))
        else {
            continue;
        };
        let (inst_global, inst_local) = zones_of(
            &tables.ibag,
            &tables.igen,
            usize::from(header.bag),
            usize::from(next.bag),
            SAMPLE_ID,
        );
        for local in &inst_local {
            let ig = inst_global.map_or(*local, |global| global.under(local));
            let sample = usize::from(ig.amount[SAMPLE_ID]);
            if sample >= tables.samples.len() {
                continue;
            }
            let (pk, pv) = (pg.range(KEY_RANGE), pg.range(VEL_RANGE));
            let (ik, iv) = (ig.range(KEY_RANGE), ig.range(VEL_RANGE));
            let (key_lo, key_hi) = (pk.0.max(ik.0), pk.1.min(ik.1));
            let (vel_lo, vel_hi) = (pv.0.max(iv.0), pv.1.min(iv.1));
            if key_lo > key_hi || vel_lo > vel_hi {
                continue;
            }
            let mut amount = [0; GENERATORS];
            for (g, value) in amount.iter_mut().enumerate() {
                if matches!(g, KEY_RANGE | VEL_RANGE | INSTRUMENT | SAMPLE_ID) {
                    continue;
                }
                *value = ig.value(g).unwrap_or(default_of(g));
                if !not_in_preset(g) {
                    *value += pg.value(g).unwrap_or(0);
                }
            }
            out.push(Zone {
                sample,
                inst,
                key_lo,
                key_hi,
                vel_lo,
                vel_hi,
                amount,
            });
            if out.len() >= MAX_ZONES {
                return out;
            }
        }
    }
    out
}
