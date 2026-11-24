//! The instruments a track can have, and the one table of the ones that come with meloet.

use std::sync::Arc;

use crate::Language;
use crate::mixer::{Gain, Pan};

/// What a track sounds with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentRef {
    Builtin(BuiltinInstrument),
    Plugin(Arc<PluginRef>),
    Sf2(Arc<Sf2Ref>),
}

impl InstrumentRef {
    /// What a track falls back to when its instrument is not known.
    pub const SINE: InstrumentRef = InstrumentRef::Builtin(BuiltinInstrument::Sine);

    /// The first track of a new song.
    pub const GRAND_PIANO: InstrumentRef = InstrumentRef::Builtin(BuiltinInstrument::GrandPiano);

    /// The group whose rules the track follows: the sends of a new track, the phrase of ▶, and
    /// for `Drums` the drum piano roll with no scale lock. An SF2 preset goes by the nearest
    /// group of its family (spec 3, SF2). A plugin counts as a synth; what it says about itself
    /// is known to the scan, not to the song.
    pub fn group(&self) -> InstrumentGroup {
        match self {
            InstrumentRef::Builtin(b) => b.group(),
            InstrumentRef::Plugin(_) => InstrumentGroup::Synth,
            InstrumentRef::Sf2(s) => s.family().group(),
        }
    }

    /// The name a new track gets, before a number tells it from another track: the short
    /// name of a built-in sound's group or of an SF2 preset's family, or the plugin's own
    /// name (spec 3 Y14).
    pub fn track_name(&self, language: Language) -> &str {
        match self {
            InstrumentRef::Builtin(b) => b.track_name(language),
            InstrumentRef::Plugin(p) => &p.name,
            InstrumentRef::Sf2(s) => s.family().track_name(language),
        }
    }
}

/// Fields of an entry of the song file that this meloet does not know, as text only the song
/// file reads. Carried along untouched, so that saving writes them back.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Kept(Option<Arc<str>>);

impl Kept {
    pub const NONE: Kept = Kept(None);

    pub fn new(text: &str) -> Kept {
        Kept((!text.is_empty()).then(|| text.into()))
    }

    pub fn text(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

/// A plugin as the song keeps it, whether or not this PC has it: its fields stay as they were
/// read (docs/architecture.md 20-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRef {
    /// The exact key of one format, `vst3:<cid>` or `lv2:<URI>` (20-5). Text, because a song
    /// may name a format this meloet does not read.
    pub key: Arc<str>,
    pub name: Arc<str>,
    pub vendor: Arc<str>,
    /// Where its bundle was when it was put in, to tell the person when it is not found.
    /// Empty when not known.
    pub path: Arc<str>,
    /// Everything the plugin said of itself when last asked. Empty: as the plugin starts.
    pub state: Arc<[u8]>,
    /// Only of an instrument; an insert's is always the default.
    pub outputs: PluginOutputs,
    pub kept: Kept,
}

impl PluginRef {
    /// As it is put in from the shelf: with the state the plugin starts with.
    pub fn new(key: &str, name: &str, vendor: &str, path: &str) -> PluginRef {
        PluginRef {
            key: key.into(),
            name: name.into(),
            vendor: vendor.into(),
            path: path.into(),
            state: Arc::from([]),
            outputs: PluginOutputs::default(),
            kept: Kept::NONE,
        }
    }

    /// The same plugin with another state.
    pub fn with_state(&self, state: Arc<[u8]>) -> PluginRef {
        PluginRef {
            state,
            ..self.clone()
        }
    }

    /// The same plugin with other settings of its output sets.
    pub fn with_outputs(&self, outputs: PluginOutputs) -> PluginRef {
        PluginRef {
            outputs,
            ..self.clone()
        }
    }
}

/// What the person set for one output set of a plugin instrument (docs/architecture.md 20-15).
/// `None`: what the default rule of the plugin host says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSetting {
    /// As the plugin host names the set.
    pub id: Arc<str>,
    /// −60.00 to +6.00 dB; never −∞ (`on` silences a set). [`OutputSetting::clamped`] keeps it
    /// so.
    pub gain: Gain,
    pub pan: Option<Pan>,
    pub on: Option<bool>,
}

impl OutputSetting {
    /// At the defaults.
    pub fn new(id: &str) -> OutputSetting {
        OutputSetting {
            id: id.into(),
            gain: Gain::UNITY,
            pan: None,
            on: None,
        }
    }

    pub fn is_default(&self) -> bool {
        self.gain == Gain::UNITY && self.pan.is_none() && self.on.is_none()
    }

    /// The same with −∞ as −60 dB.
    pub fn clamped(self) -> OutputSetting {
        OutputSetting {
            gain: output_gain(self.gain),
            ..self
        }
    }
}

/// −∞ is −60 dB: the lowest level an output set has.
fn output_gain(gain: Gain) -> Gain {
    match gain.db() {
        Some(_) => gain,
        None => Gain::from_db(Gain::MIN_DB),
    }
}

/// The settings of the output sets of a plugin instrument. Made with [`PluginOutputs::new`] or
/// [`PluginOutputs::with`], it keeps no entry at the defaults and one entry at most for each
/// id, in the order of the ids, so two tables that mean the same are equal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginOutputs {
    /// The default rule the sets follow. A rule this meloet does not know is kept as read.
    pub rule: u32,
    pub sets: Arc<[OutputSetting]>,
}

impl PluginOutputs {
    /// The rule of a plugin put in by this meloet.
    pub const DEFAULT_RULE: u32 = 1;

    /// Of each id the first entry is kept, with −∞ as −60 dB, and only when it is not at the
    /// defaults.
    pub fn new(rule: u32, sets: impl IntoIterator<Item = OutputSetting>) -> PluginOutputs {
        let mut sets: Vec<OutputSetting> = (sets.into_iter()).map(OutputSetting::clamped).collect();
        sets.sort_by(|a, b| a.id.cmp(&b.id));
        sets.dedup_by(|later, first| later.id == first.id);
        sets.retain(|set| !set.is_default());
        PluginOutputs {
            rule,
            sets: sets.into(),
        }
    }

    pub fn is_default(&self) -> bool {
        *self == PluginOutputs::default()
    }

    pub fn get(&self, id: &str) -> Option<&OutputSetting> {
        self.sets.iter().find(|set| &*set.id == id)
    }

    /// The table with `setting` in place of the entry of its id; with no entry for it when it
    /// is at the defaults.
    pub fn with(&self, setting: OutputSetting) -> PluginOutputs {
        let mut sets: Vec<OutputSetting> = (self.sets.iter())
            .filter(|set| set.id != setting.id)
            .cloned()
            .collect();
        sets.push(setting);
        PluginOutputs::new(self.rule, sets)
    }
}

impl Default for PluginOutputs {
    fn default() -> PluginOutputs {
        PluginOutputs {
            rule: PluginOutputs::DEFAULT_RULE,
            sets: Arc::from([]),
        }
    }
}

/// The bank SF2 keeps its drum kits in.
pub const SF2_DRUM_BANK: u16 = 128;

/// A preset of an SF2 file the person added. The file's name ties the song to the file
/// (20-7): a file of that name added later is taken for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sf2Ref {
    pub file: Arc<str>,
    /// Of the file as it was when the preset was put in, in lower-case hex. Empty when not
    /// known.
    pub sha256: Arc<str>,
    pub bank: u16,
    /// 0..=127.
    pub program: u8,
    /// The preset's own name, as the file has it.
    pub name: Arc<str>,
    pub kept: Kept,
}

impl Sf2Ref {
    pub fn family(&self) -> GmFamily {
        GmFamily::of(self.bank, self.program)
    }
}

/// The headings the presets of an SF2 file go under: General MIDI's sixteen families of
/// eight programs each, and the drum kits (spec 3, SF2), in the order of the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    /// `program` counts from 0; one past 127 counts as the last family.
    pub fn of(bank: u16, program: u8) -> GmFamily {
        if bank == SF2_DRUM_BANK {
            GmFamily::DrumKit
        } else {
            Self::ALL[usize::from(program.min(127) / 8)]
        }
    }

    /// The name a new track gets (spec 3 「SF2 のトラックの名前」). Part of the song, not a
    /// text of the screen.
    pub fn track_name(self, language: Language) -> &'static str {
        let (ja, en) = match self {
            GmFamily::Piano => ("ピアノ", "Piano"),
            GmFamily::Mallet => ("マレット", "Mallets"),
            GmFamily::Organ => ("オルガン", "Organ"),
            GmFamily::Guitar => ("ギター", "Guitar"),
            GmFamily::Bass => ("ベース", "Bass"),
            GmFamily::Strings => ("弦", "Strings"),
            GmFamily::Ensemble => ("アンサンブル", "Ensemble"),
            GmFamily::Brass => ("金管", "Brass"),
            GmFamily::Reed => ("木管", "Reeds"),
            GmFamily::Pipe => ("笛", "Pipes"),
            GmFamily::SynthLead | GmFamily::SynthEffects => ("シンセ", "Synth"),
            GmFamily::SynthPad => ("パッド", "Pad"),
            GmFamily::Ethnic => ("民族楽器", "Ethnic"),
            GmFamily::Percussive => ("打楽器", "Percussion"),
            GmFamily::SoundEffects => ("効果音", "Sound effects"),
            GmFamily::DrumKit => ("ドラム", "Drums"),
        };
        language.pick(ja, en)
    }

    /// The nearest group that comes with meloet (spec 3: guitars with the guitars, ethnic
    /// instruments with piano and keys, brass, reeds and pipes with the wind instruments,
    /// percussive sounds and sound effects with the synths).
    pub fn group(self) -> InstrumentGroup {
        match self {
            GmFamily::Piano | GmFamily::Mallet | GmFamily::Organ | GmFamily::Ethnic => {
                InstrumentGroup::PianoKeys
            }
            GmFamily::Guitar => InstrumentGroup::Guitar,
            GmFamily::Bass => InstrumentGroup::Bass,
            GmFamily::Strings | GmFamily::Ensemble | GmFamily::SynthPad => {
                InstrumentGroup::PadStrings
            }
            GmFamily::Brass | GmFamily::Reed | GmFamily::Pipe => InstrumentGroup::Wind,
            GmFamily::SynthLead
            | GmFamily::SynthEffects
            | GmFamily::Percussive
            | GmFamily::SoundEffects => InstrumentGroup::Synth,
            GmFamily::DrumKit => InstrumentGroup::Drums,
        }
    }
}

/// The sounds that come with meloet (spec 3 「最初から入っている音色をふやす」（84）).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinInstrument {
    GrandPiano,
    ElectricPiano,
    OrganDrawbar,
    OrganPercussive,
    OrganRock,
    /// The pipe organ (VCSL, loud). Songs from before 84 call their 「オルガン」 this.
    Organ,
    OrganQuiet,
    OrganChurch,
    OrganReed,
    Vibraphone,
    Marimba,
    GuitarNylon,
    GuitarSteel,
    GuitarElectric,
    GuitarDistortion,
    FingerBass,
    PickBass,
    WoodBass,
    SynthBass,
    StudioKit,
    ElectroKit,
    BrushKit,
    Strings,
    Pizzicato,
    WarmPad,
    Trumpet,
    BrassSection,
    AltoSax,
    Flute,
    SynthLead,
    Sine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstrumentGroup {
    PianoKeys,
    Guitar,
    Bass,
    Drums,
    PadStrings,
    Wind,
    Synth,
}

/// In the order of the shelf: the groups, and the instruments inside each. The name is the
/// one song files, the settings and the sound pack keep, so it never changes.
const TABLE: [(BuiltinInstrument, InstrumentGroup, &str); 31] = {
    use BuiltinInstrument as I;
    use InstrumentGroup as G;
    [
        (I::GrandPiano, G::PianoKeys, "grandPiano"),
        (I::ElectricPiano, G::PianoKeys, "electricPiano"),
        (I::OrganDrawbar, G::PianoKeys, "organDrawbar"),
        (I::OrganPercussive, G::PianoKeys, "organPercussive"),
        (I::OrganRock, G::PianoKeys, "organRock"),
        (I::Organ, G::PianoKeys, "organ"),
        (I::OrganQuiet, G::PianoKeys, "organQuiet"),
        (I::OrganChurch, G::PianoKeys, "organChurch"),
        (I::OrganReed, G::PianoKeys, "organReed"),
        (I::Vibraphone, G::PianoKeys, "vibraphone"),
        (I::Marimba, G::PianoKeys, "marimba"),
        (I::GuitarNylon, G::Guitar, "guitarNylon"),
        (I::GuitarSteel, G::Guitar, "guitarSteel"),
        (I::GuitarElectric, G::Guitar, "guitarElectric"),
        (I::GuitarDistortion, G::Guitar, "guitarDistortion"),
        (I::FingerBass, G::Bass, "fingerBass"),
        (I::PickBass, G::Bass, "pickBass"),
        (I::WoodBass, G::Bass, "woodBass"),
        (I::SynthBass, G::Bass, "synthBass"),
        (I::StudioKit, G::Drums, "studioKit"),
        (I::ElectroKit, G::Drums, "electroKit"),
        (I::BrushKit, G::Drums, "brushKit"),
        (I::Strings, G::PadStrings, "strings"),
        (I::Pizzicato, G::PadStrings, "pizzicato"),
        (I::WarmPad, G::PadStrings, "warmPad"),
        (I::Trumpet, G::Wind, "trumpet"),
        (I::BrassSection, G::Wind, "brassSection"),
        (I::AltoSax, G::Wind, "altoSax"),
        (I::Flute, G::Wind, "flute"),
        (I::SynthLead, G::Synth, "synthLead"),
        (I::Sine, G::Synth, "sine"),
    ]
};

impl BuiltinInstrument {
    /// Every one, in the order of the shelf.
    pub fn all() -> impl Iterator<Item = BuiltinInstrument> {
        TABLE.iter().map(|row| row.0)
    }

    fn row(self) -> &'static (BuiltinInstrument, InstrumentGroup, &'static str) {
        TABLE
            .iter()
            .find(|row| row.0 == self)
            .expect("every instrument is in the table")
    }

    pub fn group(self) -> InstrumentGroup {
        self.row().1
    }

    pub fn name(self) -> &'static str {
        self.row().2
    }

    pub fn from_name(name: &str) -> Option<BuiltinInstrument> {
        TABLE.iter().find(|row| row.2 == name).map(|row| row.0)
    }

    pub fn is_organ(self) -> bool {
        use BuiltinInstrument as I;
        matches!(
            self,
            I::OrganDrawbar
                | I::OrganPercussive
                | I::OrganRock
                | I::Organ
                | I::OrganQuiet
                | I::OrganChurch
                | I::OrganReed
        )
    }

    pub fn is_mallet(self) -> bool {
        matches!(
            self,
            BuiltinInstrument::Vibraphone | BuiltinInstrument::Marimba
        )
    }

    /// The name a new track gets (spec 3 「トラックの名前（84）」): its group's short name, but
    /// the organs are 「オルガン」 and the vibraphone and the marimba 「マレット」. Part of the
    /// song, as "はじめての曲" is: not a text of the screen.
    pub fn track_name(self, language: Language) -> &'static str {
        if self.is_organ() {
            language.pick("オルガン", "Organ")
        } else if self.is_mallet() {
            language.pick("マレット", "Mallets")
        } else {
            self.group().short_name(language)
        }
    }
}

impl InstrumentGroup {
    pub const ALL: [InstrumentGroup; 7] = [
        InstrumentGroup::PianoKeys,
        InstrumentGroup::Guitar,
        InstrumentGroup::Bass,
        InstrumentGroup::Drums,
        InstrumentGroup::PadStrings,
        InstrumentGroup::Wind,
        InstrumentGroup::Synth,
    ];

    /// The short name of the group (spec 3 「トラックの名前（84）」).
    pub fn short_name(self, language: Language) -> &'static str {
        let (ja, en) = match self {
            InstrumentGroup::PianoKeys => ("ピアノ", "Piano"),
            InstrumentGroup::Guitar => ("ギター", "Guitar"),
            InstrumentGroup::Bass => ("ベース", "Bass"),
            InstrumentGroup::Drums => ("ドラム", "Drums"),
            InstrumentGroup::PadStrings => ("パッド", "Pad"),
            InstrumentGroup::Wind => ("管楽器", "Winds"),
            InstrumentGroup::Synth => ("シンセ", "Synth"),
        };
        language.pick(ja, en)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_in_shelf_order_and_names_are_unique() {
        let names: Vec<&str> = BuiltinInstrument::all().map(|i| i.name()).collect();
        assert_eq!(
            names,
            [
                "grandPiano",
                "electricPiano",
                "organDrawbar",
                "organPercussive",
                "organRock",
                "organ",
                "organQuiet",
                "organChurch",
                "organReed",
                "vibraphone",
                "marimba",
                "guitarNylon",
                "guitarSteel",
                "guitarElectric",
                "guitarDistortion",
                "fingerBass",
                "pickBass",
                "woodBass",
                "synthBass",
                "studioKit",
                "electroKit",
                "brushKit",
                "strings",
                "pizzicato",
                "warmPad",
                "trumpet",
                "brassSection",
                "altoSax",
                "flute",
                "synthLead",
                "sine"
            ]
        );
        let mut distinct = names.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), names.len(), "every name is its own");
        for i in BuiltinInstrument::all() {
            assert_eq!(BuiltinInstrument::from_name(i.name()), Some(i));
        }
        assert_eq!(BuiltinInstrument::from_name("placeholder"), None);
        assert_eq!(
            BuiltinInstrument::from_name("organ"),
            Some(BuiltinInstrument::Organ),
            "the 「オルガン」 of songs from before 84 is the pipe organ"
        );
        // Each group's instruments are next to each other, the groups in their own order.
        let mut groups: Vec<InstrumentGroup> =
            BuiltinInstrument::all().map(|i| i.group()).collect();
        groups.dedup();
        assert_eq!(groups, InstrumentGroup::ALL);
        let count = |g| BuiltinInstrument::all().filter(|i| i.group() == g).count();
        assert_eq!(InstrumentGroup::ALL.map(count), [11, 4, 4, 3, 3, 4, 2]);
    }

    #[test]
    fn track_names() {
        assert_eq!(
            InstrumentGroup::ALL.map(|g| g.short_name(Language::Ja)),
            [
                "ピアノ",
                "ギター",
                "ベース",
                "ドラム",
                "パッド",
                "管楽器",
                "シンセ"
            ]
        );
        let organs: Vec<BuiltinInstrument> =
            BuiltinInstrument::all().filter(|i| i.is_organ()).collect();
        assert_eq!(organs.len(), 7);
        for organ in organs {
            assert_eq!(organ.group(), InstrumentGroup::PianoKeys);
            assert_eq!(organ.track_name(Language::Ja), "オルガン");
        }
        use BuiltinInstrument as I;
        let names = [
            I::GrandPiano,
            I::ElectricPiano,
            I::Vibraphone,
            I::Marimba,
            I::GuitarNylon,
            I::WoodBass,
            I::BrushKit,
            I::Pizzicato,
            I::Flute,
            I::Sine,
        ];
        assert_eq!(
            names.map(|i| i.track_name(Language::Ja)),
            [
                "ピアノ",
                "ピアノ",
                "マレット",
                "マレット",
                "ギター",
                "ベース",
                "ドラム",
                "パッド",
                "管楽器",
                "シンセ"
            ]
        );
        assert_eq!(
            names.map(|i| i.track_name(Language::En)),
            [
                "Piano", "Piano", "Mallets", "Mallets", "Guitar", "Bass", "Drums", "Pad", "Winds",
                "Synth"
            ]
        );
        assert_eq!(
            BuiltinInstrument::Sine.group(),
            InstrumentGroup::Synth,
            "the old placeholder sound is the last of the synths"
        );
    }

    #[test]
    fn sf2_families_and_what_a_track_of_each_is_called() {
        assert_eq!(GmFamily::of(0, 0), GmFamily::Piano);
        assert_eq!(GmFamily::of(0, 7), GmFamily::Piano);
        assert_eq!(GmFamily::of(0, 8), GmFamily::Mallet);
        assert_eq!(
            GmFamily::of(8, 27),
            GmFamily::Guitar,
            "the bank does not matter"
        );
        assert_eq!(GmFamily::of(0, 127), GmFamily::SoundEffects);
        assert_eq!(GmFamily::of(0, 200), GmFamily::SoundEffects);
        assert_eq!(GmFamily::of(SF2_DRUM_BANK, 0), GmFamily::DrumKit);
        assert_eq!(GmFamily::of(SF2_DRUM_BANK, 40), GmFamily::DrumKit);
        for (i, f) in GmFamily::ALL.into_iter().take(16).enumerate() {
            assert_eq!(GmFamily::of(0, i as u8 * 8), f);
        }
        assert_eq!(
            GmFamily::ALL.map(|f| f.track_name(Language::Ja)),
            [
                "ピアノ",
                "マレット",
                "オルガン",
                "ギター",
                "ベース",
                "弦",
                "アンサンブル",
                "金管",
                "木管",
                "笛",
                "シンセ",
                "パッド",
                "シンセ",
                "民族楽器",
                "打楽器",
                "効果音",
                "ドラム"
            ]
        );
        use InstrumentGroup as G;
        assert_eq!(
            GmFamily::ALL.map(GmFamily::group),
            [
                G::PianoKeys,
                G::PianoKeys,
                G::PianoKeys,
                G::Guitar,
                G::Bass,
                G::PadStrings,
                G::PadStrings,
                G::Wind,
                G::Wind,
                G::Wind,
                G::Synth,
                G::PadStrings,
                G::Synth,
                G::PianoKeys,
                G::Synth,
                G::Synth,
                G::Drums
            ]
        );
    }

    #[test]
    fn what_an_added_instrument_is_treated_as() {
        let plugin = InstrumentRef::Plugin(Arc::new(PluginRef::new(
            "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
            "Surge XT",
            "Surge Synth Team",
            "/usr/lib/lv2/Surge XT.lv2",
        )));
        assert_eq!(plugin.track_name(Language::Ja), "Surge XT");
        assert_eq!(plugin.group(), InstrumentGroup::Synth);
        let preset = |bank, program, name: &str| {
            InstrumentRef::Sf2(Arc::new(Sf2Ref {
                file: "FluidR3_GM.sf2".into(),
                sha256: "".into(),
                bank,
                program,
                name: name.into(),
                kept: Kept::NONE,
            }))
        };
        let guitar = preset(0, 27, "Clean Guitar");
        assert_eq!(guitar.track_name(Language::Ja), "ギター");
        assert_eq!(guitar.group(), InstrumentGroup::Guitar);
        let kit = preset(SF2_DRUM_BANK, 0, "Standard");
        assert_eq!(kit.track_name(Language::Ja), "ドラム");
        assert_eq!(kit.group(), InstrumentGroup::Drums);
        assert_eq!(
            InstrumentRef::GRAND_PIANO.track_name(Language::Ja),
            "ピアノ"
        );
        assert_eq!(InstrumentRef::SINE.group(), InstrumentGroup::Synth);
    }

    #[test]
    fn a_plugin_with_another_state_is_another_instrument() {
        let a = PluginRef::new("vst3:00", "A", "V", "");
        let b = a.with_state(Arc::from([1u8, 2, 3]));
        assert_ne!(a, b);
        assert_eq!((&*b.key, &*b.name, &*b.vendor), ("vst3:00", "A", "V"));
        assert_eq!(Kept::new("").text(), None);
        assert_eq!(Kept::new(r#"{"x":1}"#).text(), Some(r#"{"x":1}"#));
    }
}
