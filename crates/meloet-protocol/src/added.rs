//! 「あとから入れたもの」: SF2 files and plugins, as the shelf, the tracks and the insert slots
//! show them, and what is said when one of them is not found.

use serde::{Deserialize, Serialize};

use crate::{EffectKind, InstrumentKind, PluginFormat};

/// A sound a track can have, and so a row of the shelf's instruments: one that comes with
/// meloet, a preset of an SF2 file the person added, or a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SoundRef {
    Builtin {
        instrument: InstrumentKind,
    },
    /// `file` is `Sf2FileView::file`; `bank` and `program` as in `Sf2PresetView`.
    Sf2 {
        file: String,
        bank: u16,
        program: u8,
    },
    /// `key` is the exact key of one format (`PluginRowView::key`, `PluginFormatView::key`).
    Plugin {
        key: String,
    },
}

/// An effect an insert slot can hold, and so a row of the shelf's effects.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EffectRef {
    Builtin { effect: EffectKind },
    Plugin { key: String },
}

/// The headings the presets of an SF2 file go under, in the order of the shelf: General
/// MIDI's sixteen families and the drum kits (spec 3, SF2). The names shown are the screen's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum GmFamily {
    Piano,
    Mallet,
    Organ,
    Guitar,
    Bass,
    Strings,
    Ensemble,
    Brass,
    Reed,
    Pipe,
    SynthLead,
    SynthPad,
    SynthEffects,
    Ethnic,
    Percussive,
    SoundEffects,
    DrumKit,
}

impl GmFamily {
    pub const ALL: [GmFamily; 17] = [
        GmFamily::Piano,
        GmFamily::Mallet,
        GmFamily::Organ,
        GmFamily::Guitar,
        GmFamily::Bass,
        GmFamily::Strings,
        GmFamily::Ensemble,
        GmFamily::Brass,
        GmFamily::Reed,
        GmFamily::Pipe,
        GmFamily::SynthLead,
        GmFamily::SynthPad,
        GmFamily::SynthEffects,
        GmFamily::Ethnic,
        GmFamily::Percussive,
        GmFamily::SoundEffects,
        GmFamily::DrumKit,
    ];
}

/// 音源 or エフェクト, from what the plugin says about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PluginRole {
    Instrument,
    Effect,
}

/// The headings of the shelf's plugin instruments, in its order (spec 3, 14-A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PluginInstrumentCategory {
    Synth,
    Sampler,
    Drum,
    /// 「ピアノ ・ 鍵盤」.
    Keys,
    Other,
}

impl PluginInstrumentCategory {
    pub const ALL: [PluginInstrumentCategory; 5] = [
        PluginInstrumentCategory::Synth,
        PluginInstrumentCategory::Sampler,
        PluginInstrumentCategory::Drum,
        PluginInstrumentCategory::Keys,
        PluginInstrumentCategory::Other,
    ];
}

/// The headings of the shelf's plugin effects, in its order (spec 3, 14-A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PluginEffectCategory {
    Eq,
    Dynamics,
    Reverb,
    Delay,
    Modulation,
    Distortion,
    Filter,
    Pitch,
    Spatial,
    Analysis,
    Other,
}

impl PluginEffectCategory {
    pub const ALL: [PluginEffectCategory; 11] = [
        PluginEffectCategory::Eq,
        PluginEffectCategory::Dynamics,
        PluginEffectCategory::Reverb,
        PluginEffectCategory::Delay,
        PluginEffectCategory::Modulation,
        PluginEffectCategory::Distortion,
        PluginEffectCategory::Filter,
        PluginEffectCategory::Pitch,
        PluginEffectCategory::Spatial,
        PluginEffectCategory::Analysis,
        PluginEffectCategory::Other,
    ];
}

/// Where the shelf puts a plugin: `{"role": "effect", "category": "reverb"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "role", content = "category", rename_all = "camelCase")]
pub enum PluginKindView {
    Instrument(PluginInstrumentCategory),
    Effect(PluginEffectCategory),
}

/// 「あとから入れたもの」 as the shelf lists it: every SF2 file and every plugin there is,
/// whatever the song uses. A section of its own, as it is long (hundreds of rows) and changes
/// only when a file is added or a scan finds something; what is selected, starred or missing
/// is in `ShelfView`.
///
/// The screen makes the headings from it, folds them, and searches it: an SF2 file's
/// heading with its families under it, then one heading for each category that has a
/// plugin of the list the shelf shows (instruments, or effects). Every heading starts
/// folded, but for `opened_sf2`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    /// In the order of their names.
    pub sf2: Vec<Sf2FileView>,
    /// Instruments and effects, in the order of their names (`name` without regard to
    /// case, then `vendor`). A plugin whose scan stopped is not here.
    pub plugins: Vec<PluginRowView>,
    /// The file added last since meloet started: its heading is unfolded and the other
    /// files' are folded (spec 3 「たたみかた」). `None`: none was added yet.
    pub opened_sf2: Option<String>,
    /// The sound pack that comes with meloet is not found (spec 3, 85): the shelf says so
    /// above its instruments, and `ShelfView::available` holds only what needs no pack.
    pub pack_missing: bool,
}

/// An SF2 file that was added, by the name of the file ("FluidR3_GM.sf2"), which is also
/// what a song names it by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Sf2FileView {
    pub file: String,
    /// In the order of the shelf: by family (`GmFamily::ALL`), inside a family by `program`,
    /// then by `bank`. The count on the file's heading is its length.
    pub presets: Vec<Sf2PresetView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Sf2PresetView {
    /// 128 for a drum kit. Shown ("・ バンク 8") when it is not 0.
    pub bank: u16,
    /// From 0; the row shows it from 1.
    pub program: u8,
    /// The preset's own name, as the file has it (not translated).
    pub name: String,
    pub family: GmFamily,
}

/// A row of the shelf's plugins: one plugin in every format it was found in (LV2, VST3),
/// merged by name and vendor (docs/architecture.md 20-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PluginRowView {
    pub name: String,
    pub vendor: String,
    pub kind: PluginKindView,
    /// The format the row uses, and what commands name the row by. Of `formats` it is the
    /// one the person chose (`Command::UsePluginFormat`); else the one the song already
    /// uses, else the first that opens a window, else the first of VST3, LV2.
    pub key: String,
    /// Every format the row comes in, the one used among them, in the order VST3, LV2. With
    /// more than one the row shows "＋1" after the mark of the used one.
    pub formats: Vec<PluginFormatView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PluginFormatView {
    pub format: PluginFormat,
    pub key: String,
}

/// A sound as a track has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SoundView {
    Builtin {
        instrument: InstrumentKind,
    },
    /// As the song has it, also when the file is not found.
    Sf2 {
        file: String,
        bank: u16,
        program: u8,
        name: String,
        family: GmFamily,
    },
    Plugin(PluginView),
}

/// A plugin where it sits in the song: as the instrument of a track, or in an insert slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PluginView {
    pub key: String,
    /// As the song has it.
    pub name: String,
    /// `None`: a format this meloet does not read.
    pub format: Option<PluginFormat>,
    pub state: PluginState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PluginState {
    /// It sounds.
    Ready,
    /// It is being read, and is silent until then.
    Loading,
    /// Its process answers no more (`StatusMessage::PluginStopped`): the red frame and 「止」.
    Stopped,
    /// This PC does not have it, or cannot read its format (`MissingView::Plugin`): an
    /// instrument sounds as the sine wave (`TrackView::stand_in`), an effect is passed by.
    Missing,
}

/// Why a track sounds as the sine wave in place of its own sound: its header and the mixer
/// say 「サイン波で代わり」 where the name of the sound would be (spec 3, 85).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum StandIn {
    /// The samples of a sound that comes with meloet are not there, or did not read.
    PackMissing,
    /// The SF2 file is not found, or it has no such preset.
    Sf2Missing,
    PluginMissing,
    /// The plugin is there and did not load (spec 3, 89-12).
    PluginUnreadable,
}

/// What sits in an insert slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InsertView {
    /// Stays with what sits in the slot when the slots of its strip are put in another
    /// order, and is never given to another insert: what replaces it has another id.
    /// `AutoTarget::Insert` names the insert by it.
    pub id: u32,
    pub effect: InsertEffectView,
    /// 「とばす」 (spec 3, 13-A): the mark at the left of the slot is empty and the name
    /// gray.7. Saved with the song.
    pub bypassed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InsertEffectView {
    Builtin { effect: EffectKind },
    Plugin(PluginView),
}

/// What the song uses and this PC does not have: a notice in the shelf, with its buttons
/// (spec 2 「見つからない物の知らせ」, spec 6). The sound pack is not one of these
/// (`LibraryView::pack_missing`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MissingView {
    /// "曲で使っている FluidR3_GM.sf2 が見つかりません。2 本のトラックはサイン波で鳴らして
    /// います。" with 「SF2 を足す…」 (`Command::AddSf2`) and 「曲から外す」
    /// (`Command::RemoveMissing`).
    Sf2 {
        file: String,
        /// How many tracks use a preset of it.
        tracks: u32,
    },
    /// "前はここにありました: （場所）。曲の中の設定は残しています。" under a row for the
    /// plugin, with 「場所をさがす」 (`Command::AddPluginPlace`), 「もう一度スキャン」
    /// (`Command::RescanPlugins`) and 「曲から外す」.
    Plugin {
        key: String,
        name: String,
        vendor: String,
        /// `None`: a format this meloet does not read.
        format: Option<PluginFormat>,
        /// Which of the shelf's two lists its row is in.
        role: PluginRole,
        /// Where its bundle was, the home folder written as "~". `None`: the song does not
        /// say.
        shown_path: Option<String>,
    },
}

/// What `Command::RemoveMissing` names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MissingRef {
    Sf2 {
        file: String,
    },
    Plugin {
        key: String,
    },
    /// An audio file, by its number in the song's audio table (`MissingAudioView::audio`):
    /// every clip that plays it is deleted. One step.
    Audio {
        audio: u32,
    },
}

/// A sample of the shelf (`Command::PlaySampleOf`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SoundSampleView {
    pub sound: SoundRef,
    /// `None`: the sound is still being read, and the place of the time says "読みこみ中…"
    /// (spec 3 Y14). Then the screen counts "0:02 / 0:04" from these on its own clock
    /// (`HelloReply::mono_us`), with no message per second.
    pub playing: Option<SamplePlaying>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SamplePlaying {
    /// When its first sound reached the ear, CLOCK_MONOTONIC µs.
    pub started_us: u64,
    pub length_us: u64,
}

/// A sound by the name the status line gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SoundName {
    /// The screen has the name.
    Builtin { instrument: InstrumentKind },
    /// "Clean Guitar ・ FluidR3_GM.sf2" (mock F5-SF2Shelf).
    Sf2 { name: String, file: String },
    /// "Surge XT".
    Plugin { name: String },
}

/// Files are held over the window (spec 2 「ファイルを持ってきている間の知らせ」): the box in
/// the middle of the window. Rust hears of the files from the PC, not from the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FileDropView {
    /// The name of the file, without its folder; of the first one when there are more.
    pub name: String,
    /// False: "SF2 のファイルではないので、足せません。" True when one of the files is an SF2
    /// file or an audio file.
    pub addable: bool,
    /// How many files are held, and how many of them are SF2 files: with more than one the
    /// box says "3 つのファイル ・ SF2 は 2 つ" in place of the name (spec 3, 89-10).
    pub files: u32,
    pub sf2: u32,
    /// How many of them are audio files (by their ending: wav, mp3, flac, ogg). Above 0 the
    /// pointer's place comes as `Event::FileDrag`, the screen draws the gray.7 frame where
    /// they would go, and `name` is the first audio file ("ドラム キック.wav ほか 7"). 2 or
    /// more: the box is the two halves of 30-C. With `sf2` above 0 as well the box says
    /// "SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ" (spec 3 「読めるファイル」).
    pub audio: u32,
}

/// One name of the selected notes and how many of them have it ("スネア ×2").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyCount {
    pub key: u8,
    pub count: u32,
}

impl InsertView {
    /// An effect that comes with meloet, not passed by.
    pub fn builtin(id: u32, effect: EffectKind) -> InsertView {
        InsertView {
            id,
            effect: InsertEffectView::Builtin { effect },
            bypassed: false,
        }
    }
}
