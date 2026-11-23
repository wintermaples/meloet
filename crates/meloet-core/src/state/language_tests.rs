//! D4 「ことば / Language」: a setting; the PC's language until one is chosen; the names of new
//! songs and tracks in the language they are made in.

use meloet_protocol::Language;

use super::tests::playing;
use super::*;

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn names(m: &Model) -> (String, String) {
    let song = m.view().song;
    (song.name, song.tracks[0].name.clone())
}

#[test]
fn the_pcs_language_is_the_screens_until_one_is_chosen() {
    let m = Model::new(Settings::default());
    assert_eq!(m.view().workspace.language, Language::Ja);
    assert_eq!(names(&m), ("はじめての曲".into(), "ピアノ".into()));

    let m = Model::new(Settings::default()).with_pc_language(Language::En);
    assert_eq!(m.view().workspace.language, Language::En);
    assert_eq!(names(&m), ("My first song".into(), "Piano".into()));
    assert_eq!(m.settings().language, None, "nothing is chosen yet");

    let chosen = Settings {
        language: Some(Language::Ja),
        ..Settings::default()
    };
    let m = Model::new(chosen).with_pc_language(Language::En);
    assert_eq!(m.view().workspace.language, Language::Ja);
    assert_eq!(names(&m), ("はじめての曲".into(), "ピアノ".into()));
}

#[test]
fn choosing_is_kept_in_the_settings_and_names_what_is_made_from_then_on() {
    let mut m = Model::new(Settings::default());
    let before = m.view();
    let effects = cmd(
        &mut m,
        Command::SetLanguage {
            language: Language::En,
        },
    );
    let [Effect::SaveSettings(saved)] = &effects[..] else {
        panic!("{effects:?}");
    };
    assert_eq!(saved.language, Some(Language::En));
    let after = m.view();
    assert_eq!(after.workspace.language, Language::En);
    assert_eq!(m.play().undo_count(), 0, "not a step");
    assert_eq!(after.song, before.song, "the song keeps its names");
    assert_eq!(
        cmd(
            &mut m,
            Command::SetLanguage {
                language: Language::En
            }
        ),
        []
    );

    // No sound pack here: the first track is the sine wave, under its group's name.
    cmd(&mut m, Command::NewSong);
    assert_eq!(names(&m), ("My first song".into(), "Synth".into()));
}

#[test]
fn it_cannot_be_changed_while_counting_in_or_recording() {
    let mut m = playing();
    cmd(&mut m, Command::Record);
    assert!(m.play.busy());
    let command = Command::SetLanguage {
        language: Language::En,
    };
    assert_eq!(cmd(&mut m, command), []);
    assert_eq!(m.view().workspace.language, Language::Ja);
}

/// A song made in Japanese, saved and opened while the screen is English, keeps its names.
#[test]
fn a_song_keeps_its_names_in_the_other_language() {
    let mut song = Song::new_default();
    let (e, _) =
        meloet_model::edit::add_vocal_track(&song.doc, None, meloet_model::Language::Ja).unwrap();
    song.doc = e.doc;
    let read = crate::songfile::decode(&crate::songfile::encode(&song)).unwrap();
    let mut m = Model::new(Settings::default()).with_pc_language(Language::En);
    m.replace_song(read, false, &mut Vec::new());
    let view = m.view().song;
    let tracks: Vec<&str> = view.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        (view.name.as_str(), tracks),
        ("はじめての曲", vec!["ピアノ", "歌"])
    );
}
