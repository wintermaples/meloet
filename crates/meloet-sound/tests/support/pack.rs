//! Little packs for the sample player's tests: WAV files and an instrument.json in a folder of
//! their own, removed when the test ends.

use std::path::PathBuf;
use std::sync::Arc;

use meloet_engine::{Instrument, InstrumentEvent};
use meloet_sound::sampler::{LoadError, SampleBank, Sampler, load_instrument};

pub struct TestPack {
    pub dir: PathBuf,
}

impl TestPack {
    /// `name` must differ from test to test: tests run side by side.
    pub fn new(name: &str) -> TestPack {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("sampler-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("samples")).unwrap();
        TestPack { dir }
    }

    /// Writes `samples/<name>.wav`, 16 bit. One slice is mono, two are left and right.
    pub fn wav(&self, name: &str, rate: u32, channels: &[&[f32]]) {
        let frames = channels[0].len();
        let count = channels.len() as u16;
        let data_bytes = (frames * channels.len() * 2) as u32;
        let mut bytes = Vec::with_capacity(44 + data_bytes as usize);
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_bytes).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        bytes.extend(rate.to_le_bytes());
        bytes.extend((rate * u32::from(count) * 2).to_le_bytes());
        bytes.extend((count * 2).to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(data_bytes.to_le_bytes());
        for frame in 0..frames {
            for channel in channels {
                let pcm = (channel[frame].clamp(-1.0, 1.0) * 32767.0).round() as i16;
                bytes.extend(pcm.to_le_bytes());
            }
        }
        let path = self.dir.join("samples").join(format!("{name}.wav"));
        std::fs::write(path, bytes).unwrap();
    }

    /// Writes instrument.json. `top` is the instrument's own fields besides format, version, kind
    /// and regions (may be empty); each region is the inside of its JSON object.
    pub fn instrument(&self, top: &str, regions: &[String]) -> PathBuf {
        let top = if top.is_empty() {
            String::new()
        } else {
            format!(" {top},")
        };
        let regions: Vec<String> = regions.iter().map(|r| format!("    {{{r}}}")).collect();
        self.write_json(&format!(
            "{{\"format\": \"meloet-instrument\", \"version\": 1, \"kind\": \"test\",{top}\n  \
             \"regions\": [\n{}\n  ]}}\n",
            regions.join(",\n")
        ))
    }

    pub fn write_json(&self, text: &str) -> PathBuf {
        let path = self.dir.join("instrument.json");
        std::fs::write(&path, text).unwrap();
        path
    }

    pub fn try_load(&self) -> Result<Arc<SampleBank>, LoadError> {
        load_instrument(&self.dir.join("instrument.json"), 2)
    }

    pub fn load(&self) -> Arc<SampleBank> {
        self.try_load().unwrap()
    }

    /// The instrument, loaded and prepared.
    pub fn sampler(&self, rate: f32, block: usize) -> Sampler {
        let mut sampler = Sampler::new(self.load());
        sampler.prepare(rate, block);
        sampler
    }
}

impl Drop for TestPack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A region on `sample` for keys `lo..=hi` with `root`, plus `more` fields (may be empty).
pub fn region(sample: &str, lo: u8, hi: u8, root: u8, more: &str) -> String {
    let comma = if more.is_empty() { "" } else { ", " };
    format!(
        "\"sample\": \"samples/{sample}.wav\", \"keyLo\": {lo}, \"keyHi\": {hi}, \
         \"root\": {root}{comma}{more}"
    )
}

pub fn tone(freq: f64, rate: u32, frames: usize, amp: f32) -> Vec<f32> {
    (0..frames)
        .map(|i| {
            let phase = std::f64::consts::TAU * freq * i as f64 / f64::from(rate);
            (f64::from(amp) * phase.sin()) as f32
        })
        .collect()
}

/// The same value on every frame: what comes out is the gain.
pub fn level(value: f32, frames: usize) -> Vec<f32> {
    vec![value; frames]
}

pub fn on(note: u8, velocity: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOn { note, velocity }
}

pub fn off(note: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOff { note }
}

pub fn pedal(down: bool) -> InstrumentEvent {
    InstrumentEvent::Control {
        controller: 64,
        value: if down { 127 } else { 0 },
    }
}

/// Renders `frames` in blocks of `block`; returns left and right.
pub fn play(inst: &mut dyn Instrument, frames: usize, block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut left, mut right) = (vec![0.0; frames], vec![0.0; frames]);
    for (l, r) in left.chunks_mut(block).zip(right.chunks_mut(block)) {
        inst.render(l, r);
    }
    (left, right)
}

/// A pack that uses every field: two looped velocity layers (one with a loop decay), a plain
/// range with an attack, release and pedal sounds, and drums with turns and a choke group.
/// Polyphony 8, so voices are stolen all the time.
pub fn everything(name: &str) -> TestPack {
    let pack = TestPack::new(name);
    let left = tone(441.0, 44_100, 22_050, 0.4);
    let right = tone(882.0, 44_100, 22_050, 0.3);
    pack.wav("tone", 44_100, &[&left, &right]);
    pack.wav("mono", 48_000, &[&tone(480.0, 48_000, 12_000, 0.4)]);
    pack.wav("click", 48_000, &[&tone(2400.0, 48_000, 2400, 0.3)]);
    let looped = "\"loopStart\": 4410, \"loopEnd\": 17640";
    let soft = format!("\"velHi\": 64, {looped}, \"tuneCents\": 7");
    let hard = format!("\"velLo\": 65, {looped}, \"loopDecayDbPerS\": 30, \"releaseMs\": 80");
    let up = "\"trigger\": \"release\", \"pitchKeytrack\": 0, \"rtDecayDbPerS\": 6";
    pack.instrument(
        "\"gainDb\": -3, \"polyphony\": 8, \"sustainPedal\": true",
        &[
            region("tone", 0, 59, 48, &soft),
            region("tone", 0, 59, 48, &hard),
            region(
                "mono",
                60,
                95,
                72,
                "\"attackMs\": 5, \"ampVeltrack\": 60, \"pan\": -0.3",
            ),
            region("click", 0, 95, 60, up),
            region(
                "click",
                0,
                0,
                60,
                "\"trigger\": \"pedalDown\", \"gainDb\": -6",
            ),
            region(
                "click",
                0,
                0,
                60,
                "\"trigger\": \"pedalUp\", \"gainDb\": -6",
            ),
            region(
                "click",
                96,
                96,
                96,
                "\"oneShot\": true, \"rrIndex\": 1, \"rrCount\": 2",
            ),
            region(
                "mono",
                96,
                96,
                96,
                "\"oneShot\": true, \"rrIndex\": 2, \"rrCount\": 2",
            ),
            region(
                "click",
                97,
                97,
                97,
                "\"oneShot\": true, \"chokeGroup\": 1, \"pan\": 0.5",
            ),
            region(
                "mono",
                98,
                98,
                98,
                "\"oneShot\": true, \"chokedBy\": [1], \"pan\": -1",
            ),
        ],
    );
    pack
}

/// Events that keep every part of the player busy; the same every run.
pub struct Storm(u32);

impl Storm {
    pub fn new(seed: u32) -> Storm {
        Storm(seed)
    }

    fn below(&mut self, below: u32) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) % below
    }

    pub fn next(&mut self) -> InstrumentEvent {
        let note = self.below(104) as u8;
        match self.below(20) {
            0..=8 => on(note, 1 + self.below(127) as u8),
            9..=13 => off(note),
            14 => on(96 + self.below(3) as u8, 100),
            15 => pedal(self.below(2) == 0),
            16 => InstrumentEvent::PitchBend {
                value: self.below(16_384) as i16 - 8192,
            },
            17 => InstrumentEvent::Control {
                controller: self.below(128) as u8,
                value: self.below(128) as u8,
            },
            18 => on(note, 0),
            _ if self.below(40) == 0 => InstrumentEvent::AllNotesOff,
            _ => on(note, 127),
        }
    }
}
