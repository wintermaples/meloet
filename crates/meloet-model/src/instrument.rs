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
    pub fn track_name(self) -> &'static str {
        if self.is_organ() {
            "オルガン"
        } else if self.is_mallet() {
            "マレット"
        } else {
            self.group().short_name()
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
    pub fn short_name(self) -> &'static str {
        match self {
            InstrumentGroup::PianoKeys => "ピアノ",
            InstrumentGroup::Guitar => "ギター",
            InstrumentGroup::Bass => "ベース",
            InstrumentGroup::Drums => "ドラム",
            InstrumentGroup::PadStrings => "パッド",
            InstrumentGroup::Wind => "管楽器",
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
            InstrumentGroup::ALL.map(InstrumentGroup::short_name),
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
            assert_eq!(organ.track_name(), "オルガン");
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
        ]
        .map(BuiltinInstrument::track_name);
        assert_eq!(
            names,
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
            BuiltinInstrument::Sine.group(),
            InstrumentGroup::Synth,
            "the old placeholder sound is the last of the synths"
        );
    }
}
