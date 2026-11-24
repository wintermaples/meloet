//! MIDI in the rules of the export (protocol 36; spec 3 "MIDI export"): the choices of
//! 「入れるトラック」 are kept, and the choices of audio stay for the audio formats.

use std::path::PathBuf;

use meloet_protocol::{Command, ExportChange, ExportFormat, ExportMidiTracks, RecordFormat};

use super::ExportDirs;
use crate::settings::Settings;
use crate::state::{Input, Model};

fn cmd(m: &mut Model, command: Command) {
    m.handle(Input::Command(command));
}

fn set(m: &mut Model, change: ExportChange) {
    cmd(m, Command::SetExport { change });
}

fn dialog(m: &Model) -> meloet_protocol::ExportDialogView {
    m.view().export.dialog.expect("the dialog")
}

#[test]
fn midi_is_the_last_format_and_keeps_the_choices_of_audio_and_its_own() {
    let mut m = Model::new(Settings::default()).with_export_dirs(ExportDirs {
        home: Some(PathBuf::from("/home/me")),
        usual: Some(PathBuf::from("/home/me/音楽")),
    });
    cmd(
        &mut m,
        Command::AddNote {
            take: None,
            start: 0,
            length: 480,
            key: 60,
            velocity: 100,
        },
    );
    cmd(&mut m, Command::OpenExport);
    let d = dialog(&m);
    assert_eq!(d.formats.last(), Some(&ExportFormat::Midi));
    assert_eq!(
        (d.midi_tracks, d.midi_numbers.as_str()),
        (ExportMidiTracks::All, "")
    );
    set(
        &mut m,
        ExportChange::Bits {
            bits: RecordFormat::Bits16,
        },
    );
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Midi,
        },
    );
    let d = dialog(&m);
    assert!(
        d.place.starts_with("~/音楽/") && d.place.ends_with(".mid"),
        "{}",
        d.place
    );
    assert!(d.bits_choices.is_empty());
    assert!(!d.picture_usable);

    set(
        &mut m,
        ExportChange::MidiTracks {
            tracks: ExportMidiTracks::Numbers,
        },
    );
    set(
        &mut m,
        ExportChange::MidiNumbers {
            text: "1-3, 5".to_owned(),
        },
    );
    // Kept while meloet runs, as the other choices.
    cmd(&mut m, Command::CloseExport);
    cmd(&mut m, Command::OpenExport);
    let d = dialog(&m);
    assert_eq!(d.format, ExportFormat::Midi);
    assert_eq!(
        (d.midi_tracks, d.midi_numbers.as_str()),
        (ExportMidiTracks::Numbers, "1-3, 5")
    );

    // Back to wav: the bits chosen before.
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Wav,
        },
    );
    let d = dialog(&m);
    assert_eq!(d.bits, RecordFormat::Bits16);
    assert!(d.place.ends_with(".wav"), "{}", d.place);
}
