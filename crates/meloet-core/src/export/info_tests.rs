//! 「曲の情報」 of the export (round 12 F2) through the whole of the rules, and in the files a
//! job writes.

use std::path::PathBuf;

use lofty::file::TaggedFileExt;
use lofty::tag::{Accessor, TagType};
use meloet_protocol::{
    Command, ExportChange, ExportFormat, ExportLoudness, ExportTail, RecordFormat, StatusMessage,
};

use super::fixture::{BAR, Place, RATE, song as fixture_song};
use super::tags::tests::picture;
use super::tags::{PictureKind, PictureProblem, SongInfo};
use super::{ExportAsk, ExportDirs, ExportNews, ExportTarget};
use crate::settings::Settings;
use crate::state::{Effect, Input, Model};

fn effects(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn asks(effects: Vec<Effect>) -> Vec<ExportAsk> {
    (effects.into_iter())
        .filter_map(|e| match e {
            Effect::Export(ask) => Some(ask),
            _ => None,
        })
        .collect()
}

fn set(m: &mut Model, change: ExportChange) -> Vec<ExportAsk> {
    asks(effects(m, Command::SetExport { change }))
}

fn news(m: &mut Model, news: ExportNews) {
    m.handle(Input::Export(news));
}

/// A song with one clip on its first track, never saved.
fn song(settings: Settings) -> Model {
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

/// The job of the dialog says it is rendered.
fn counted(m: &mut Model, asked: &[ExportAsk]) {
    let Some(ExportAsk::Render(job)) = asked.first() else {
        panic!("no job: {asked:?}");
    };
    news(
        m,
        ExportNews::Counted {
            job: job.id,
            percent: 100,
            peak_db: Some(-6.0),
            over: 0,
            length_s: 2,
            remaining_s: None,
            lufs: None,
            true_peak_db: None,
            done: true,
        },
    );
}

fn target(asked: &[ExportAsk]) -> &ExportTarget {
    match asked {
        [ExportAsk::Finish { target, .. }] => target,
        other => panic!("not one finish: {other:?}"),
    }
}

#[test]
fn the_title_is_the_songs_name_until_typed_and_goes_with_the_song() {
    let mut m = song(Settings::default());
    let name = m.view().song.name;
    effects(&mut m, Command::OpenExport);
    let d = dialog(&m);
    assert_eq!((d.title.as_str(), d.artist.as_str()), (name.as_str(), ""));
    assert_eq!((&d.picture, d.picture_usable), (&None, false));
    assert!(
        set(
            &mut m,
            ExportChange::Title {
                title: "歌".into()
            }
        )
        .is_empty()
    );
    assert_eq!(dialog(&m).title, "歌");
    // Empty is what was typed.
    set(
        &mut m,
        ExportChange::Title {
            title: String::new(),
        },
    );
    assert_eq!(dialog(&m).title, "");
    set(
        &mut m,
        ExportChange::Title {
            title: "歌".into()
        },
    );

    // Kept when the dialog opens again for the same song…
    effects(&mut m, Command::CloseExport);
    effects(&mut m, Command::OpenExport);
    assert_eq!(dialog(&m).title, "歌");
    // …and not for another one.
    effects(&mut m, Command::CloseExport);
    effects(&mut m, Command::NewSong);
    let asked = m
        .view()
        .workspace
        .song_dialog
        .expect("unsaved: a question")
        .id;
    effects(
        &mut m,
        Command::AnswerSongDialog {
            dialog: asked,
            answer: meloet_protocol::SongAnswer::DontSave,
        },
    );
    effects(&mut m, Command::OpenExport);
    assert_eq!(dialog(&m).title, m.view().song.name);
}

#[test]
fn the_artist_is_kept_with_the_settings() {
    let mut m = song(Settings::default());
    effects(&mut m, Command::OpenExport);
    let said = effects(
        &mut m,
        Command::SetExport {
            change: ExportChange::Artist {
                artist: "わたし".into(),
            },
        },
    );
    let saved: Vec<&Settings> = (said.iter())
        .filter_map(|e| match e {
            Effect::SaveSettings(s) => Some(&**s),
            _ => None,
        })
        .collect();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].export_artist, "わたし");
    assert_eq!(dialog(&m).artist, "わたし");
    // The same again is not saved again.
    let again = effects(
        &mut m,
        Command::SetExport {
            change: ExportChange::Artist {
                artist: "わたし".into(),
            },
        },
    );
    assert!(!again.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    // Without the dialog nothing changes.
    effects(&mut m, Command::CloseExport);
    let closed = effects(
        &mut m,
        Command::SetExport {
            change: ExportChange::Artist { artist: "x".into() },
        },
    );
    assert!(!closed.iter().any(|e| matches!(e, Effect::SaveSettings(_))));

    let settings = Settings {
        export_artist: "わたし".into(),
        ..Settings::default()
    };
    let mut m = song(settings);
    effects(&mut m, Command::OpenExport);
    assert_eq!(dialog(&m).artist, "わたし");
}

#[test]
fn a_picture_is_chosen_in_the_pcs_window_and_one_that_cannot_be_used_is_told() {
    let mut m = song(Settings::default());
    effects(&mut m, Command::OpenExport);
    // A wav holds no picture: the row cannot be used.
    assert!(set(&mut m, ExportChange::ChoosePicture).is_empty());
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Flac,
        },
    );
    assert!(dialog(&m).picture_usable);
    assert_eq!(
        set(&mut m, ExportChange::ChoosePicture),
        [ExportAsk::PickPicture {
            folder: Some(PathBuf::from("/home/me"))
        }]
    );
    // One window at a time.
    assert!(set(&mut m, ExportChange::ChoosePicture).is_empty());
    news(&mut m, ExportNews::PicturePicked(None));
    assert_eq!(dialog(&m).picture, None);

    let at = PathBuf::from("/home/me/絵/表紙.png");
    set(&mut m, ExportChange::ChoosePicture);
    news(
        &mut m,
        ExportNews::PicturePicked(Some((at.clone(), Ok(picture(PictureKind::Png))))),
    );
    assert_eq!(dialog(&m).picture.as_deref(), Some("表紙.png"));
    // The next window starts where the last picture was.
    assert_eq!(
        set(&mut m, ExportChange::ChoosePicture),
        [ExportAsk::PickPicture {
            folder: Some(PathBuf::from("/home/me/絵"))
        }]
    );
    let big = PathBuf::from("/home/me/絵/大きい.png");
    news(
        &mut m,
        ExportNews::PicturePicked(Some((big, Err(PictureProblem::TooLarge)))),
    );
    assert_eq!(
        m.view().status.message,
        Some(StatusMessage::ExportPictureTooLarge {
            file: "大きい.png".into()
        })
    );
    assert_eq!(dialog(&m).picture.as_deref(), Some("表紙.png"));
    set(&mut m, ExportChange::ChoosePicture);
    let bmp = PathBuf::from("/home/me/絵/表紙.bmp");
    news(
        &mut m,
        ExportNews::PicturePicked(Some((bmp, Err(PictureProblem::Unreadable)))),
    );
    assert_eq!(
        m.view().status.message,
        Some(StatusMessage::ExportPictureUnreadable {
            file: "表紙.bmp".into()
        })
    );
    // An answer nobody waits for is not taken.
    news(
        &mut m,
        ExportNews::PicturePicked(Some((at, Ok(picture(PictureKind::Jpeg))))),
    );
    assert_eq!(dialog(&m).picture.as_deref(), Some("表紙.png"));
    set(&mut m, ExportChange::RemovePicture);
    assert_eq!(dialog(&m).picture, None);
}

#[test]
fn the_export_takes_the_information_and_a_wav_leaves_the_picture_out() {
    let mut m = song(Settings::default());
    let asked = asks(effects(&mut m, Command::OpenExport));
    counted(&mut m, &asked);
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Flac,
        },
    );
    set(&mut m, ExportChange::ChoosePicture);
    let png = picture(PictureKind::Png);
    news(
        &mut m,
        ExportNews::PicturePicked(Some(("/p/表紙.png".into(), Ok(png.clone())))),
    );
    set(
        &mut m,
        ExportChange::Title {
            title: "歌".into()
        },
    );
    set(
        &mut m,
        ExportChange::Artist {
            artist: "わたし".into(),
        },
    );
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Wav,
        },
    );
    // A wav: the picture stays chosen, and is not written.
    assert_eq!(dialog(&m).picture.as_deref(), Some("表紙.png"));
    assert!(!dialog(&m).picture_usable);
    let started = asks(effects(&mut m, Command::StartExport));
    assert_eq!(
        target(&started).info,
        SongInfo {
            title: "歌".into(),
            artist: "わたし".into(),
            picture: None,
        }
    );

    let mut m = song(Settings::default());
    let asked = asks(effects(&mut m, Command::OpenExport));
    counted(&mut m, &asked);
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Flac,
        },
    );
    set(&mut m, ExportChange::ChoosePicture);
    news(
        &mut m,
        ExportNews::PicturePicked(Some(("/p/表紙.png".into(), Ok(png.clone())))),
    );
    let started = asks(effects(&mut m, Command::StartExport));
    let name = m.view().song.name;
    assert_eq!(
        target(&started).info,
        SongInfo {
            title: name,
            artist: String::new(),
            picture: Some(png),
        }
    );
}

#[test]
fn each_track_of_tracks_is_titled_after_the_song_and_one_file_after_the_title() {
    use crate::audiofiles::decode::Decoder;
    let place = Place::new("export-info-tracks");
    let (doc, _) = fixture_song();
    place.files_of(&doc);
    let info = SongInfo {
        title: "はじめての曲".into(),
        artist: "わたし".into(),
        picture: Some(picture(PictureKind::Jpeg)),
    };
    let out = place.root.join("out");
    let finish = |id: u64, per_track: bool, path: PathBuf, format: ExportFormat| {
        let mut job = place.job(id, &doc, (0, BAR), ExportTail::Off);
        job.per_track = per_track;
        let run = place.start(place.setup(job));
        run.rendered();
        run.handle.finish(ExportTarget {
            path,
            format,
            sample_rate: RATE,
            bits: RecordFormat::Bits24,
            loudness: ExportLoudness::AsIs,
            mp3_kbps: meloet_protocol::EXPORT_MP3_DEFAULT_KBPS,
            info: info.clone(),
        });
        let seen = run.until("finished", |n| {
            matches!(n, ExportNews::Finished { .. } | ExportNews::Failed { .. })
        });
        match seen.into_iter().last() {
            Some(ExportNews::Finished { files, .. }) => files,
            other => panic!("not finished: {other:?}"),
        }
    };
    let files = finish(1, true, out.clone(), ExportFormat::Flac);
    let names: Vec<String> = doc.tracks.iter().map(|t| t.name.clone()).collect();
    assert_eq!(files.len(), names.len());
    for (file, name) in files.iter().zip(&names) {
        let tagged = lofty::read_from_path(file).unwrap();
        let tag = tagged.tag(TagType::VorbisComments).unwrap();
        assert_eq!(
            tag.title().as_deref(),
            Some(&*format!("はじめての曲 - {name}"))
        );
        assert_eq!(tag.artist().as_deref(), Some("わたし"));
        assert_eq!(tag.pictures().len(), 1);
        assert!(Decoder::open(file).unwrap().header_frames().is_some());
    }

    let wav = finish(2, false, out.join("song.wav"), ExportFormat::Wav);
    let tagged = lofty::read_from_path(&wav[0]).unwrap();
    let tag = tagged.tag(TagType::RiffInfo).unwrap();
    assert_eq!(tag.title().as_deref(), Some("はじめての曲"));
    assert!(tag.pictures().is_empty());
}
