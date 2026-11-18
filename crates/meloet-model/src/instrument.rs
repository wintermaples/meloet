//! The instruments a track can have, and the one table of the ones that come with meloet.

/// What a track sounds with. Plugin instruments get their own arm here later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentRef {
    Builtin(BuiltinInstrument),
}

impl InstrumentRef {
    /// What a track falls back to when its instrument is not known.
    pub const SINE: InstrumentRef = InstrumentRef::Builtin(BuiltinInstrument::Sine);

    /// The first track of a new song.
    pub const GRAND_PIANO: InstrumentRef = InstrumentRef::Builtin(BuiltinInstrument::GrandPiano);
}

/// The sounds that come with meloet (spec 3 「最初から入っている音色」).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinInstrument {
    GrandPiano,
    ElectricPiano,
    Organ,
    FingerBass,
    SynthBass,
    StudioKit,
    ElectroKit,
    Strings,
    WarmPad,
    SynthLead,
    Sine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstrumentGroup {
    PianoKeys,
    Bass,
    Drums,
    PadStrings,
    Synth,
}

/// In the order of the shelf: the groups, and the instruments inside each. The name is the
/// one song files and the settings keep, so it never changes.
const TABLE: [(BuiltinInstrument, InstrumentGroup, &str); 11] = {
    use BuiltinInstrument as I;
    use InstrumentGroup as G;
    [
        (I::GrandPiano, G::PianoKeys, "grandPiano"),
        (I::ElectricPiano, G::PianoKeys, "electricPiano"),
        (I::Organ, G::PianoKeys, "organ"),
        (I::FingerBass, G::Bass, "fingerBass"),
        (I::SynthBass, G::Bass, "synthBass"),
        (I::StudioKit, G::Drums, "studioKit"),
        (I::ElectroKit, G::Drums, "electroKit"),
        (I::Strings, G::PadStrings, "strings"),
        (I::WarmPad, G::PadStrings, "warmPad"),
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
}

impl InstrumentGroup {
    pub const ALL: [InstrumentGroup; 5] = [
        InstrumentGroup::PianoKeys,
        InstrumentGroup::Bass,
        InstrumentGroup::Drums,
        InstrumentGroup::PadStrings,
        InstrumentGroup::Synth,
    ];

    /// The name a new track gets (spec 3 「トラック」). Part of the song, as "はじめての曲" is:
    /// not a text of the screen.
    pub fn track_name(self) -> &'static str {
        match self {
            InstrumentGroup::PianoKeys => "ピアノ",
            InstrumentGroup::Bass => "ベース",
            InstrumentGroup::Drums => "ドラム",
            InstrumentGroup::PadStrings => "パッド",
            InstrumentGroup::Synth => "シンセ",
        }
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
                "organ",
                "fingerBass",
                "synthBass",
                "studioKit",
                "electroKit",
                "strings",
                "warmPad",
                "synthLead",
                "sine"
            ]
        );
        for i in BuiltinInstrument::all() {
            assert_eq!(BuiltinInstrument::from_name(i.name()), Some(i));
        }
        assert_eq!(BuiltinInstrument::from_name("placeholder"), None);
        // Each group's instruments are next to each other, the groups in their own order.
        let mut groups: Vec<InstrumentGroup> =
            BuiltinInstrument::all().map(|i| i.group()).collect();
        groups.dedup();
        assert_eq!(groups, InstrumentGroup::ALL);
    }

    #[test]
    fn track_names_of_the_groups() {
        let names = InstrumentGroup::ALL.map(InstrumentGroup::track_name);
        assert_eq!(names, ["ピアノ", "ベース", "ドラム", "パッド", "シンセ"]);
        assert_eq!(
            BuiltinInstrument::Sine.group(),
            InstrumentGroup::Synth,
            "the old placeholder sound is the last of the synths"
        );
    }
}
