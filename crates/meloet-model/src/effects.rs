//! The built-in effects and their parameter tables (spec chapter 3 for reverb and delay,
//! docs/architecture.md 19-3 for the other four, the round 12 spec M6 for gate, de-esser and
//! saturation).
//!
//! A parameter is addressed by its position in its effect's table; the same number goes to the
//! effect on the audio thread. A value is an `f32` in the parameter's own unit. The names here
//! are the ones written to the song file; what the screen shows is the screen's business.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectKind {
    Eq4,
    Compressor,
    Reverb,
    Delay,
    Chorus,
    Limiter,
    Gate,
    DeEsser,
    Saturation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Hz,
    Db,
    Ms,
    Seconds,
    Percent,
    /// Width of an EQ peak in octaves.
    Octaves,
    /// The compressor's "n : 1".
    Ratio,
    /// Steps and switches.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamShape {
    /// Any value from `min` to `max`.
    Continuous,
    /// Whole numbers from 0; one name per step, as written to the song file.
    Steps(&'static [&'static str]),
    /// 0 is off, 1 is on.
    Switch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    /// camelCase, unique within the effect.
    pub name: &'static str,
    /// The heading the parameter sits under (the EQ's four bands).
    pub group: Option<&'static str>,
    pub shape: ParamShape,
    pub unit: Unit,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

impl ParamSpec {
    /// Whether the bar of the parameters panel moves over this parameter by its logarithm:
    /// frequencies always, times that span 50 times or more.
    fn log_scale(&self) -> bool {
        self.min > 0.0
            && (self.unit == Unit::Hz
                || matches!(self.unit, Unit::Ms | Unit::Seconds) && self.max / self.min >= 50.0)
    }

    /// Where a value is on the bar of the parameters panel, 0..=1; a step n is
    /// n / (steps − 1), a switch 0 or 1. What an automation point of the parameter holds
    /// (docs/architecture.md 21-5); the screen's `fractionOf` gives the same.
    pub fn position(&self, value: f32) -> f32 {
        let v = f64::from(if value.is_nan() { self.default } else { value })
            .clamp(f64::from(self.min), f64::from(self.max));
        let (min, max) = (f64::from(self.min), f64::from(self.max));
        let f = if self.log_scale() {
            (v / min).ln() / (max / min).ln()
        } else {
            (v - min) / (max - min)
        };
        if f.is_finite() {
            f.clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }

    /// The value at a place on the bar, the other way from [`ParamSpec::position`]. Steps
    /// and switches go to the nearest whole number; a continuous value is not rounded to
    /// what the panel shows (the screen's `valueAt` is, for its text), so that a line of
    /// automation moves without stairs.
    pub fn value_at(&self, position: f32) -> f32 {
        let f = f64::from(if position.is_nan() { 0.0 } else { position }).clamp(0.0, 1.0);
        let (min, max) = (f64::from(self.min), f64::from(self.max));
        let v = if self.log_scale() {
            min * (max / min).powf(f)
        } else {
            min + f * (max - min)
        };
        self.clamp(v as f32)
    }

    /// Brings a value into the range: steps and switches to the nearest whole number, a value
    /// that is not a number to the default.
    pub fn clamp(&self, value: f32) -> f32 {
        if value.is_nan() {
            return self.default;
        }
        let v = value.clamp(self.min, self.max);
        match self.shape {
            ParamShape::Continuous => v,
            ParamShape::Steps(_) | ParamShape::Switch => v.round(),
        }
    }

    /// Reads a value as a person types it into the parameters panel, with or without its
    /// unit ("4" and "4 : 1", "−24 dB", "１２０"): the number the text starts with, in the
    /// parameter's own unit. A unit that says another scale is followed ("500 ms" for
    /// seconds, "1.5 s" for milliseconds, "2 kHz"). `None` when the text starts with no
    /// number. The value is not brought into the range.
    pub fn read(&self, text: &str) -> Option<f32> {
        let text: String = (text.trim().chars())
            .map(|c| match c {
                '０'..='９' => char::from_digit(c as u32 - '０' as u32, 10).unwrap_or(c),
                'Ａ'..='Ｚ' | 'ａ'..='ｚ' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
                '−' | '－' => '-',
                '＋' => '+',
                '．' => '.',
                c => c,
            })
            .collect();
        let (sign, rest) = match text.strip_prefix(['-', '+']) {
            Some(rest) if text.starts_with('-') => (-1.0, rest.trim_start()),
            Some(rest) => (1.0, rest.trim_start()),
            None => (1.0, &*text),
        };
        let whole = rest.chars().take_while(char::is_ascii_digit).count();
        let part = match rest[whole..].strip_prefix('.') {
            Some(after) => 1 + after.chars().take_while(char::is_ascii_digit).count(),
            None => 0,
        };
        let end = whole + part;
        if !rest[..end].contains(|c: char| c.is_ascii_digit()) {
            return None;
        }
        let number: f32 = rest[..end].parse().ok()?;
        let unit = rest[end..].trim_start().to_lowercase();
        let scale = match self.unit {
            Unit::Seconds if unit.starts_with("ms") => 0.001,
            Unit::Ms if unit.starts_with('s') => 1000.0,
            Unit::Hz if unit.starts_with('k') => 1000.0,
            _ => 1.0,
        };
        Some(sign * number * scale)
    }
}

const fn cont(name: &'static str, unit: Unit, min: f32, max: f32, default: f32) -> ParamSpec {
    ParamSpec {
        name,
        group: None,
        shape: ParamShape::Continuous,
        unit,
        min,
        max,
        default,
    }
}

const fn band(
    group: &'static str,
    name: &'static str,
    unit: Unit,
    min: f32,
    max: f32,
    default: f32,
) -> ParamSpec {
    ParamSpec {
        name,
        group: Some(group),
        shape: ParamShape::Continuous,
        unit,
        min,
        max,
        default,
    }
}

const fn switch(name: &'static str, on: bool) -> ParamSpec {
    ParamSpec {
        name,
        group: None,
        shape: ParamShape::Switch,
        unit: Unit::None,
        min: 0.0,
        max: 1.0,
        default: if on { 1.0 } else { 0.0 },
    }
}

/// Positions in the EQ's table.
pub mod eq4 {
    pub const LOW_FREQ: u32 = 0;
    pub const LOW_GAIN: u32 = 1;
    pub const LOW_MID_FREQ: u32 = 2;
    pub const LOW_MID_GAIN: u32 = 3;
    pub const LOW_MID_WIDTH: u32 = 4;
    pub const HIGH_MID_FREQ: u32 = 5;
    pub const HIGH_MID_GAIN: u32 = 6;
    pub const HIGH_MID_WIDTH: u32 = 7;
    pub const HIGH_FREQ: u32 = 8;
    pub const HIGH_GAIN: u32 = 9;
}

/// Positions in the compressor's table.
pub mod compressor {
    pub const THRESHOLD: u32 = 0;
    pub const RATIO: u32 = 1;
    pub const ATTACK: u32 = 2;
    pub const RELEASE: u32 = 3;
    pub const MAKEUP: u32 = 4;
}

/// Positions in the reverb's table.
pub mod reverb {
    pub const DECAY: u32 = 0;
    pub const SIZE: u32 = 1;
    pub const PRE_DELAY: u32 = 2;
    pub const HIGH_CUT: u32 = 3;
    pub const LOW_CUT: u32 = 4;
    pub const WIDTH: u32 = 5;
    pub const MIX: u32 = 6;
}

/// Positions in the delay's table.
pub mod delay {
    /// Used while `SYNC` is on: a position in [`super::DELAY_INTERVALS`].
    pub const INTERVAL_STEP: u32 = 0;
    /// Used while `SYNC` is off.
    pub const INTERVAL_MS: u32 = 1;
    pub const SYNC: u32 = 2;
    pub const FEEDBACK: u32 = 3;
    pub const HIGH_CUT: u32 = 4;
    pub const LOW_CUT: u32 = 5;
    pub const PING_PONG: u32 = 6;
    pub const MIX: u32 = 7;
}

/// Positions in the chorus's table.
pub mod chorus {
    pub const RATE: u32 = 0;
    pub const DEPTH: u32 = 1;
    pub const WIDTH: u32 = 2;
    pub const MIX: u32 = 3;
}

/// Positions in the limiter's table.
pub mod limiter {
    pub const CEILING: u32 = 0;
    pub const GAIN: u32 = 1;
    pub const RELEASE: u32 = 2;
}

/// Positions in the gate's table.
pub mod gate {
    /// 「しきい値」.
    pub const THRESHOLD: u32 = 0;
    /// 「下げる量」: how far below the sound is brought (−60 dB at most).
    pub const RANGE: u32 = 1;
    /// 「戻る速さ」.
    pub const RELEASE: u32 = 2;
}

/// Positions in the de-esser's table.
pub mod de_esser {
    /// 「周波数」: the band above it is watched and lowered.
    pub const FREQUENCY: u32 = 0;
    /// 「しきい値」.
    pub const THRESHOLD: u32 = 1;
    /// 「下げる量」: the most the band is lowered.
    pub const REDUCTION: u32 = 2;
}

/// Positions in the saturation's table.
pub mod saturation {
    /// 「ひずみ」.
    pub const DRIVE: u32 = 0;
    /// 「明るさ」: the low-pass after the curve.
    pub const TONE: u32 = 1;
    /// 「まぜる量」.
    pub const MIX: u32 = 2;
    /// 「出る大きさ」.
    pub const OUTPUT: u32 = 3;
}

/// One step of the delay's interval: its name in the song file and its length in quarter notes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DelayInterval {
    pub name: &'static str,
    pub quarters: f32,
}

/// 1/4, dotted 1/8, 1/8, 1/8 triplet, 1/16.
pub const DELAY_INTERVALS: [DelayInterval; 5] = [
    DelayInterval {
        name: "quarter",
        quarters: 1.0,
    },
    DelayInterval {
        name: "eighthDotted",
        quarters: 0.75,
    },
    DelayInterval {
        name: "eighth",
        quarters: 0.5,
    },
    DelayInterval {
        name: "eighthTriplet",
        quarters: 1.0 / 3.0,
    },
    DelayInterval {
        name: "sixteenth",
        quarters: 0.25,
    },
];

const DELAY_INTERVAL_NAMES: [&str; 5] = [
    DELAY_INTERVALS[0].name,
    DELAY_INTERVALS[1].name,
    DELAY_INTERVALS[2].name,
    DELAY_INTERVALS[3].name,
    DELAY_INTERVALS[4].name,
];

const EQ4: [ParamSpec; 10] = [
    band("low", "lowFreq", Unit::Hz, 20.0, 1000.0, 100.0),
    band("low", "lowGain", Unit::Db, -18.0, 18.0, 0.0),
    band("lowMid", "lowMidFreq", Unit::Hz, 100.0, 5000.0, 500.0),
    band("lowMid", "lowMidGain", Unit::Db, -18.0, 18.0, 0.0),
    band("lowMid", "lowMidWidth", Unit::Octaves, 0.2, 4.0, 1.0),
    band("highMid", "highMidFreq", Unit::Hz, 500.0, 16000.0, 3000.0),
    band("highMid", "highMidGain", Unit::Db, -18.0, 18.0, 0.0),
    band("highMid", "highMidWidth", Unit::Octaves, 0.2, 4.0, 1.0),
    band("high", "highFreq", Unit::Hz, 1000.0, 20000.0, 8000.0),
    band("high", "highGain", Unit::Db, -18.0, 18.0, 0.0),
];

const COMPRESSOR: [ParamSpec; 5] = [
    cont("threshold", Unit::Db, -60.0, 0.0, -18.0),
    cont("ratio", Unit::Ratio, 1.0, 20.0, 4.0),
    cont("attack", Unit::Ms, 0.1, 100.0, 10.0),
    cont("release", Unit::Ms, 10.0, 1000.0, 100.0),
    cont("makeup", Unit::Db, 0.0, 24.0, 0.0),
];

const REVERB: [ParamSpec; 7] = [
    cont("decay", Unit::Seconds, 0.2, 10.0, 2.4),
    cont("size", Unit::Percent, 0.0, 100.0, 60.0),
    cont("preDelay", Unit::Ms, 0.0, 200.0, 20.0),
    cont("highCut", Unit::Hz, 1000.0, 20000.0, 6000.0),
    cont("lowCut", Unit::Hz, 20.0, 1000.0, 120.0),
    cont("width", Unit::Percent, 0.0, 100.0, 100.0),
    cont("mix", Unit::Percent, 0.0, 100.0, 100.0),
];

const DELAY: [ParamSpec; 8] = [
    ParamSpec {
        name: "intervalStep",
        group: None,
        shape: ParamShape::Steps(&DELAY_INTERVAL_NAMES),
        unit: Unit::None,
        min: 0.0,
        max: (DELAY_INTERVALS.len() - 1) as f32,
        default: 2.0,
    },
    cont("intervalMs", Unit::Ms, 1.0, 2000.0, 250.0),
    switch("sync", true),
    cont("feedback", Unit::Percent, 0.0, 95.0, 35.0),
    cont("highCut", Unit::Hz, 1000.0, 20000.0, 5000.0),
    cont("lowCut", Unit::Hz, 20.0, 1000.0, 200.0),
    switch("pingPong", false),
    cont("mix", Unit::Percent, 0.0, 100.0, 100.0),
];

const CHORUS: [ParamSpec; 4] = [
    cont("rate", Unit::Hz, 0.05, 5.0, 0.8),
    cont("depth", Unit::Percent, 0.0, 100.0, 50.0),
    cont("width", Unit::Percent, 0.0, 100.0, 100.0),
    cont("mix", Unit::Percent, 0.0, 100.0, 50.0),
];

const LIMITER: [ParamSpec; 3] = [
    cont("ceiling", Unit::Db, -12.0, 0.0, -0.3),
    cont("gain", Unit::Db, 0.0, 12.0, 0.0),
    cont("release", Unit::Ms, 1.0, 1000.0, 100.0),
];

const GATE: [ParamSpec; 3] = [
    cont("threshold", Unit::Db, -80.0, 0.0, -45.0),
    cont("range", Unit::Db, -60.0, 0.0, -60.0),
    cont("release", Unit::Ms, 5.0, 1000.0, 120.0),
];

const DE_ESSER: [ParamSpec; 3] = [
    cont("frequency", Unit::Hz, 3000.0, 10000.0, 6500.0),
    cont("threshold", Unit::Db, -60.0, 0.0, -30.0),
    cont("reduction", Unit::Db, 0.0, 18.0, 8.0),
];

const SATURATION: [ParamSpec; 4] = [
    cont("drive", Unit::Db, 0.0, 36.0, 6.0),
    cont("tone", Unit::Hz, 1000.0, 20000.0, 20000.0),
    cont("mix", Unit::Percent, 0.0, 100.0, 100.0),
    cont("output", Unit::Db, -24.0, 6.0, 0.0),
];

impl EffectKind {
    pub const ALL: [EffectKind; 9] = [
        EffectKind::Eq4,
        EffectKind::Compressor,
        EffectKind::Reverb,
        EffectKind::Delay,
        EffectKind::Chorus,
        EffectKind::Limiter,
        EffectKind::Gate,
        EffectKind::DeEsser,
        EffectKind::Saturation,
    ];

    /// The name written to the song file.
    pub fn name(self) -> &'static str {
        match self {
            EffectKind::Eq4 => "eq4",
            EffectKind::Compressor => "compressor",
            EffectKind::Reverb => "reverb",
            EffectKind::Delay => "delay",
            EffectKind::Chorus => "chorus",
            EffectKind::Limiter => "limiter",
            EffectKind::Gate => "gate",
            EffectKind::DeEsser => "deEsser",
            EffectKind::Saturation => "saturation",
        }
    }

    pub fn from_name(name: &str) -> Option<EffectKind> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }

    /// The parameters in the order the screen lists them.
    pub fn params(self) -> &'static [ParamSpec] {
        match self {
            EffectKind::Eq4 => &EQ4,
            EffectKind::Compressor => &COMPRESSOR,
            EffectKind::Reverb => &REVERB,
            EffectKind::Delay => &DELAY,
            EffectKind::Chorus => &CHORUS,
            EffectKind::Limiter => &LIMITER,
            EffectKind::Gate => &GATE,
            EffectKind::DeEsser => &DE_ESSER,
            EffectKind::Saturation => &SATURATION,
        }
    }

    pub fn param(self, index: u32) -> Option<&'static ParamSpec> {
        self.params().get(index as usize)
    }

    pub fn param_index(self, name: &str) -> Option<u32> {
        self.params()
            .iter()
            .position(|p| p.name == name)
            .map(|i| i as u32)
    }

    /// Brings a value into its parameter's range; `None` if the effect has no such parameter.
    pub fn clamp(self, index: u32, value: f32) -> Option<f32> {
        self.param(index).map(|p| p.clamp(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_defaults_are_in_range() {
        for kind in EffectKind::ALL {
            assert_eq!(EffectKind::from_name(kind.name()), Some(kind));
            let params = kind.params();
            for (i, p) in params.iter().enumerate() {
                assert!(p.min < p.max, "{} {}", kind.name(), p.name);
                assert_eq!(p.clamp(p.default), p.default, "{} {}", kind.name(), p.name);
                assert_eq!(kind.param_index(p.name), Some(i as u32));
                if let ParamShape::Steps(names) = p.shape {
                    assert_eq!(p.min, 0.0);
                    assert_eq!(p.max, (names.len() - 1) as f32);
                }
            }
        }
        assert_eq!(EffectKind::from_name("phaser"), None);
    }

    #[test]
    fn positions_match_the_tables() {
        let at = |kind: EffectKind, i: u32| kind.param(i).unwrap().name;
        assert_eq!(at(EffectKind::Eq4, eq4::LOW_FREQ), "lowFreq");
        assert_eq!(at(EffectKind::Eq4, eq4::LOW_GAIN), "lowGain");
        assert_eq!(at(EffectKind::Eq4, eq4::LOW_MID_FREQ), "lowMidFreq");
        assert_eq!(at(EffectKind::Eq4, eq4::LOW_MID_GAIN), "lowMidGain");
        assert_eq!(at(EffectKind::Eq4, eq4::LOW_MID_WIDTH), "lowMidWidth");
        assert_eq!(at(EffectKind::Eq4, eq4::HIGH_MID_FREQ), "highMidFreq");
        assert_eq!(at(EffectKind::Eq4, eq4::HIGH_MID_GAIN), "highMidGain");
        assert_eq!(at(EffectKind::Eq4, eq4::HIGH_MID_WIDTH), "highMidWidth");
        assert_eq!(at(EffectKind::Eq4, eq4::HIGH_FREQ), "highFreq");
        assert_eq!(at(EffectKind::Eq4, eq4::HIGH_GAIN), "highGain");
        assert_eq!(
            at(EffectKind::Compressor, compressor::THRESHOLD),
            "threshold"
        );
        assert_eq!(at(EffectKind::Compressor, compressor::RATIO), "ratio");
        assert_eq!(at(EffectKind::Compressor, compressor::ATTACK), "attack");
        assert_eq!(at(EffectKind::Compressor, compressor::RELEASE), "release");
        assert_eq!(at(EffectKind::Compressor, compressor::MAKEUP), "makeup");
        assert_eq!(at(EffectKind::Reverb, reverb::DECAY), "decay");
        assert_eq!(at(EffectKind::Reverb, reverb::SIZE), "size");
        assert_eq!(at(EffectKind::Reverb, reverb::PRE_DELAY), "preDelay");
        assert_eq!(at(EffectKind::Reverb, reverb::HIGH_CUT), "highCut");
        assert_eq!(at(EffectKind::Reverb, reverb::LOW_CUT), "lowCut");
        assert_eq!(at(EffectKind::Reverb, reverb::WIDTH), "width");
        assert_eq!(at(EffectKind::Reverb, reverb::MIX), "mix");
        assert_eq!(at(EffectKind::Delay, delay::INTERVAL_STEP), "intervalStep");
        assert_eq!(at(EffectKind::Delay, delay::INTERVAL_MS), "intervalMs");
        assert_eq!(at(EffectKind::Delay, delay::SYNC), "sync");
        assert_eq!(at(EffectKind::Delay, delay::FEEDBACK), "feedback");
        assert_eq!(at(EffectKind::Delay, delay::HIGH_CUT), "highCut");
        assert_eq!(at(EffectKind::Delay, delay::LOW_CUT), "lowCut");
        assert_eq!(at(EffectKind::Delay, delay::PING_PONG), "pingPong");
        assert_eq!(at(EffectKind::Delay, delay::MIX), "mix");
        assert_eq!(at(EffectKind::Chorus, chorus::RATE), "rate");
        assert_eq!(at(EffectKind::Chorus, chorus::DEPTH), "depth");
        assert_eq!(at(EffectKind::Chorus, chorus::WIDTH), "width");
        assert_eq!(at(EffectKind::Chorus, chorus::MIX), "mix");
        assert_eq!(at(EffectKind::Limiter, limiter::CEILING), "ceiling");
        assert_eq!(at(EffectKind::Limiter, limiter::GAIN), "gain");
        assert_eq!(at(EffectKind::Limiter, limiter::RELEASE), "release");
        assert_eq!(at(EffectKind::Gate, gate::THRESHOLD), "threshold");
        assert_eq!(at(EffectKind::Gate, gate::RANGE), "range");
        assert_eq!(at(EffectKind::Gate, gate::RELEASE), "release");
        assert_eq!(at(EffectKind::DeEsser, de_esser::FREQUENCY), "frequency");
        assert_eq!(at(EffectKind::DeEsser, de_esser::THRESHOLD), "threshold");
        assert_eq!(at(EffectKind::DeEsser, de_esser::REDUCTION), "reduction");
        assert_eq!(at(EffectKind::Saturation, saturation::DRIVE), "drive");
        assert_eq!(at(EffectKind::Saturation, saturation::TONE), "tone");
        assert_eq!(at(EffectKind::Saturation, saturation::MIX), "mix");
        assert_eq!(at(EffectKind::Saturation, saturation::OUTPUT), "output");
    }

    /// The table of the round 12 spec M6: names in the song file, ranges and defaults.
    #[test]
    fn the_three_of_round_12() {
        let row = |kind: EffectKind| {
            (kind.params().iter())
                .map(|p| (p.name, p.unit, p.min, p.max, p.default))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            [
                EffectKind::Gate,
                EffectKind::DeEsser,
                EffectKind::Saturation
            ]
            .map(EffectKind::name),
            ["gate", "deEsser", "saturation"]
        );
        assert_eq!(
            row(EffectKind::Gate),
            [
                ("threshold", Unit::Db, -80.0, 0.0, -45.0),
                ("range", Unit::Db, -60.0, 0.0, -60.0),
                ("release", Unit::Ms, 5.0, 1000.0, 120.0),
            ]
        );
        assert_eq!(
            row(EffectKind::DeEsser),
            [
                ("frequency", Unit::Hz, 3000.0, 10000.0, 6500.0),
                ("threshold", Unit::Db, -60.0, 0.0, -30.0),
                ("reduction", Unit::Db, 0.0, 18.0, 8.0),
            ]
        );
        assert_eq!(
            row(EffectKind::Saturation),
            [
                ("drive", Unit::Db, 0.0, 36.0, 6.0),
                ("tone", Unit::Hz, 1000.0, 20000.0, 20000.0),
                ("mix", Unit::Percent, 0.0, 100.0, 100.0),
                ("output", Unit::Db, -24.0, 6.0, 0.0),
            ]
        );
    }

    #[test]
    fn the_spec_defaults() {
        let d = |kind: EffectKind, i: u32| kind.param(i).unwrap().default;
        assert_eq!(d(EffectKind::Reverb, reverb::DECAY), 2.4);
        assert_eq!(d(EffectKind::Reverb, reverb::HIGH_CUT), 6000.0);
        assert_eq!(d(EffectKind::Delay, delay::FEEDBACK), 35.0);
        assert_eq!(d(EffectKind::Delay, delay::INTERVAL_MS), 250.0);
        assert_eq!(d(EffectKind::Limiter, limiter::CEILING), -0.3);
        let step = d(EffectKind::Delay, delay::INTERVAL_STEP) as usize;
        assert_eq!(DELAY_INTERVALS[step].name, "eighth");
        assert_eq!(
            EffectKind::Eq4.params()[2].group,
            Some("lowMid"),
            "the EQ's headings"
        );
    }

    #[test]
    fn a_typed_value_reads_with_or_without_its_unit() {
        let of = |kind: EffectKind, i: u32, text: &str| kind.param(i).unwrap().read(text);
        let comp = EffectKind::Compressor;
        assert_eq!(of(comp, compressor::RATIO, "4"), Some(4.0));
        assert_eq!(of(comp, compressor::RATIO, "4 : 1"), Some(4.0));
        assert_eq!(of(comp, compressor::RATIO, " 2.5:1 "), Some(2.5));
        assert_eq!(of(comp, compressor::RATIO, "25"), Some(25.0), "not clamped");
        assert_eq!(of(comp, compressor::THRESHOLD, "−24.0 dB"), Some(-24.0));
        assert_eq!(of(comp, compressor::THRESHOLD, "-24"), Some(-24.0));
        assert_eq!(of(comp, compressor::THRESHOLD, "－ ２４．５"), Some(-24.5));
        assert_eq!(of(comp, compressor::MAKEUP, "+3 dB"), Some(3.0));
        assert_eq!(of(comp, compressor::MAKEUP, ".5"), Some(0.5));
        assert_eq!(of(comp, compressor::MAKEUP, "5."), Some(5.0));
        assert_eq!(of(comp, compressor::ATTACK, "0.02 s"), Some(20.0));
        assert_eq!(of(comp, compressor::ATTACK, "20 ms"), Some(20.0));
        let reverb = EffectKind::Reverb;
        assert_eq!(of(reverb, reverb::DECAY, "1.8"), Some(1.8));
        assert_eq!(of(reverb, reverb::DECAY, "1.8 s"), Some(1.8));
        assert_eq!(of(reverb, reverb::DECAY, "500 ms"), Some(0.5));
        assert_eq!(of(reverb, reverb::HIGH_CUT, "6 kHz"), Some(6000.0));
        assert_eq!(of(reverb, reverb::HIGH_CUT, "6000 Hz"), Some(6000.0));
        assert_eq!(of(reverb, reverb::MIX, "30%"), Some(30.0));
        assert_eq!(of(reverb, reverb::DECAY, "５００ｍｓ"), Some(0.5));
        assert_eq!(of(reverb, reverb::DECAY, "５００　ＭＳ"), Some(0.5));
        assert_eq!(of(comp, compressor::ATTACK, "１．５ｓ"), Some(1500.0));
        assert_eq!(of(reverb, reverb::HIGH_CUT, "２ｋＨｚ"), Some(2000.0));
        assert_eq!(of(EffectKind::Delay, delay::SYNC, "1"), Some(1.0));
        for text in [
            "", " ", "dB", "abc", "-", "+ dB", ".", "- .", "x4", "NaN", "inf",
        ] {
            assert_eq!(of(comp, compressor::THRESHOLD, text), None, "{text:?}");
        }
    }

    #[test]
    fn clamp_brings_values_into_range() {
        let k = EffectKind::Delay;
        assert_eq!(k.clamp(delay::FEEDBACK, 120.0), Some(95.0));
        assert_eq!(k.clamp(delay::FEEDBACK, -3.0), Some(0.0));
        assert_eq!(k.clamp(delay::FEEDBACK, f32::NAN), Some(35.0));
        assert_eq!(k.clamp(delay::FEEDBACK, f32::INFINITY), Some(95.0));
        assert_eq!(k.clamp(delay::INTERVAL_STEP, 2.6), Some(3.0));
        assert_eq!(k.clamp(delay::INTERVAL_STEP, 9.0), Some(4.0));
        assert_eq!(k.clamp(delay::SYNC, 0.4), Some(0.0));
        assert_eq!(k.clamp(delay::SYNC, 7.0), Some(1.0));
        assert_eq!(k.clamp(99, 1.0), None);
    }
}
