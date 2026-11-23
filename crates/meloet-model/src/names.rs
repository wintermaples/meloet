//! The language the names of new things are made in (round 12, D4): a new song, a new track.
//! Once made, a name is part of the song: it stays as it is when the language changes later,
//! and a song made in one language keeps its names when opened in the other.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    #[default]
    Ja,
    En,
}

impl Language {
    pub fn pick(self, ja: &'static str, en: &'static str) -> &'static str {
        match self {
            Language::Ja => ja,
            Language::En => en,
        }
    }
}

/// The name of the song a person starts from.
pub fn first_song_name(language: Language) -> &'static str {
    language.pick("はじめての曲", "My first song")
}
