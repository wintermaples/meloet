//! Little SoundFont 2 files made in memory for the tests of the reader and the player.

use std::path::PathBuf;

use meloet_engine::Instrument;
use meloet_sound::sampler::Sampler;
use meloet_sound::sf2::Sf2File;

// Generator numbers (SoundFont 2.01, 8.1.2).
pub const START_OFFSET: u16 = 0;
pub const MOD_LFO_TO_PITCH: u16 = 5;
pub const VIB_LFO_TO_PITCH: u16 = 6;
pub const MOD_ENV_TO_PITCH: u16 = 7;
pub const FILTER_FC: u16 = 8;
pub const FILTER_Q: u16 = 9;
pub const MOD_LFO_TO_FILTER: u16 = 10;
pub const MOD_ENV_TO_FILTER: u16 = 11;
pub const MOD_LFO_TO_VOLUME: u16 = 13;
pub const PAN: u16 = 17;
pub const DELAY_MOD_LFO: u16 = 21;
pub const FREQ_MOD_LFO: u16 = 22;
pub const DELAY_VIB_LFO: u16 = 23;
pub const FREQ_VIB_LFO: u16 = 24;
pub const ATTACK_MOD_ENV: u16 = 26;
pub const HOLD_MOD_ENV: u16 = 27;
pub const DECAY_MOD_ENV: u16 = 28;
pub const SUSTAIN_MOD_ENV: u16 = 29;
pub const RELEASE_MOD_ENV: u16 = 30;
pub const DELAY_VOL_ENV: u16 = 33;
pub const ATTACK_VOL_ENV: u16 = 34;
pub const HOLD_VOL_ENV: u16 = 35;
pub const DECAY_VOL_ENV: u16 = 36;
pub const SUSTAIN_VOL_ENV: u16 = 37;
pub const RELEASE_VOL_ENV: u16 = 38;
pub const KEY_TO_VOL_ENV_HOLD: u16 = 39;
pub const KEY_TO_VOL_ENV_DECAY: u16 = 40;
pub const INSTRUMENT: u16 = 41;
pub const KEY_RANGE: u16 = 43;
pub const VEL_RANGE: u16 = 44;
pub const ATTENUATION: u16 = 48;
pub const COARSE_TUNE: u16 = 51;
pub const FINE_TUNE: u16 = 52;
pub const SAMPLE_ID: u16 = 53;
pub const SAMPLE_MODES: u16 = 54;
pub const SCALE_TUNING: u16 = 56;
pub const EXCLUSIVE_CLASS: u16 = 57;
pub const ROOT_KEY: u16 = 58;

/// The amount of a key or velocity range.
pub fn range(lo: u8, hi: u8) -> i16 {
    i16::from_le_bytes([lo, hi])
}

/// Timecents of a time in seconds.
pub fn timecents(seconds: f64) -> i16 {
    (1200.0 * seconds.log2()).round() as i16
}

pub type Zone = Vec<(u16, i16)>;

pub struct Smp {
    pub name: &'static str,
    pub data: Vec<i16>,
    pub rate: u32,
    pub root: u8,
    pub correction: i8,
    /// Frames of the sample.
    pub looped: (u32, u32),
    pub kind: u16,
}

impl Smp {
    pub fn new(name: &'static str, data: Vec<i16>, rate: u32, root: u8) -> Smp {
        let end = data.len() as u32;
        Smp {
            name,
            data,
            rate,
            root,
            correction: 0,
            looped: (0, end),
            kind: 1,
        }
    }
}

/// A zone list whose first zone has no instrument or sampleID is read as the global zone.
pub struct Inst {
    pub name: &'static str,
    pub zones: Vec<Zone>,
}

pub struct Preset {
    pub name: &'static str,
    pub bank: u16,
    pub program: u16,
    pub zones: Vec<Zone>,
}

pub struct Font {
    pub version: (u16, u16),
    pub samples: Vec<Smp>,
    pub insts: Vec<Inst>,
    pub presets: Vec<Preset>,
}

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 9);
    out.extend(id);
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}

fn list(kind: &[u8; 4], parts: &[Vec<u8>]) -> Vec<u8> {
    let mut body = kind.to_vec();
    for part in parts {
        body.extend(part);
    }
    chunk(b"LIST", &body)
}

fn name20(name: &str) -> [u8; 20] {
    let mut out = [0_u8; 20];
    out[..name.len()].copy_from_slice(name.as_bytes());
    out
}

/// The bag and generator tables of zone lists, each with its terminal record; `starts` gets
/// the first bag of every list and the terminal one.
fn bags_and_gens(lists: &[&Vec<Zone>]) -> (Vec<u16>, Vec<u8>, Vec<u8>) {
    let (mut starts, mut bags, mut gens) = (Vec::new(), Vec::new(), Vec::new());
    let (mut bag_count, mut gen_count) = (0_u16, 0_u16);
    for zones in lists {
        starts.push(bag_count);
        for zone in zones.iter() {
            bags.extend(gen_count.to_le_bytes());
            bags.extend(0_u16.to_le_bytes());
            bag_count += 1;
            for (generator, amount) in zone {
                gens.extend(generator.to_le_bytes());
                gens.extend(amount.to_le_bytes());
                gen_count += 1;
            }
        }
    }
    starts.push(bag_count);
    bags.extend(gen_count.to_le_bytes());
    bags.extend(0_u16.to_le_bytes());
    gens.extend([0_u8; 4]);
    (starts, bags, gens)
}

impl Font {
    pub fn new(samples: Vec<Smp>, insts: Vec<Inst>, presets: Vec<Preset>) -> Font {
        Font {
            version: (2, 1),
            samples,
            insts,
            presets,
        }
    }

    /// One preset 0:0 that plays one instrument of the given zones.
    pub fn simple(samples: Vec<Smp>, zones: Vec<Zone>) -> Font {
        let inst = Inst {
            name: "inst",
            zones,
        };
        let preset = Preset {
            name: "preset",
            bank: 0,
            program: 0,
            zones: vec![vec![(INSTRUMENT, 0)]],
        };
        Font::new(samples, vec![inst], vec![preset])
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut ifil = Vec::new();
        ifil.extend(self.version.0.to_le_bytes());
        ifil.extend(self.version.1.to_le_bytes());
        let info = list(
            b"INFO",
            &[
                chunk(b"ifil", &ifil),
                chunk(b"isng", b"EMU8000\0"),
                chunk(b"INAM", b"test\0\0"),
            ],
        );

        let (mut smpl, mut shdr) = (Vec::new(), Vec::new());
        for s in &self.samples {
            let start = (smpl.len() / 2) as u32;
            for x in &s.data {
                smpl.extend(x.to_le_bytes());
            }
            smpl.extend([0_u8; 92]);
            shdr.extend(name20(s.name));
            shdr.extend(start.to_le_bytes());
            shdr.extend((start + s.data.len() as u32).to_le_bytes());
            shdr.extend((start + s.looped.0).to_le_bytes());
            shdr.extend((start + s.looped.1).to_le_bytes());
            shdr.extend(s.rate.to_le_bytes());
            shdr.push(s.root);
            shdr.push(s.correction as u8);
            shdr.extend(0_u16.to_le_bytes());
            shdr.extend(s.kind.to_le_bytes());
        }
        shdr.extend(name20("EOS"));
        shdr.extend([0_u8; 26]);
        let sdta = list(b"sdta", &[chunk(b"smpl", &smpl)]);

        let preset_zones: Vec<&Vec<Zone>> = self.presets.iter().map(|p| &p.zones).collect();
        let (starts, pbag, pgen) = bags_and_gens(&preset_zones);
        let mut phdr = Vec::new();
        let headers = self.presets.iter().map(|p| (p.name, p.program, p.bank));
        for ((name, program, bank), bag) in headers.chain([("EOP", 0, 0)]).zip(&starts) {
            phdr.extend(name20(name));
            phdr.extend(program.to_le_bytes());
            phdr.extend(bank.to_le_bytes());
            phdr.extend(bag.to_le_bytes());
            phdr.extend([0_u8; 12]);
        }
        let inst_zones: Vec<&Vec<Zone>> = self.insts.iter().map(|i| &i.zones).collect();
        let (starts, ibag, igen) = bags_and_gens(&inst_zones);
        let mut inst = Vec::new();
        for (name, bag) in self
            .insts
            .iter()
            .map(|i| i.name)
            .chain(["EOI"])
            .zip(&starts)
        {
            inst.extend(name20(name));
            inst.extend(bag.to_le_bytes());
        }
        let pdta = list(
            b"pdta",
            &[
                chunk(b"phdr", &phdr),
                chunk(b"pbag", &pbag),
                chunk(b"pmod", &[0; 10]),
                chunk(b"pgen", &pgen),
                chunk(b"inst", &inst),
                chunk(b"ibag", &ibag),
                chunk(b"imod", &[0; 10]),
                chunk(b"igen", &igen),
                chunk(b"shdr", &shdr),
            ],
        );

        let mut body = b"sfbk".to_vec();
        body.extend(info);
        body.extend(sdta);
        body.extend(pdta);
        chunk(b"RIFF", &body)
    }
}

/// A file in the tests' folder, removed when the test ends.
pub struct TempFile(pub PathBuf);

impl TempFile {
    /// `name` must differ from test to test: tests run side by side.
    pub fn new(name: &str, bytes: &[u8]) -> TempFile {
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("sf2-{}-{name}.sf2", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        TempFile(path)
    }

    pub fn open(&self) -> Sf2File {
        Sf2File::open(&self.0).unwrap()
    }

    /// Preset 0:0, loaded and prepared.
    pub fn sampler(&self, rate: f32, block: usize) -> Sampler {
        let bank = self.open().load(0, 0, rate).unwrap();
        let mut sampler = Sampler::new(bank.into());
        sampler.prepare(rate, block);
        sampler
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A sine as 16-bit samples, `amp` of full scale.
pub fn sine(freq: f64, rate: u32, frames: usize, amp: f64) -> Vec<i16> {
    (0..frames)
        .map(|i| {
            let phase = std::f64::consts::TAU * freq * i as f64 / f64::from(rate);
            (amp * 32767.0 * phase.sin()).round() as i16
        })
        .collect()
}

/// The same value on every frame: what comes out is the gain.
pub fn flat(value: i16, frames: usize) -> Vec<i16> {
    vec![value; frames]
}

/// A file that uses every part of the player: both envelopes, both LFOs, the filter moved by
/// envelope and LFO, velocity layers, the three loop modes, two exclusive classes, a global
/// zone at each level and a second preset in the drum bank.
pub fn rich() -> Font {
    let tone = sine(441.0, 44_100, 22_050, 0.4);
    let mut bright = sine(2400.0, 48_000, 9600, 0.3);
    bright.extend(sine(600.0, 48_000, 2400, 0.3));
    let click = sine(1200.0, 48_000, 2400, 0.5);
    let mut looped = Smp::new("bright", bright, 48_000, 72);
    looped.looped = (480, 9600);
    let samples = vec![
        Smp::new("tone", tone, 44_100, 60),
        looped,
        Smp::new("click", click, 48_000, 255),
    ];
    let keys = vec![
        vec![
            (RELEASE_VOL_ENV, timecents(0.3)),
            (FREQ_MOD_LFO, -700),
            (PAN, -100),
        ],
        vec![
            (KEY_RANGE, range(0, 59)),
            (VEL_RANGE, range(0, 64)),
            (FILTER_FC, 8000),
            (FILTER_Q, 120),
            (MOD_ENV_TO_FILTER, 3600),
            (ATTACK_MOD_ENV, timecents(0.01)),
            (DECAY_MOD_ENV, timecents(0.3)),
            (SUSTAIN_MOD_ENV, 700),
            (RELEASE_MOD_ENV, timecents(0.2)),
            (DECAY_VOL_ENV, timecents(2.0)),
            (SUSTAIN_VOL_ENV, 200),
            (KEY_TO_VOL_ENV_DECAY, 50),
            (SAMPLE_MODES, 1),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(0, 59)),
            (VEL_RANGE, range(65, 127)),
            (DELAY_VOL_ENV, timecents(0.005)),
            (ATTACK_VOL_ENV, timecents(0.02)),
            (HOLD_VOL_ENV, timecents(0.05)),
            (KEY_TO_VOL_ENV_HOLD, 100),
            (MOD_LFO_TO_FILTER, 1200),
            (MOD_LFO_TO_VOLUME, 30),
            (MOD_LFO_TO_PITCH, 20),
            (DELAY_MOD_LFO, timecents(0.05)),
            (ATTENUATION, 60),
            (SAMPLE_MODES, 1),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(60, 95)),
            (VIB_LFO_TO_PITCH, 40),
            (FREQ_VIB_LFO, -500),
            (DELAY_VIB_LFO, timecents(0.1)),
            (MOD_ENV_TO_PITCH, -200),
            (DECAY_MOD_ENV, timecents(0.1)),
            (SUSTAIN_MOD_ENV, 1000),
            (FINE_TUNE, 7),
            (PAN, 300),
            (SAMPLE_MODES, 3),
            (SAMPLE_ID, 1),
        ],
        vec![
            (KEY_RANGE, range(96, 97)),
            (EXCLUSIVE_CLASS, 1),
            (SCALE_TUNING, 0),
            (SAMPLE_ID, 2),
        ],
        vec![
            (KEY_RANGE, range(98, 98)),
            (EXCLUSIVE_CLASS, 700),
            (FILTER_FC, 9000),
            (SAMPLE_MODES, 1),
            (SAMPLE_ID, 1),
        ],
        vec![
            (KEY_RANGE, range(99, 127)),
            (EXCLUSIVE_CLASS, 700),
            (SAMPLE_ID, 2),
        ],
    ];
    let insts = vec![
        Inst {
            name: "keys",
            zones: keys,
        },
        Inst {
            name: "drum",
            zones: vec![vec![(ROOT_KEY, 60), (SAMPLE_ID, 2)]],
        },
    ];
    let presets = vec![
        Preset {
            name: "Rich",
            bank: 0,
            program: 0,
            zones: vec![
                vec![(ATTENUATION, 20)],
                vec![(FINE_TUNE, -3), (INSTRUMENT, 0)],
            ],
        },
        Preset {
            name: "Kit",
            bank: 128,
            program: 0,
            zones: vec![vec![(KEY_RANGE, range(35, 81)), (INSTRUMENT, 1)]],
        },
    ];
    Font::new(samples, insts, presets)
}
