//! The output sets of a plugin (docs/architecture.md 20-15): which of its audio outputs sound
//! together, where each set is heard by default, and the person's level, pan and on/off for
//! each. A format says which outputs form a set; the mixing down to meloet's stereo pair, the
//! moves over 20 ms and the capture for measuring are here, the same for every format.
//!
//! With every set at its default the pair is exactly what [`fold`] makes of the outputs heard
//! by default, bit for bit: a song whose sets were never touched sounds as it always did.

use std::f32::consts::FRAC_PI_2;

use meloet_plugin::wire::{CapturedSet, MAX_BLOB_LEN, OutputSet, OutputSetting, OutputSettings};

/// Where an audio output of the plugin goes in meloet's stereo pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Both,
}

impl Side {
    /// The pan that puts a set of one output there.
    pub fn pan(self) -> f32 {
        match self {
            Side::Left => -1.0,
            Side::Right => 1.0,
            Side::Both => 0.0,
        }
    }
}

/// Mixes the outputs heard by default down to `left` and `right`: `out` holds every output
/// of the plugin, `stride` frames apart. One or two outputs are copied as they are.
pub fn fold(
    heard: &[(usize, Side)],
    out: &[f32],
    stride: usize,
    left: &mut [f32],
    right: &mut [f32],
) {
    let frames = left.len();
    let channel = |at: usize| &out[at * stride..][..frames];
    match *heard {
        [(only, _)] => {
            left.copy_from_slice(channel(only));
            right.copy_from_slice(channel(only));
        }
        [(l, _), (r, _)] => {
            left.copy_from_slice(channel(l));
            right.copy_from_slice(channel(r));
        }
        _ => {
            left.fill(0.0);
            right.fill(0.0);
            for &(at, side) in heard {
                let channel = channel(at);
                if side != Side::Right {
                    for (sum, x) in left.iter_mut().zip(channel) {
                        *sum += x;
                    }
                }
                if side != Side::Left {
                    for (sum, x) in right.iter_mut().zip(channel) {
                        *sum += x;
                    }
                }
            }
            not_a_number_is_silence(left, right);
        }
    }
}

fn not_a_number_is_silence(left: &mut [f32], right: &mut [f32]) {
    for sum in left.iter_mut().chain(right.iter_mut()) {
        *sum = if sum.is_finite() { *sum } else { 0.0 };
    }
}

/// What a pan multiplies left and right by: the law of a track's pan
/// (`meloet_engine::pan_gains`), which this process does not link.
pub fn pan_gains(pan: f32) -> [f32; 2] {
    let down = |amount: f32| {
        if amount >= 1.0 {
            0.0
        } else {
            (amount * FRAC_PI_2).cos()
        }
    };
    if !pan.is_finite() {
        [1.0, 1.0]
    } else if pan > 0.0 {
        [down(pan), 1.0]
    } else {
        [1.0, down(-pan)]
    }
}

/// How long a set takes to reach a new level, pan or on/off, as a mixer strip does.
const RAMP_S: f64 = 0.020;
const LOWEST_DB: f32 = -60.0;
const HIGHEST_DB: f32 = 6.0;

/// The outputs of one set, by their place among the plugin's audio outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ports {
    Mono(usize),
    Stereo { left: usize, right: usize },
}

impl Ports {
    pub fn first(self) -> usize {
        match self {
            Ports::Mono(at) => at,
            Ports::Stereo { left, right } => left.min(right),
        }
    }
}

/// One output set as a format describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct SetShape {
    pub id: String,
    pub name: String,
    pub ports: Ports,
    /// Heard by default.
    pub on: bool,
    /// Where it is heard by default: of one output, its place; of two, the balance.
    pub pan: f32,
}

impl SetShape {
    fn info(&self) -> OutputSet {
        OutputSet {
            id: self.id.clone(),
            name: self.name.clone(),
            channels: match self.ports {
                Ports::Mono(_) => 1,
                Ports::Stereo { .. } => 2,
            },
            on: self.on,
            pan: self.pan,
        }
    }

    fn default_gains(&self) -> [f32; 2] {
        if self.on {
            pan_gains(self.pan)
        } else {
            [0.0, 0.0]
        }
    }
}

/// The outputs heard by default and where, as the sets say: what [`fold`] is given for them.
#[cfg(test)]
pub fn heard_by_default(sets: &[SetShape]) -> Vec<(usize, Side)> {
    let mut heard = Vec::new();
    for set in sets.iter().filter(|set| set.on) {
        match set.ports {
            Ports::Mono(at) => {
                let side = [Side::Left, Side::Right, Side::Both]
                    .into_iter()
                    .find(|side| side.pan() == set.pan)
                    .expect("a set of one output heard by default is left, right or both");
                heard.push((at, side));
            }
            Ports::Stereo { left, right } => {
                assert_eq!(set.pan, 0.0, "{}", set.id);
                heard.push((left, Side::Left));
                heard.push((right, Side::Right));
            }
        }
    }
    heard.sort_by_key(|&(at, _)| at);
    heard
}

/// A gain that goes to its target in a straight line, one step a frame. Its value is worked
/// out from the frames still to go, so it ends on the target exactly.
#[derive(Debug, Clone, Copy)]
struct Ramp {
    target: f32,
    step: f32,
    left: u32,
}

impl Ramp {
    fn at(value: f32) -> Ramp {
        Ramp {
            target: value,
            step: 0.0,
            left: 0,
        }
    }

    fn now(&self) -> f32 {
        if self.left == 0 {
            self.target
        } else {
            self.target - self.step * self.left as f32
        }
    }

    /// `frames` 0: at once.
    fn to(&mut self, target: f32, frames: u32) {
        let from = self.now();
        self.target = target;
        if frames == 0 || from == target {
            self.snap();
        } else {
            self.step = (target - from) / frames as f32;
            self.left = frames;
        }
    }

    fn snap(&mut self) {
        self.step = 0.0;
        self.left = 0;
    }

    fn steady(&self) -> bool {
        self.left == 0
    }

    #[inline]
    fn next(&mut self) -> f32 {
        self.left = self.left.saturating_sub(1);
        self.now()
    }

    fn skip(&mut self, frames: usize) {
        self.left = self
            .left
            .saturating_sub(u32::try_from(frames).unwrap_or(u32::MAX));
    }
}

struct Set {
    ports: Ports,
    /// Its gains by the default rule.
    default: [f32; 2],
    /// Its gains in a capture: at 0 dB where the rule puts it.
    alone: [f32; 2],
    gains: [Ramp; 2],
}

impl Set {
    fn steady(&self) -> bool {
        self.gains.iter().all(Ramp::steady)
    }

    fn at_default(&self) -> bool {
        self.steady()
            && self.gains[0].target == self.default[0]
            && self.gains[1].target == self.default[1]
    }

    fn silent(&self) -> bool {
        self.steady() && self.gains.iter().all(|gain| gain.target == 0.0)
    }
}

/// The sound of each output set, kept for measuring (`Request::Capture`).
pub struct Capture {
    ids: Vec<String>,
    /// Room for this many frames a set.
    frames: usize,
    written: usize,
    /// Set after set, `frames` frames each, left and right in turn.
    sound: Box<[f32]>,
    /// By set: the format could not measure it in a block ([`Outputs::not_measured`]); it is
    /// left out of the answer.
    unmeasured: Box<[bool]>,
}

impl Capture {
    /// Room for `frames` frames of each set `ids` names. `Err` when what it would answer is
    /// longer than a message may be.
    pub fn new(ids: Vec<String>, frames: u64) -> Result<Capture, String> {
        let bytes_a_frame = ids.len() as u64 * 8;
        if bytes_a_frame > 0 && frames > MAX_BLOB_LEN as u64 / bytes_a_frame {
            return Err(format!(
                "{frames} frames of {} output sets are more than one message holds; {} at most",
                ids.len(),
                MAX_BLOB_LEN as u64 / bytes_a_frame
            ));
        }
        let frames = frames as usize;
        let sound = vec![0.0f32; ids.len() * frames * 2].into_boxed_slice();
        let unmeasured = vec![false; ids.len()].into_boxed_slice();
        Ok(Capture {
            ids,
            frames,
            written: 0,
            sound,
            unmeasured,
        })
    }

    /// What `Reply::Captured` holds: each set measured with the frames written, and the blob.
    pub fn reply(&self) -> (Vec<CapturedSet>, Vec<u8>) {
        let measured = || (0..self.ids.len()).filter(|&set| !self.unmeasured[set]);
        let sets = measured()
            .map(|set| CapturedSet {
                id: self.ids[set].clone(),
                frames: self.written as u64,
            })
            .collect();
        let mut blob = Vec::with_capacity(self.ids.len() * self.written * 8);
        for set in measured() {
            let from = set * self.frames * 2;
            for sample in &self.sound[from..from + self.written * 2] {
                blob.extend_from_slice(&sample.to_le_bytes());
            }
        }
        (sets, blob)
    }

    /// Each set's id and what was kept of it, read back from [`Capture::reply`]: left and
    /// right in turn.
    #[cfg(test)]
    pub fn sounds(&self) -> Vec<(String, Vec<f32>)> {
        let (sets, blob) = self.reply();
        let floats: Vec<f32> = (blob.as_chunks::<4>().0.iter())
            .map(|&b| f32::from_le_bytes(b))
            .collect();
        let mut at = 0;
        (sets.into_iter())
            .map(|set| {
                let n = set.frames as usize * 2;
                at += n;
                (set.id, floats[at - n..at].to_vec())
            })
            .collect()
    }

    /// Keeps the next frames of each set: `sound(set, frame)` gives that frame's left and
    /// right (silence when `None`). Stops when full.
    fn keep(&mut self, frames: usize, mut sound: Option<impl FnMut(usize, usize) -> (f32, f32)>) {
        let n = frames.min(self.frames - self.written);
        if n == 0 {
            return;
        }
        let from = self.written * 2;
        for (at, set) in self.sound.chunks_exact_mut(self.frames * 2).enumerate() {
            let into = &mut set[from..from + n * 2];
            match sound.as_mut() {
                None => into.fill(0.0),
                Some(sound) => {
                    for (frame, pair) in into.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                        let (left, right) = sound(at, frame);
                        pair[0] = if left.is_finite() { left } else { 0.0 };
                        pair[1] = if right.is_finite() { right } else { 0.0 };
                    }
                }
            }
        }
        self.written += n;
    }
}

/// A plugin's output sets on the audio thread: the gains each is heard with now, and the
/// capture. The main thread changes them ([`Outputs::set`], [`Outputs::arm`]) only while no
/// block runs at the plugin (`crate::rt`); nothing here allocates on the audio thread.
pub struct Outputs {
    info: Box<[OutputSet]>,
    sets: Box<[Set]>,
    /// The outputs heard by default and where ([`fold`]).
    heard: Box<[(usize, Side)]>,
    /// Takes the person's settings. An effect is heard by the default rule whatever they say.
    adjustable: bool,
    /// [`RAMP_S`] in frames.
    ramp_frames: u32,
    capture: Option<Capture>,
}

impl Outputs {
    /// `heard` must be what `sets` give by default: the outputs of the sets that are on, a
    /// set of one on the side its pan says, a pair left and right.
    pub fn new(
        sets: Vec<SetShape>,
        heard: Vec<(usize, Side)>,
        adjustable: bool,
        sample_rate: f64,
    ) -> Outputs {
        let mut sets = sets;
        sets.sort_by_key(|set| set.ports.first());
        Outputs {
            info: sets.iter().map(SetShape::info).collect(),
            sets: (sets.iter())
                .map(|set| {
                    let default = set.default_gains();
                    Set {
                        ports: set.ports,
                        default,
                        alone: match set.ports {
                            Ports::Mono(_) => pan_gains(set.pan),
                            Ports::Stereo { .. } => [1.0, 1.0],
                        },
                        gains: default.map(Ramp::at),
                    }
                })
                .collect(),
            heard: heard.into_boxed_slice(),
            adjustable,
            ramp_frames: ((RAMP_S * sample_rate).round() as u32).max(1),
            capture: None,
        }
    }

    /// The sets as `LoadedPlugin::outputs` tells them, in port order.
    pub fn list(&self) -> Vec<OutputSet> {
        self.info.to_vec()
    }

    pub fn heard(&self) -> &[(usize, Side)] {
        &self.heard
    }

    pub fn ids(&self) -> Vec<String> {
        self.info.iter().map(|set| set.id.clone()).collect()
    }

    /// The person's settings, in place of the ones before: over [`RAMP_S`] when `ramp`, at
    /// once when not. A setting of no set here is passed over.
    pub fn set(&mut self, settings: &OutputSettings, ramp: bool) {
        // Every rule is read as rule 1, the only one so far.
        let frames = if ramp { self.ramp_frames } else { 0 };
        for (info, set) in self.info.iter().zip(self.sets.iter_mut()) {
            let target = match setting_of(info, settings) {
                Some(setting) if self.adjustable => gains(info, setting),
                _ => set.default,
            };
            for (gain, target) in set.gains.iter_mut().zip(target) {
                gain.to(target, frames);
            }
        }
    }

    /// Every gain at its target at once.
    pub fn snap(&mut self) {
        for set in self.sets.iter_mut() {
            for gain in &mut set.gains {
                gain.snap();
            }
        }
    }

    /// From the next block on, keeps the sound of each set in `capture` (made for
    /// [`Outputs::ids`]); the one kept before, if any, comes back.
    pub fn arm(&mut self, capture: Capture) -> Option<Capture> {
        debug_assert_eq!(capture.ids.len(), self.sets.len());
        self.capture.replace(capture)
    }

    pub fn take_capture(&mut self) -> Option<Capture> {
        self.capture.take()
    }

    /// The set is neither heard nor about to be: its gains are steady at 0.
    pub fn silent(&self, set: usize) -> bool {
        self.sets.get(set).is_some_and(Set::silent)
    }

    /// The format cannot measure the set in this block: the capture leaves it out. Before
    /// [`Outputs::mix`] or [`Outputs::missed`] of the block.
    pub fn not_measured(&mut self, set: usize) {
        if let Some(seen) = (self.capture.as_mut()).and_then(|c| c.unmeasured.get_mut(set)) {
            *seen = true;
        }
    }

    /// Mixes the block the plugin made down to `left` and `right`: `out` holds every audio
    /// output of the plugin, `stride` frames apart.
    pub fn mix(&mut self, out: &[f32], stride: usize, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len();
        let channel = |at: usize| &out[at * stride..][..frames];
        if let Some(capture) = self.capture.as_mut() {
            let sets = &self.sets;
            capture.keep(
                frames,
                Some(|set: usize, frame: usize| {
                    let Some(set) = sets.get(set) else {
                        return (0.0, 0.0);
                    };
                    let (l, r) = match set.ports {
                        Ports::Mono(at) => (channel(at)[frame], channel(at)[frame]),
                        Ports::Stereo { left, right } => {
                            (channel(left)[frame], channel(right)[frame])
                        }
                    };
                    (l * set.alone[0], r * set.alone[1])
                }),
            );
        }
        if self.sets.iter().all(Set::at_default) {
            fold(&self.heard, out, stride, left, right);
            return;
        }
        left.fill(0.0);
        right.fill(0.0);
        for set in self.sets.iter_mut() {
            if set.silent() {
                continue;
            }
            let (from_left, from_right) = match set.ports {
                Ports::Mono(at) => (channel(at), channel(at)),
                Ports::Stereo { left, right } => (channel(left), channel(right)),
            };
            if set.steady() {
                add(left, from_left, set.gains[0].now());
                add(right, from_right, set.gains[1].now());
                continue;
            }
            let [gain_left, gain_right] = &mut set.gains;
            for frame in 0..frames {
                left[frame] += from_left[frame] * gain_left.next();
                right[frame] += from_right[frame] * gain_right.next();
            }
        }
        not_a_number_is_silence(left, right);
    }

    /// A block in which the plugin did not run: the gains move on, a capture keeps silence.
    pub fn missed(&mut self, frames: usize) {
        for set in self.sets.iter_mut() {
            for gain in &mut set.gains {
                gain.skip(frames);
            }
        }
        if let Some(capture) = self.capture.as_mut() {
            capture.keep(frames, None::<fn(usize, usize) -> (f32, f32)>);
        }
    }
}

fn add(sum: &mut [f32], x: &[f32], gain: f32) {
    if gain == 0.0 {
        return;
    }
    for (sum, x) in sum.iter_mut().zip(x) {
        *sum += x * gain;
    }
}

fn setting_of<'a>(set: &OutputSet, settings: &'a OutputSettings) -> Option<&'a OutputSetting> {
    settings.sets.iter().find(|setting| setting.id == set.id)
}

/// Whether the settings have the set heard (of an instrument).
pub fn is_on(set: &OutputSet, settings: &OutputSettings) -> bool {
    setting_of(set, settings)
        .and_then(|setting| setting.on)
        .unwrap_or(set.on)
}

/// Left and right gains of a set the person set.
fn gains(info: &OutputSet, setting: &OutputSetting) -> [f32; 2] {
    if !setting.on.unwrap_or(info.on) {
        return [0.0, 0.0];
    }
    let pan = (setting.pan)
        .filter(|pan| pan.is_finite())
        .map_or(info.pan, |pan| pan.clamp(-1.0, 1.0));
    let db = if setting.gain_db.is_finite() {
        setting.gain_db.clamp(LOWEST_DB, HIGHEST_DB)
    } else {
        0.0
    };
    let level = if db == 0.0 {
        1.0
    } else {
        10f32.powf(db / 20.0)
    };
    pan_gains(pan).map(|gain| gain * level)
}

#[cfg(test)]
mod tests {
    use meloet_plugin::wire::OutputSetting;

    use super::Side::{Both, Left, Right};
    use super::*;
    use crate::rtcheck;

    const RATE: f64 = 48_000.0;
    /// [`RAMP_S`] at [`RATE`].
    const RAMP: usize = 960;

    /// Five outputs in four sets, as the fake `outs` has them: `a` a pair, `b` in the middle,
    /// `c` on the left, `d` not heard.
    fn shapes() -> (Vec<SetShape>, Vec<(usize, Side)>) {
        let set = |id: &str, ports, on, pan| SetShape {
            id: id.into(),
            name: id.to_uppercase(),
            ports,
            on,
            pan,
        };
        let sets = vec![
            set("a", Ports::Stereo { left: 0, right: 1 }, true, 0.0),
            set("b", Ports::Mono(2), true, 0.0),
            set("c", Ports::Mono(3), true, -1.0),
            set("d", Ports::Mono(4), false, 0.0),
        ];
        let heard = vec![(0, Left), (1, Right), (2, Both), (3, Left)];
        assert_eq!(heard_by_default(&sets), heard);
        (sets, heard)
    }

    fn outputs() -> Outputs {
        let (sets, heard) = shapes();
        Outputs::new(sets, heard, true, RATE)
    }

    const LEVELS: [f32; 5] = [0.0625, 0.125, 0.03125, 0.25, 0.5];

    /// Every output `frames` long, all of it its level.
    fn constant(frames: usize) -> Vec<f32> {
        LEVELS.iter().flat_map(|&x| vec![x; frames]).collect()
    }

    fn mixed(outputs: &mut Outputs, out: &[f32], frames: usize) -> (Vec<f32>, Vec<f32>) {
        let (mut left, mut right) = (vec![f32::NAN; frames], vec![f32::NAN; frames]);
        outputs.mix(out, frames, &mut left, &mut right);
        (left, right)
    }

    fn setting(id: &str, gain_db: f32, pan: Option<f32>, on: Option<bool>) -> OutputSetting {
        OutputSetting {
            id: id.into(),
            gain_db,
            pan,
            on,
        }
    }

    fn settings(sets: Vec<OutputSetting>) -> OutputSettings {
        OutputSettings { rule: 1, sets }
    }

    fn bits(x: &[f32]) -> Vec<u32> {
        x.iter().map(|x| x.to_bits()).collect()
    }

    /// The pair as the settings say, from [`LEVELS`] with every gain steady.
    fn steady(settings: &OutputSettings) -> (f32, f32) {
        let mut outputs = outputs();
        outputs.set(settings, false);
        let (left, right) = mixed(&mut outputs, &constant(4), 4);
        (left[3], right[3])
    }

    #[test]
    fn the_pan_law_is_the_tracks() {
        assert_eq!(meloet_engine::pan_gains(0.0), [1.0, 1.0]);
        assert_eq!(meloet_engine::pan_gains(-1.0), [1.0, 0.0]);
        assert_eq!(meloet_engine::pan_gains(1.0), [0.0, 1.0]);
        let mut pans: Vec<f32> = (-2000..=2000).map(|n| n as f32 / 1000.0).collect();
        pans.extend([
            -0.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            1e-30,
            -1e-30,
        ]);
        for pan in pans {
            assert_eq!(
                pan_gains(pan).map(f32::to_bits),
                meloet_engine::pan_gains(pan).map(f32::to_bits),
                "{pan}"
            );
        }
    }

    #[test]
    fn level_pan_and_on_off_show_in_the_pair() {
        let [a_l, a_r, b, c, d] = LEVELS;
        let none = settings(Vec::new());
        assert_eq!(steady(&none), (a_l + b + c, a_r + b));
        // A level.
        let half = 10f32.powf(-6.0 / 20.0);
        let quieter = settings(vec![setting("c", -6.0, None, None)]);
        assert_eq!(steady(&quieter), (a_l + b + c * half, a_r + b));
        // Of one output, the pan is its place.
        let [gl, gr] = pan_gains(0.5);
        let moved = settings(vec![setting("c", 0.0, Some(0.5), None)]);
        assert_eq!(steady(&moved), (a_l + b + c * gl, a_r + b + c * gr));
        assert_eq!(gr, 1.0);
        // Of two, the balance.
        let [gl, gr] = pan_gains(-0.5);
        let turned = settings(vec![setting("a", 0.0, Some(-0.5), None)]);
        assert_eq!(steady(&turned), (a_l * gl + b + c, a_r * gr + b));
        assert_eq!(gl, 1.0);
        // On and off.
        let off = settings(vec![setting("a", 0.0, None, Some(false))]);
        assert_eq!(steady(&off), (b + c, b));
        let on = settings(vec![setting("d", 0.0, None, Some(true))]);
        assert_eq!(steady(&on), (a_l + b + c + d, a_r + b + d));
        // The ends of the ranges.
        let [gl, gr] = pan_gains(1.0);
        let far = settings(vec![setting("d", 40.0, Some(3.0), Some(true))]);
        let loudest = 10f32.powf(6.0 / 20.0);
        assert_eq!(
            steady(&far),
            (a_l + b + c + d * gl * loudest, a_r + b + d * gr * loudest)
        );
        let lowest = settings(vec![setting("c", -200.0, None, None)]);
        let thousandth = 10f32.powf(-60.0 / 20.0);
        assert_eq!(steady(&lowest), (a_l + b + c * thousandth, a_r + b));
        // What is not a number: 0 dB, and the pan the rule gives.
        let odd = settings(vec![setting("c", f32::NAN, Some(f32::NAN), None)]);
        assert_eq!(steady(&odd), steady(&none));
    }

    /// Outputs with the values a plugin may give that a sum could change: −0.0, values below
    /// the smallest normal, what is not a number.
    fn awkward(outputs: usize, frames: usize) -> Vec<f32> {
        let values = [
            -0.0f32,
            1.0e-40,
            -1.0e-42,
            f32::from_bits(0x7fc0_1234),
            f32::INFINITY,
            0.25,
            -0.5,
            0.0,
            f32::MIN_POSITIVE,
            -3.0,
        ];
        (0..outputs * frames)
            .map(|n| values[(n * 7 + n / frames) % values.len()])
            .collect()
    }

    fn mono(id: &str, at: usize, on: bool, pan: f32) -> SetShape {
        SetShape {
            id: id.into(),
            name: id.to_uppercase(),
            ports: Ports::Mono(at),
            on,
            pan,
        }
    }

    #[test]
    fn with_every_set_at_its_default_the_pair_is_fold_bit_for_bit() {
        let one = vec![mono("out", 0, true, 0.0)];
        let two = vec![SetShape {
            id: "l".into(),
            name: "Main".into(),
            ports: Ports::Stereo { left: 0, right: 1 },
            on: true,
            pan: 0.0,
        }];
        let two_alone = vec![
            mono("l", 0, true, -1.0),
            mono("r", 1, true, 1.0),
            mono("x", 2, false, 0.0),
        ];
        let (five, _) = shapes();
        let frames = 37;
        for sets in [one, two, two_alone, five] {
            let heard = heard_by_default(&sets);
            let count = sets.iter().map(|s| s.ports.first() + 2).max().unwrap();
            let out = awkward(count, frames);
            let (mut left, mut right) = (vec![0.0; frames], vec![0.0; frames]);
            fold(&heard, &out, frames, &mut left, &mut right);
            let want = (bits(&left), bits(&right));
            let ids: Vec<String> = sets.iter().map(|s| s.id.clone()).collect();
            let at_defaults = settings(ids.iter().map(|id| setting(id, 0.0, None, None)).collect());
            let away = settings(
                (ids.iter())
                    .map(|id| setting(id, -3.0, Some(0.3), Some(true)))
                    .collect(),
            );
            for adjustable in [true, false] {
                let mut outputs = Outputs::new(sets.clone(), heard.clone(), adjustable, RATE);
                let got = mixed(&mut outputs, &out, frames);
                assert_eq!((bits(&got.0), bits(&got.1)), want);
                outputs.set(&at_defaults, true);
                let got = mixed(&mut outputs, &out, frames);
                assert_eq!((bits(&got.0), bits(&got.1)), want);
                // Away and back again: once the move is over, fold again.
                outputs.set(&away, true);
                for _ in 0..RAMP {
                    mixed(&mut outputs, &out, 1);
                }
                outputs.set(&settings(Vec::new()), true);
                for _ in 0..RAMP {
                    mixed(&mut outputs, &out, 1);
                }
                let got = mixed(&mut outputs, &out, frames);
                assert_eq!((bits(&got.0), bits(&got.1)), want);
            }
        }
    }

    #[test]
    fn an_unknown_set_or_rule_changes_nothing() {
        let out = awkward(5, 64);
        let (mut left, mut right) = (vec![0.0; 64], vec![0.0; 64]);
        let (_, heard) = shapes();
        fold(&heard, &out, 64, &mut left, &mut right);
        let mut unknown = outputs();
        unknown.set(
            &OutputSettings {
                rule: 7,
                sets: vec![setting("nowhere", -20.0, Some(1.0), Some(false))],
            },
            true,
        );
        let got = mixed(&mut unknown, &out, 64);
        assert_eq!((bits(&got.0), bits(&got.1)), (bits(&left), bits(&right)));
        // A rule not known is read as rule 1.
        let quieter = vec![setting("c", -6.0, None, None)];
        let by_rule = |rule| {
            let mut outputs = outputs();
            outputs.set(
                &OutputSettings {
                    rule,
                    sets: quieter.clone(),
                },
                false,
            );
            let (left, right) = mixed(&mut outputs, &constant(4), 4);
            (bits(&left), bits(&right))
        };
        assert_eq!(by_rule(1), by_rule(99));
        assert_ne!(by_rule(1), {
            let (left, right) = steady(&settings(Vec::new()));
            (vec![left.to_bits(); 4], vec![right.to_bits(); 4])
        });
    }

    #[test]
    fn an_effect_is_heard_by_the_rule_whatever_is_set() {
        let (sets, heard) = shapes();
        let mut effect = Outputs::new(sets, heard, false, RATE);
        effect.set(
            &settings(vec![
                setting("a", -20.0, None, Some(false)),
                setting("d", 0.0, None, Some(true)),
            ]),
            false,
        );
        let (left, right) = mixed(&mut effect, &constant(4), 4);
        let [a_l, a_r, b, c, _] = LEVELS;
        assert_eq!((left[0], right[0]), (a_l + b + c, a_r + b));
    }

    /// Runs `blocks` blocks of `frames` with the constant outputs, `change` before each; the
    /// pair, frame after frame.
    fn play(
        outputs: &mut Outputs,
        frames: usize,
        blocks: usize,
        mut change: impl FnMut(usize, &mut Outputs),
    ) -> (Vec<f32>, Vec<f32>) {
        let out = constant(frames);
        let (mut all_left, mut all_right) = (Vec::new(), Vec::new());
        for block in 0..blocks {
            change(block, outputs);
            let (left, right) = mixed(outputs, &out, frames);
            all_left.extend(left);
            all_right.extend(right);
        }
        (all_left, all_right)
    }

    /// No step between two frames is larger than `most`, give or take the rounding of f32.
    fn smooth(x: &[f32], most: f32) {
        for (at, pair) in x.windows(2).enumerate() {
            let step = (pair[1] - pair[0]).abs();
            assert!(
                step <= most * 1.0001 + 1e-7,
                "frame {}: a step of {step}, more than {most}",
                at + 1
            );
        }
    }

    #[test]
    fn a_change_moves_in_a_straight_line_over_20_ms() {
        let [a_l, a_r, b, c, d] = LEVELS;
        // `d` on in the middle (both sides 0 → 1), `c` off (left 1 → 0), `a` 6 dB down.
        let changed = settings(vec![
            setting("d", 0.0, None, Some(true)),
            setting("c", 0.0, None, Some(false)),
            setting("a", -6.0, None, None),
        ]);
        let half = 10f32.powf(-6.0 / 20.0);
        let mut outputs = outputs();
        // Blocks of 100: the move starts and ends inside blocks.
        let (left, right) = play(&mut outputs, 100, 15, |block, outputs| {
            if block == 2 {
                outputs.set(&changed, true);
            }
        });
        let most_left = (d + c + a_l * (1.0 - half)) / RAMP as f32;
        let most_right = (d + a_r * (1.0 - half)) / RAMP as f32;
        smooth(&left, most_left);
        smooth(&right, most_right);
        let before = (a_l + b + c, a_r + b);
        let after = steady(&changed);
        assert_eq!((left[199], right[199]), before);
        // The first frame of the block it arrives in has moved one step.
        let first_step = (d - c - a_l * (1.0 - half)) / RAMP as f32;
        assert!((left[200] - before.0 - first_step).abs() < 1e-7);
        // The move ends on its frame, 20 ms after it began, and stays there.
        assert_ne!((left[200 + RAMP - 2], right[200 + RAMP - 2]), after);
        for at in 200 + RAMP - 1..left.len() {
            assert_eq!((left[at], right[at]), after, "{at}");
        }
    }

    #[test]
    fn a_set_switched_off_and_on_again_never_jumps() {
        let c = LEVELS[3];
        let off = settings(vec![setting("c", 0.0, None, Some(false))]);
        let on = settings(Vec::new());
        let mut outputs = outputs();
        let (left, right) = play(&mut outputs, 64, 50, |block, outputs| match block {
            1 => outputs.set(&off, true),
            // Half way down.
            8 => outputs.set(&on, true),
            // Off, and on again one block later.
            25 => outputs.set(&off, true),
            26 => outputs.set(&on, true),
            _ => {}
        });
        smooth(&left, c / RAMP as f32);
        smooth(&right, 0.0);
        let [a_l, _, b, _, _] = LEVELS;
        assert!(left[8 * 64 - 1] < a_l + b + c * 0.6, "{}", left[8 * 64 - 1]);
        assert_eq!(left.last().copied(), Some(a_l + b + c));
        // Back at the defaults: fold, bit for bit.
        let out = awkward(5, 16);
        let (mut want_left, mut want_right) = (vec![0.0; 16], vec![0.0; 16]);
        fold(outputs.heard(), &out, 16, &mut want_left, &mut want_right);
        let got = mixed(&mut outputs, &out, 16);
        assert_eq!(
            (bits(&got.0), bits(&got.1)),
            (bits(&want_left), bits(&want_right))
        );
    }

    #[test]
    fn settings_of_a_load_hold_from_the_first_block() {
        let changed = settings(vec![
            setting("b", -12.0, Some(0.25), None),
            setting("d", 3.0, Some(-0.75), Some(true)),
        ]);
        let mut outputs = outputs();
        outputs.set(&changed, false);
        let (left, right) = mixed(&mut outputs, &constant(8), 8);
        let want = steady(&changed);
        assert!(left.iter().zip(&right).all(|(&l, &r)| (l, r) == want));
        assert_ne!(want, steady(&settings(Vec::new())));
    }

    #[test]
    fn a_reset_ends_every_move_at_its_target() {
        let changed = settings(vec![setting("a", -20.0, Some(1.0), None)]);
        let mut outputs = outputs();
        outputs.set(&changed, true);
        mixed(&mut outputs, &constant(10), 10);
        outputs.snap();
        let (left, right) = mixed(&mut outputs, &constant(10), 10);
        assert_eq!((left[0], right[0]), steady(&changed));
        // The settings stay.
        outputs.snap();
        let (left, _) = mixed(&mut outputs, &constant(10), 10);
        assert_eq!(left[9], steady(&changed).0);
    }

    #[test]
    fn a_missed_block_moves_the_gains_on() {
        let changed = settings(vec![setting("c", 0.0, None, Some(false))]);
        let mut outputs = outputs();
        outputs.set(&changed, true);
        outputs.missed(RAMP - 1);
        let (left, right) = mixed(&mut outputs, &constant(2), 2);
        assert_eq!((left[0], right[0]), steady(&changed));
    }

    #[test]
    fn what_is_not_a_number_does_not_reach_the_pair() {
        let frames = 4;
        let mut out = constant(frames);
        out[2 * frames + 1] = f32::NAN;
        out[3 * frames + 2] = f32::INFINITY;
        // Of a set nobody hears.
        out[4 * frames + 3] = f32::NAN;
        let quieter = settings(vec![setting("c", -3.0, None, None)]);
        let mut outputs = outputs();
        outputs.set(&quieter, false);
        let (left, right) = mixed(&mut outputs, &out, frames);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        assert_eq!((left[1], right[1]), (0.0, 0.0));
        assert_eq!((left[2], right[2]), (0.0, LEVELS[1] + LEVELS[2]));
        assert_eq!((left[3], right[3]), steady(&quieter));
    }

    #[test]
    fn a_capture_keeps_each_set_alone_before_its_settings() {
        let [a_l, a_r, b, c, d] = LEVELS;
        let mut outputs = outputs();
        outputs.set(
            &settings(vec![
                setting("a", -12.0, Some(0.5), Some(false)),
                setting("c", 6.0, Some(1.0), None),
            ]),
            false,
        );
        let ids = outputs.ids();
        assert!(
            outputs
                .arm(Capture::new(ids.clone(), 250).unwrap())
                .is_none()
        );
        play(&mut outputs, 100, 2, |_, _| {});
        outputs.missed(30);
        play(&mut outputs, 100, 2, |_, _| {});
        let capture = outputs.take_capture().unwrap();
        assert!(outputs.take_capture().is_none());
        let sets = capture.sounds();
        let got: Vec<&str> = sets.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(got, ids);
        let expect = [(a_l, a_r), (b, b), (c, 0.0), (d, d)];
        for ((id, sound), alone) in sets.iter().zip(expect) {
            let frames: Vec<(f32, f32)> = sound.chunks(2).map(|p| (p[0], p[1])).collect();
            // Full after 250: two blocks, 30 missed, 20 of the next block.
            assert_eq!(frames.len(), 250, "{id}");
            assert!(frames[..200].iter().all(|&f| f == alone), "{id}");
            assert!(frames[200..230].iter().all(|&f| f == (0.0, 0.0)), "{id}");
            assert!(frames[230..].iter().all(|&f| f == alone), "{id}");
        }
        // A second capture starts anew and hands back the first.
        outputs.arm(Capture::new(ids.clone(), 10).unwrap());
        let first = outputs.arm(Capture::new(ids.clone(), 10).unwrap()).unwrap();
        assert_eq!(first.reply().0[0].frames, 0);
        play(&mut outputs, 4, 1, |_, _| {});
        let second = outputs.take_capture().unwrap();
        assert!(second.sounds().iter().all(|(_, x)| x.len() == 8));
    }

    #[test]
    fn a_set_the_format_could_not_measure_is_left_out() {
        let [_, _, b, _, d] = LEVELS;
        let mut outputs = outputs();
        outputs.not_measured(1);
        outputs.arm(Capture::new(outputs.ids(), 8).unwrap());
        play(&mut outputs, 4, 1, |_, _| {});
        outputs.not_measured(2);
        outputs.missed(4);
        let sounds = outputs.take_capture().unwrap().sounds();
        let ids: Vec<&str> = sounds.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "d"]);
        assert_eq!(&sounds[1].1[..2], [b, b]);
        assert_eq!(&sounds[2].1[..8], [d; 8]);
        assert_eq!(&sounds[2].1[8..], [0.0; 8]);
        let (sets, blob) = Capture::new(outputs.ids(), 8).unwrap().reply();
        assert_eq!((sets.len(), blob.len()), (4, 0));
    }

    #[test]
    fn a_capture_too_long_for_one_message_is_refused() {
        let ids: Vec<String> = (0..4).map(|n| n.to_string()).collect();
        let most = MAX_BLOB_LEN as u64 / 32;
        assert!(Capture::new(ids.clone(), most + 1).is_err());
        let message = Capture::new(ids.clone(), u64::MAX).err().unwrap();
        assert!(message.contains(&most.to_string()), "{message}");
        assert!(Capture::new(Vec::new(), u64::MAX).is_ok());
        assert!(Capture::new(ids, 0).is_ok());
    }

    #[test]
    fn the_audio_thread_allocates_nothing_here() {
        let mut outputs = outputs();
        let out = constant(256);
        outputs.arm(Capture::new(outputs.ids(), 1000).unwrap());
        let changed = settings(vec![setting("c", -6.0, Some(0.5), Some(true))]);
        let (mut left, mut right) = (vec![0.0; 256], vec![0.0; 256]);
        let before = rtcheck::count();
        for block in 0..20 {
            if block == 3 {
                outputs.set(&changed, true);
            }
            let _audio = rtcheck::enter();
            if block == 5 {
                outputs.missed(256);
            } else {
                outputs.mix(&out, 256, &mut left, &mut right);
            }
            if block == 10 {
                outputs.snap();
            }
        }
        assert_eq!(rtcheck::count(), before);
    }
}
