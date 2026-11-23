//! The four knobs of a built-in sound (protocol 33, round 12 E3): rows of the parameters panel
//! in the shape of a built-in effect's, when the selected instrument is a sound of meloet.

use serde::{Deserialize, Serialize};

/// A knob, in the order of the rows. Its place in `ALL` is its `param`: in
/// `Command::SelectParam`, and in `AutoTarget::Instrument` of a lane of a built-in sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SoundKnob {
    /// 「明るさ」.
    Brightness,
    /// 「立ち上がり」.
    Attack,
    /// 「のび」.
    Release,
    /// 「ゆれ」.
    Vibrato,
}

impl SoundKnob {
    pub const ALL: [SoundKnob; 4] = [
        SoundKnob::Brightness,
        SoundKnob::Attack,
        SoundKnob::Release,
        SoundKnob::Vibrato,
    ];
}

/// Where each knob starts, in percent, in the order of `SoundKnob::ALL` (what Home and a
/// double click on the knob's mark put back).
pub const SOUND_KNOB_DEFAULTS: [f32; 4] = [50.0, 50.0, 50.0, 0.0];

/// The knobs of the selected built-in sound (`SelectedSound::knobs`). A preview
/// (`commit: false`) is not in here, as for `SelectedInsert::values`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SoundKnobsView {
    /// Percent, 0.0..=100.0, in the order of `SoundKnob::ALL`, as the song has them.
    pub values: [f32; 4],
    /// Whether each knob does something to this sound: a drum kit has only 「明るさ」 and
    /// 「のび」; the other rows are shown disabled.
    pub usable: [bool; 4],
    /// The chosen row (`Command::SelectParam`), by its place in `SoundKnob::ALL`.
    pub row: Option<u32>,
}
