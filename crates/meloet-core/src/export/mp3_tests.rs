//! mp3 in the rules of the export (round 12 F2): its bitrates, its two rates, no bits, and
//! the bitrate in 「書き出しました」.

use std::path::PathBuf;

use meloet_protocol::{
    Command, EXPORT_MP3_KBPS, ExportChange, ExportFormat, ExportRunView, RecordFormat,
};

use super::{ExportAsk, ExportDirs, ExportNews, mp3_rate};
use crate::settings::{Settings, SoundSettings};
use crate::state::{Effect, Input, Model};

fn asks(effects: Vec<Effect>) -> Vec<ExportAsk> {
    (effects.into_iter())
        .filter_map(|e| match e {
            Effect::Export(ask) => Some(ask),
            _ => None,
        })
        .collect()
}

fn cmd(m: &mut Model, command: Command) -> Vec<ExportAsk> {
    asks(m.handle(Input::Command(command)))
}

fn set(m: &mut Model, change: ExportChange) -> Vec<ExportAsk> {
    cmd(m, Command::SetExport { change })
}

/// A song with one clip, played at `rate`.
fn song(rate: u32) -> Model {
    let settings = Settings {
        sound: SoundSettings {
            sample_rate: rate,
            ..SoundSettings::default()
        },
        ..Settings::default()
    };
    let mut m = Model::new(settings).with_export_dirs(ExportDirs {
        home: Some(PathBuf::from("/home/me")),
        usual: Some(PathBuf::from("/home/me/音楽")),
    });
    m.handle(Input::Command(Command::AddNote {
        take: None,
        start: 0,
        length: 480,
        key: 60,
        velocity: 100,
    }));
    m
}

fn dialog(m: &Model) -> meloet_protocol::ExportDialogView {
    m.view().export.dialog.expect("the dialog")
}

#[test]
fn another_rate_gives_the_mp3_rate_of_its_family() {
    assert_eq!([22_050, 44_100, 88_200, 176_400].map(mp3_rate), [44_100; 4]);
    assert_eq!([32_000, 48_000, 96_000, 192_000].map(mp3_rate), [48_000; 4]);
}

#[test]
fn mp3_has_its_bitrates_and_two_rates_and_keeps_the_bits_of_the_others() {
    let mut m = song(96_000);
    let opened = cmd(&mut m, Command::OpenExport);
    set(
        &mut m,
        ExportChange::Bits {
            bits: RecordFormat::Bits16,
        },
    );
    let d = dialog(&m);
    assert_eq!(d.formats, ExportFormat::ALL);
    assert_eq!(d.mp3_kbps_choices, EXPORT_MP3_KBPS);
    assert_eq!((d.mp3_kbps, d.sample_rate), (192, 96_000));

    // Choosing the format leaves the job running: it belongs to the last step.
    assert!(
        set(
            &mut m,
            ExportChange::Format {
                format: ExportFormat::Mp3
            }
        )
        .is_empty()
    );
    let d = dialog(&m);
    assert!(d.bits_choices.is_empty());
    assert_eq!(d.bits, RecordFormat::Bits16);
    assert_eq!(
        (d.sample_rate, &d.sample_rate_choices[..]),
        (48_000, &[44_100, 48_000][..])
    );
    assert!(d.place.ends_with(".mp3"), "{}", d.place);
    assert!(d.picture_usable);
    // Not a rate or a bitrate of mp3.
    set(&mut m, ExportChange::SampleRate { hz: 96_000 });
    set(&mut m, ExportChange::Mp3Kbps { kbps: 160 });
    assert_eq!((dialog(&m).sample_rate, dialog(&m).mp3_kbps), (48_000, 192));
    set(&mut m, ExportChange::SampleRate { hz: 44_100 });
    set(&mut m, ExportChange::Mp3Kbps { kbps: 320 });
    // Bits mean nothing to mp3.
    set(
        &mut m,
        ExportChange::Bits {
            bits: RecordFormat::Bits24,
        },
    );
    let d = dialog(&m);
    assert_eq!(
        (d.sample_rate, d.mp3_kbps, d.bits),
        (44_100, 320, RecordFormat::Bits16)
    );

    // Back to wav: the bits and the rate as they were chosen.
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Wav,
        },
    );
    let d = dialog(&m);
    assert_eq!((d.bits, d.sample_rate), (RecordFormat::Bits16, 44_100));
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Mp3,
        },
    );

    // 「書き出す」: the target says the bitrate, and so does 「書き出しました」.
    let Some(ExportAsk::Render(job)) = opened.first() else {
        panic!("no job");
    };
    let id = job.id;
    m.handle(Input::Export(ExportNews::Counted {
        job: id,
        percent: 100,
        peak_db: Some(-3.0),
        over: 0,
        length_s: 2,
        remaining_s: None,
        lufs: None,
        true_peak_db: None,
        done: true,
    }));
    let started = cmd(&mut m, Command::StartExport);
    let [ExportAsk::Finish { target, .. }] = &started[..] else {
        panic!("not one finish: {started:?}");
    };
    assert_eq!(
        (target.format, target.sample_rate, target.mp3_kbps),
        (ExportFormat::Mp3, 44_100, 320)
    );
    let file = target.path.clone();
    m.handle(Input::Export(ExportNews::Finished {
        job: id,
        files: vec![file],
        wave: vec![9; 200],
    }));
    let Some(ExportRunView::Done { mp3_kbps, .. }) = m.view().export.run else {
        panic!("not done");
    };
    assert_eq!(mp3_kbps, Some(320));
}

#[test]
fn a_wav_or_a_flac_has_no_bitrate_when_done() {
    let mut m = song(48_000);
    let opened = cmd(&mut m, Command::OpenExport);
    let Some(ExportAsk::Render(job)) = opened.first() else {
        panic!("no job");
    };
    let id = job.id;
    m.handle(Input::Export(ExportNews::Counted {
        job: id,
        percent: 100,
        peak_db: Some(-3.0),
        over: 0,
        length_s: 2,
        remaining_s: None,
        lufs: None,
        true_peak_db: None,
        done: true,
    }));
    let started = cmd(&mut m, Command::StartExport);
    let [ExportAsk::Finish { target, .. }] = &started[..] else {
        panic!("not one finish");
    };
    let file = target.path.clone();
    m.handle(Input::Export(ExportNews::Finished {
        job: id,
        files: vec![file],
        wave: vec![9; 200],
    }));
    let Some(ExportRunView::Done { mp3_kbps, .. }) = m.view().export.run else {
        panic!("not done");
    };
    assert_eq!(mp3_kbps, None);
}
