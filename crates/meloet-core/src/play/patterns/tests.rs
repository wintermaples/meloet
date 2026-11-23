use meloet_model::BuiltinInstrument as B;
use meloet_model::InstrumentRef;
use meloet_protocol::{
    Band, ClipView, Command, InstrumentKind, PatternGroup, ShelfMode, SoundRef, StatusMessage,
    TakeNotes, TrackKindView,
};

use crate::patterns;
use crate::play::EngineOp;
use crate::play::tests::{cmd, ready, status};
use crate::state::{Effect, Input, Model};

const BAR: u64 = 4 * meloet_model::TICKS_PER_QUARTER;

fn ops(effects: &[Effect]) -> Vec<EngineOp> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(op) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

/// A new song (track 1 グランドピアノ) on a PC with the three kits, the shelf open as for a
/// new track.
fn shelf() -> Model {
    let mut m = ready().with_sounds(vec![B::GrandPiano, B::StudioKit, B::ElectroKit]);
    cmd(
        &mut m,
        Command::OpenShelf {
            mode: ShelfMode::NewTrack,
        },
    );
    m
}

fn place(m: &mut Model, id: &str) -> Vec<Effect> {
    cmd(m, Command::PlacePattern { id: id.into() })
}

fn steps(m: &Model) -> u32 {
    m.view().song.undo_count
}

fn clips(m: &Model, track: usize) -> Vec<ClipView> {
    m.view().song.tracks[track].clips.clone()
}

/// (song tick, key, length, velocity) of the clip's notes.
fn notes_of(m: &Model, clip: &ClipView) -> Vec<(u64, u8, u64, u8)> {
    let notes = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    let zero = (clip.start as i64 + clip.take_offset) as u64;
    (notes.notes.iter())
        .map(|n| {
            (
                zero + u64::from(n.start),
                n.key,
                u64::from(n.length),
                n.velocity,
            )
        })
        .collect()
}

fn expected(id: &str, start: u64) -> Vec<(u64, u8, u64, u8)> {
    let mut want: Vec<_> = (patterns::find(id).unwrap().notes_at(start).into_iter())
        .map(|n| (n.start, n.key, n.length, n.velocity))
        .collect();
    want.sort_by_key(|n| (n.0, n.1));
    want
}

#[test]
fn the_shelf_lists_the_patterns_only_for_a_new_track() {
    let m = &mut shelf();
    let view = m.view().shelf;
    assert_eq!(view.patterns.len(), 12);
    assert_eq!(view.patterns[0].id, "eightBeat");
    assert_eq!(view.patterns[0].bars, 1);
    assert_eq!(view.patterns[11].group, PatternGroup::Fill);

    cmd(
        m,
        Command::ShelfSelectPattern {
            id: Some("shuffle".into()),
        },
    );
    assert_eq!(m.view().shelf.selected_pattern.as_deref(), Some("shuffle"));
    cmd(
        m,
        Command::ShelfSelectPattern {
            id: Some("nope".into()),
        },
    );
    assert_eq!(m.view().shelf.selected_pattern, None);
    cmd(
        m,
        Command::ShelfSelectPattern {
            id: Some("shuffle".into()),
        },
    );

    // Replacing a track's sound: no patterns.
    cmd(
        m,
        Command::OpenShelf {
            mode: ShelfMode::Replace { track: 1 },
        },
    );
    assert!(m.view().shelf.patterns.is_empty());
    assert_eq!(m.view().shelf.selected_pattern, None);
    cmd(
        m,
        Command::ShelfSelectPattern {
            id: Some("shuffle".into()),
        },
    );
    assert_eq!(m.view().shelf.selected_pattern, None);

    // Closing the shelf forgets the row.
    cmd(
        m,
        Command::OpenShelf {
            mode: ShelfMode::NewTrack,
        },
    );
    cmd(
        m,
        Command::ShelfSelectPattern {
            id: Some("ballad".into()),
        },
    );
    cmd(m, Command::ToggleBand { band: Band::Shelf });
    assert_eq!(m.view().shelf.selected_pattern, None);
    // And with the shelf closed nothing is chosen or placed.
    cmd(
        m,
        Command::ShelfSelectPattern {
            id: Some("ballad".into()),
        },
    );
    assert_eq!(m.view().shelf.selected_pattern, None);
    place(m, "ballad");
    assert_eq!(steps(m), 0);
}

#[test]
fn on_a_track_that_is_no_drum_track_it_makes_a_studio_kit_track_in_the_same_step() {
    let m = &mut shelf();
    cmd(m, Command::Locate { tick: BAR + 960 });
    assert!(place(m, "eightBeat").iter().any(|e| matches!(
        e,
        Effect::Engine(EngineOp::Locate { tick }) if *tick == 2 * BAR
    )));
    assert_eq!(status(m), None);
    assert_eq!(steps(m), 1);
    let view = m.view();
    assert_eq!(view.song.tracks.len(), 2);
    let kit = &view.song.tracks[1];
    assert_eq!(kit.name, "ドラム");
    assert!(matches!(kit.kind, TrackKindView::Keys));
    assert_eq!(
        kit.sound,
        meloet_protocol::SoundView::Builtin {
            instrument: InstrumentKind::StudioKit
        }
    );
    // The new track is selected, the clip too, and the shelf stays open for the next one.
    assert_eq!(view.song.play_target, Some(kit.id));
    let clip = &clips(m, 1)[0];
    assert_eq!((clip.start, clip.end), (BAR, 2 * BAR));
    assert_eq!(notes_of(m, clip), expected("eightBeat", BAR));
    assert_eq!(view.arrange.selected_clips, vec![clip.id]);
    assert!(view.workspace.layout.right_open.contains(&Band::Shelf));
    assert_eq!(view.transport.position, 2 * BAR);

    // Enter again: on the kit track, after the first one.
    place(m, "tomFill");
    assert_eq!(steps(m), 2);
    assert_eq!(m.view().song.tracks.len(), 2);
    let second = &clips(m, 1)[1];
    assert_eq!((second.start, second.end), (2 * BAR, 3 * BAR));
    assert_eq!(notes_of(m, second), expected("tomFill", 2 * BAR));
    assert_eq!(m.view().transport.position, 3 * BAR);

    // One undo each: the second clip, then the track with the first.
    cmd(m, Command::Undo);
    assert_eq!(clips(m, 1).len(), 1);
    cmd(m, Command::Undo);
    assert_eq!(m.view().song.tracks.len(), 1);
    assert!(clips(m, 0).is_empty());
    cmd(m, Command::Redo);
    assert_eq!(m.view().song.tracks.len(), 2);
    assert_eq!(notes_of(m, &clips(m, 1)[0]), expected("eightBeat", BAR));
}

#[test]
fn where_it_would_overlap_a_clip_nothing_is_placed() {
    let m = &mut shelf();
    place(m, "eightBeat");
    cmd(m, Command::Locate { tick: BAR / 2 });
    let effects = place(m, "sixteenBeat");
    assert_eq!(status(m), Some(StatusMessage::NotesNotWritten));
    assert_eq!(steps(m), 1);
    assert_eq!(clips(m, 1).len(), 1);
    assert!(ops(&effects).is_empty());
    // Right before a clip it fits.
    let kit = m.view().song.tracks[1].id;
    cmd(
        m,
        Command::CreateClip {
            track: kit,
            tick: 3 * BAR,
        },
    );
    cmd(m, Command::Locate { tick: 2 * BAR });
    let n = steps(m);
    place(m, "sixteenBeat");
    assert_eq!(steps(m), n + 1);
    let placed: Vec<_> = clips(m, 1).iter().map(|c| (c.start, c.end)).collect();
    assert_eq!(placed, [(0, BAR), (2 * BAR, 3 * BAR), (3 * BAR, 7 * BAR)]);
    // Inside that clip it does not.
    cmd(m, Command::Locate { tick: 5 * BAR });
    place(m, "sixteenBeat");
    assert_eq!(steps(m), n + 1);
    assert_eq!(status(m), Some(StatusMessage::NotesNotWritten));
}

#[test]
fn only_where_the_meter_is_four_four() {
    let m = &mut shelf();
    cmd(
        m,
        Command::AddMeterChange {
            bar: 2,
            num: 3,
            den: 4,
        },
    );
    let n = steps(m);
    cmd(m, Command::Locate { tick: 2 * BAR + 10 });
    place(m, "eightBeat");
    assert_eq!(status(m), Some(StatusMessage::PatternNeedsFourFour));
    assert_eq!((steps(m), m.view().song.tracks.len()), (n, 1));
    // Bar 2 is 4/4.
    cmd(m, Command::Locate { tick: BAR });
    place(m, "eightBeat");
    assert_eq!(status(m), None);
    assert_eq!(steps(m), n + 1);
}

#[test]
fn nothing_while_counting_in_and_an_old_id_does_nothing() {
    let m = &mut shelf();
    place(m, "nope");
    assert_eq!(steps(m), 0);
    cmd(m, Command::SetCountIn { bars: 1 });
    cmd(m, Command::Record);
    assert_eq!(
        m.view().transport.state,
        meloet_protocol::TransportState::CountIn
    );
    place(m, "eightBeat");
    assert_eq!(steps(m), 0);
}

#[test]
fn on_a_drum_track_it_goes_on_that_track() {
    let m = &mut shelf();
    place(m, "eightBeat");
    let kit = m.view().song.tracks[1].id;
    let select = |m: &mut Model, track: u32| {
        cmd(
            m,
            Command::SelectTrack {
                track,
                from_arm: false,
            },
        )
    };
    select(m, 1);
    cmd(m, Command::Locate { tick: 4 * BAR });
    place(m, "halfTime");
    let tracks = m.view().song.tracks;
    assert_eq!(tracks.len(), 3, "the piano is no drum track");
    assert_eq!(tracks[2].id, kit, "the new one is right below the piano");
    select(m, kit);
    cmd(m, Command::Locate { tick: 6 * BAR });
    place(m, "halfTime");
    assert_eq!(m.view().song.tracks.len(), 3);
    let on_kit: Vec<u64> = clips(m, 2).iter().map(|c| c.start).collect();
    assert_eq!(on_kit, [0, 6 * BAR]);
}

#[test]
fn the_pattern_is_heard_once_at_the_song_tempo_with_the_kit_of_the_track() {
    let m = &mut shelf();
    m.handle(Input::AudioFrames(Some(256)));
    cmd(m, Command::SetTempo { bpm: 60.0 });
    m.set_now(1_000_000, None);
    let played = |effects: &[Effect]| {
        ops(effects).into_iter().find_map(|op| match op {
            EngineOp::PlayPattern { instrument, hits } => Some((instrument, hits)),
            _ => None,
        })
    };
    // The kit is read first.
    assert!(
        played(&cmd(
            m,
            Command::PlayPattern {
                id: "eightBeat".into()
            }
        ))
        .is_none()
    );
    let waiting = m.view().shelf.pattern_sample.unwrap();
    assert_eq!((&waiting.id[..], waiting.playing), ("eightBeat", None));
    let (kit, hits) = played(&m.handle(Input::SoundLoaded(B::StudioKit))).unwrap();
    assert_eq!(
        kit,
        InstrumentRef::Builtin(B::StudioKit),
        "the piano is no kit"
    );
    let p = patterns::find("eightBeat").unwrap();
    assert_eq!(hits.len(), p.notes.len());
    // At 60 a quarter is a second: the snare on beat 2 is at 1 s.
    let snare = hits.iter().find(|h| h.2 == 38).unwrap();
    assert!((snare.0 - 1.0).abs() < 1e-9 && (snare.1 - 0.125).abs() < 1e-9);
    assert_eq!(snare.3, 110);

    let view = m.view().shelf;
    let sample = view.pattern_sample.unwrap();
    assert_eq!(sample.id, "eightBeat");
    let playing = sample.playing.unwrap();
    assert_eq!(playing.length_us, 4_000_000);
    assert_eq!(view.sound_sample, None);
    assert!(
        !matches!(status(m), Some(StatusMessage::SamplePlaying { .. })),
        "no status of a sound's sample"
    );
    assert_eq!(steps(m), 1, "only the tempo");

    // It ends by itself after the bar and the tail.
    m.set_now(playing.started_us + 5_999_000, None);
    assert!(ops(&m.handle(Input::Tick)).is_empty());
    m.set_now(playing.started_us + 6_000_000, None);
    assert_eq!(ops(&m.handle(Input::Tick)), [EngineOp::StopSample]);
    assert_eq!(m.view().shelf.pattern_sample, None);

    // On a drum track, with its kit; StopSample stops it.
    cmd(
        m,
        Command::ShelfSelectSound {
            sound: Some(SoundRef::Builtin {
                instrument: InstrumentKind::ElectroKit,
            }),
        },
    );
    m.handle(Input::SoundLoaded(B::ElectroKit));
    cmd(m, Command::ShelfConfirm);
    assert_eq!(m.view().song.tracks.len(), 2);
    let (kit, _) = played(&cmd(
        m,
        Command::PlayPattern {
            id: "shuffle".into(),
        },
    ))
    .unwrap();
    assert_eq!(kit, InstrumentRef::Builtin(B::ElectroKit));
    assert_eq!(ops(&cmd(m, Command::StopSample)), [EngineOp::StopSample]);
    assert_eq!(m.view().shelf.pattern_sample, None);

    // A sound's ▶ in place of the pattern: one sample at a time.
    cmd(
        m,
        Command::PlayPattern {
            id: "shuffle".into(),
        },
    );
    cmd(
        m,
        Command::PlaySample {
            instrument: InstrumentKind::GrandPiano,
        },
    );
    assert_eq!(m.view().shelf.pattern_sample, None);
    assert!(m.view().shelf.sound_sample.is_some());
}

#[test]
fn a_new_track_past_sixty_four_is_refused() {
    use std::sync::Arc;

    use meloet_model::{Track, TrackColor, TrackId};

    let m = &mut shelf();
    let mut song = crate::play::tests::new_song_without_pack();
    for id in 2..=crate::play::MAX_TRACKS as u32 {
        let track = Track {
            id: TrackId(id),
            name: format!("トラック {id}"),
            color: TrackColor::for_index(id as usize - 1),
            ..Track::clone(&song.doc.tracks[0])
        };
        song.doc.tracks.push(Arc::new(track));
    }
    cmd(m, Command::OpenSong);
    let path = std::path::PathBuf::from("/nowhere/曲/曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    });
    cmd(
        m,
        Command::OpenShelf {
            mode: ShelfMode::NewTrack,
        },
    );
    place(m, "eightBeat");
    assert_eq!(status(m), Some(StatusMessage::TooManyTracks));
    assert_eq!(m.view().song.tracks.len(), crate::play::MAX_TRACKS);
    assert_eq!(steps(m), 0);
}
