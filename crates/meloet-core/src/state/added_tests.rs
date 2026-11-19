//! 「あとから入れたもの」 through the rules: SF2 presets and plugins in the shelf, on tracks
//! and in insert slots, and what is said when one of them is not there.

use crate::seen::*;
use std::sync::Arc;

use meloet_model::{InstrumentRef, Kept, PluginRef, Sf2Ref, Song, edit, mixer};
use meloet_protocol::{
    Band, EffectKind, FileDropView, GmFamily, InsertEffectView, InsertRef, MissingRef, MissingView,
    PluginSlotRef, PluginState, SendBus, ShelfMode, SoundName, SoundRef, SoundView, StandIn,
    StripRef, TrackRef, TransportState,
};

use super::*;
use crate::catalog::tests::{COMP_LV2, DRUMS_LV2, SURGE_LV2, SURGE_VST3, scanned};
use crate::play::tests::{cmd, event, off, on, ready, report, started, status};
use crate::play::{AUDITION_SEAT, RecordReport, Spot, StripSeat};
use crate::sf2store::Sf2Preset;

pub(crate) const TINY: &str = "Tiny.sf2";

fn preset(bank: u16, program: u8, name: &str, keys: &[u8]) -> Sf2Preset {
    Sf2Preset {
        bank,
        program,
        name: name.into(),
        keys: keys.to_vec(),
    }
}

/// An SF2 file as meloet's folder would have it: a piano, two guitars and a drum kit.
pub(crate) fn tiny() -> Sf2Info {
    Sf2Info {
        file: TINY.into(),
        sha256: "ab12".into(),
        presets: vec![
            preset(0, 0, "Piano 1", &[]),
            preset(0, 24, "Nylon Guitar", &[]),
            preset(8, 24, "Ukulele", &[]),
            preset(128, 0, "Standard", &[35, 36, 38, 42, 49, 81]),
        ],
    }
}

/// Ready to play, with the plugins of `catalog::tests::scanned` and the file of `tiny`.
pub(crate) fn rigged() -> Model {
    let mut m = ready()
        .with_sf2(vec![tiny()])
        .with_plugin_places(vec!["/p/lv2".into(), "/p/vst3".into()]);
    m.handle(Input::PluginsStart {
        cache: Box::new(scanned()),
        first: false,
    });
    m
}

pub(crate) fn plugin(key: &str) -> SoundRef {
    SoundRef::Plugin { key: key.into() }
}

pub(crate) fn sf2(bank: u16, program: u8) -> SoundRef {
    SoundRef::Sf2 {
        file: TINY.into(),
        bank,
        program,
    }
}

fn select(sound: SoundRef) -> Command {
    Command::ShelfSelectSound { sound: Some(sound) }
}

fn open(mode: ShelfMode) -> Command {
    Command::OpenShelf { mode }
}

fn ops(effects: &[Effect]) -> Vec<EngineOp> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(op) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

/// The loads asked for: ticket, key and state.
pub(crate) fn loads(effects: &[Effect]) -> Vec<(u64, String, PlugState)> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::LoadPlugin {
                ticket, key, state, ..
            } => Some((*ticket, key.to_string(), state.clone())),
            _ => None,
        })
        .collect()
}

fn one_load(effects: &[Effect]) -> u64 {
    let loads = loads(effects);
    assert_eq!(loads.len(), 1, "{effects:?}");
    loads[0].0
}

fn loaded(m: &mut Model, ticket: u64) -> Vec<Effect> {
    m.handle(Input::PluginLoaded { ticket })
}

fn track_names(m: &Model) -> Vec<String> {
    let tracks = m.view().song.tracks;
    tracks.into_iter().map(|t| t.name).collect()
}

fn plugin_state(m: &Model, track: usize) -> Option<PluginState> {
    match &m.view().song.tracks[track].sound {
        SoundView::Plugin(p) => Some(p.state),
        _ => None,
    }
}

/// A model whose first track sounds with Surge XT, loaded. Returns its ticket and its slot.
fn with_surge() -> (Model, u64, PluginSlotRef) {
    let mut m = rigged();
    let track = m.view().song.tracks[0].id;
    cmd(&mut m, open(ShelfMode::Replace { track }));
    let ticket = one_load(&cmd(&mut m, select(plugin(SURGE_VST3))));
    loaded(&mut m, ticket);
    cmd(&mut m, Command::ShelfConfirm);
    assert_eq!(plugin_state(&m, 0), Some(PluginState::Ready));
    (m, ticket, PluginSlotRef::Instrument { track })
}

#[test]
fn a_plugin_is_read_before_it_is_tried_and_enter_makes_its_track() {
    let mut m = rigged();
    cmd(&mut m, open(ShelfMode::NewTrack));
    let effects = cmd(&mut m, select(plugin(SURGE_VST3)));
    let ticket = one_load(&effects);
    assert_eq!(loads(&effects)[0].1, SURGE_VST3);
    assert_eq!(loads(&effects)[0].2, PlugState::Fresh);
    assert!(
        matches!(
            &ops(&effects)[..],
            [EngineOp::Instrument { seat: AUDITION_SEAT, instrument: InstrumentRef::Plugin(p) }, ..]
                if &*p.key == SURGE_VST3 && &*p.path == "/p/vst3/Surge XT.vst3"
        ),
        "the seat is silent until the plugin is there: {effects:?}"
    );
    let shelf = m.view().shelf;
    assert_eq!(shelf.selected_sound, Some(plugin(SURGE_VST3)));
    assert_eq!(shelf.loading_sound, Some(plugin(SURGE_VST3)));
    assert_eq!(shelf.selected(), None, "no sound of meloet");
    let name = "Surge XT".to_owned();
    assert_eq!(status(&m), Some(StatusMessage::PluginLoading { name }));
    assert!(
        cmd(&mut m, Command::ShelfConfirm).is_empty(),
        "no track for a plugin that may yet not load"
    );
    assert_eq!(m.view().song.tracks.len(), 1);

    let effects = loaded(&mut m, ticket);
    let spot = Spot::Seat(AUDITION_SEAT);
    assert_eq!(ops(&effects), [EngineOp::Plug { ticket, spot }]);
    assert_eq!(m.view().shelf.loading_sound, None);
    let sound = SoundName::Plugin {
        name: "Surge XT".into(),
    };
    assert_eq!(
        status(&m),
        Some(StatusMessage::AuditionNewTrackSound { sound })
    );

    let effects = cmd(&mut m, Command::ShelfConfirm);
    assert!(
        loads(&effects).is_empty(),
        "the one that was tried: {effects:?}"
    );
    let to = m.track_seats()[1].unwrap();
    let moved = EngineOp::MoveInstrument {
        from: AUDITION_SEAT,
        to,
    };
    assert!(ops(&effects).contains(&moved));
    assert_eq!(track_names(&m), ["ピアノ", "Surge XT"]);
    assert_eq!(plugin_state(&m, 1), Some(PluginState::Ready));
    assert_eq!(m.view().song.tracks[1].stand_in, None);
    assert_eq!(m.plugin_tickets().len(), 1);
    assert_eq!(m.plugin_tickets()[0].1, ticket);

    // A second one is "Surge XT 2" (spec 3 Y14), in the format the song uses already.
    cmd(&mut m, open(ShelfMode::NewTrack));
    assert_eq!(m.view().library.plugins[1].key, SURGE_VST3);
    let second = one_load(&cmd(&mut m, select(plugin(SURGE_VST3))));
    loaded(&mut m, second);
    cmd(&mut m, Command::ShelfConfirm);
    assert_eq!(track_names(&m), ["ピアノ", "Surge XT", "Surge XT 2"]);

    // Undo takes the track and its plugin away; redo loads it again as the song has it.
    let effects = cmd(&mut m, Command::Undo);
    assert!(matches!(
        &ops(&effects)[..],
        [EngineOp::ClearSeat { .. }, ..]
    ));
    assert_eq!(m.plugin_tickets().len(), 1);
    let again = one_load(&cmd(&mut m, Command::Redo));
    assert_eq!(plugin_state(&m, 2), Some(PluginState::Loading));
    loaded(&mut m, again);
    assert_eq!(plugin_state(&m, 2), Some(PluginState::Ready));
}

#[test]
fn a_plugin_that_does_not_load_makes_no_track_and_the_shelf_says_so() {
    let mut m = rigged();
    cmd(&mut m, open(ShelfMode::NewTrack));
    let ticket = one_load(&cmd(&mut m, select(plugin(SURGE_LV2))));
    let effects = m.handle(Input::PluginUnreadable { ticket });
    let cleared = EngineOp::ClearSeat {
        seat: AUDITION_SEAT,
    };
    assert!(ops(&effects).contains(&cleared));
    let shelf = m.view().shelf;
    assert_eq!(shelf.unreadable, Some(plugin(SURGE_LV2)));
    assert_eq!((shelf.selected_sound, shelf.loading_sound), (None, None));
    assert!(cmd(&mut m, Command::ShelfConfirm).is_empty());
    assert_eq!(m.view().song.tracks.len(), 1);
    // Until another row is selected.
    cmd(&mut m, select(sf2(0, 0)));
    assert_eq!(m.view().shelf.unreadable, None);

    // A load that answers after the row was left is let go of.
    let ticket = one_load(&cmd(&mut m, select(plugin(SURGE_LV2))));
    cmd(&mut m, Command::ShelfCancel);
    assert_eq!(loaded(&mut m, ticket), [Effect::DropPlugin { ticket }]);
}

#[test]
fn a_track_gets_a_plugin_in_place_of_its_sound_and_cancel_gives_it_back() {
    let mut m = rigged();
    let track = m.view().song.tracks[0].id;
    let seat = m.track_seats()[0].unwrap();
    cmd(&mut m, open(ShelfMode::Replace { track }));
    let effects = cmd(&mut m, select(plugin(SURGE_VST3)));
    let ticket = one_load(&effects);
    assert!(matches!(
        &ops(&effects)[..],
        [EngineOp::TryInstrument { seat: s, .. }, ..] if *s == seat
    ));
    let spot = Spot::Trial(seat);
    assert_eq!(
        ops(&loaded(&mut m, ticket)),
        [EngineOp::Plug { ticket, spot }]
    );
    assert!(matches!(
        status(&m),
        Some(StatusMessage::AuditionReplaceSound {
            sound: SoundName::Plugin { .. },
            ..
        })
    ));

    // Cancel: the plugin goes with the trial, and the track's own sound is made again.
    let effects = cmd(&mut m, Command::ShelfCancel);
    let own = m.play().song().doc.tracks[0].instrument.clone();
    assert_eq!(
        ops(&effects),
        [
            EngineOp::EndTrial { seat, keep: false },
            EngineOp::Instrument {
                seat,
                instrument: own
            }
        ]
    );
    assert!(m.plugin_tickets().is_empty());

    // Enter: it is the track's own, as one step; the name of the track stays.
    let ticket = one_load(&cmd(&mut m, select(plugin(SURGE_VST3))));
    loaded(&mut m, ticket);
    let effects = cmd(&mut m, Command::ShelfConfirm);
    assert!(ops(&effects).contains(&EngineOp::EndTrial { seat, keep: true }));
    assert!(loads(&effects).is_empty());
    assert_eq!(
        status(&m),
        Some(StatusMessage::InstrumentReplacedSound {
            track: TrackRef {
                number: 1,
                name: "ピアノ".into()
            },
            sound: SoundName::Plugin {
                name: "Surge XT".into()
            },
        })
    );
    assert_eq!(
        m.plugin_tickets(),
        [(PluginPlace::Instrument(TrackId(track)), ticket)]
    );
    assert_eq!(m.view().song.undo_count, 1);
    // Undo: the piano is back and the plugin is handed back by the engine.
    let effects = cmd(&mut m, Command::Undo);
    assert!(matches!(
        &ops(&effects)[..],
        [
            EngineOp::Instrument {
                instrument: InstrumentRef::Builtin(_),
                ..
            },
            ..
        ]
    ));
    assert!(m.plugin_tickets().is_empty());
}

use meloet_model::TrackId;

#[test]
fn a_plugin_effect_goes_into_a_slot_is_loaded_and_comes_out_again() {
    let mut m = rigged();
    let track = m.view().song.tracks[0].id;
    let seat = m.track_seats()[0].unwrap();
    let strip = StripRef::Track { id: track };
    let insert = InsertRef { strip, slot: 1 };
    cmd(&mut m, open(ShelfMode::InsertEffect { strip, slot: 1 }));
    let choose = |key: &str| Command::ShelfChooseEffect {
        effect: Some(EffectRef::Plugin { key: key.into() }),
    };
    // An instrument is no effect.
    cmd(&mut m, choose(SURGE_VST3));
    assert_eq!(m.view().shelf.chosen_effect, None);
    cmd(&mut m, choose(COMP_LV2));
    let chosen = EffectRef::Plugin {
        key: COMP_LV2.into(),
    };
    assert_eq!(m.view().shelf.chosen_effect, Some(chosen));
    assert_eq!(m.view().shelf.selected_effect(), None);

    let effects = cmd(&mut m, Command::ShelfConfirm);
    let ticket = one_load(&effects);
    assert!(
        ops(&effects).is_empty(),
        "the slot is empty until it is read"
    );
    let slot_view = |m: &Model| m.view().song.tracks[0].slots[1].clone().unwrap();
    let state = |m: &Model| match slot_view(m).effect {
        InsertEffectView::Plugin(p) => (p.name, p.state),
        other => panic!("{other:?}"),
    };
    assert_eq!(state(&m), ("ZamComp".into(), PluginState::Loading));
    assert_eq!(
        m.view().song.tracks[0].inserts()[1],
        None,
        "no effect of meloet"
    );
    assert_eq!(m.view().song.undo_count, 1);
    let spot = Spot::Insert(StripSeat::Track(seat), 1);
    assert_eq!(
        ops(&loaded(&mut m, ticket)),
        [EngineOp::Plug { ticket, spot }]
    );
    assert_eq!(state(&m).1, PluginState::Ready);
    let place = PluginPlace::Insert(StripId::Track(TrackId(track)), 1);
    assert_eq!(m.plugin_tickets(), [(place, ticket)]);
    // A fader moves nothing of it.
    let fader = Command::SetGain {
        strip,
        db: Some(-6.0),
        commit: true,
    };
    assert!(loads(&cmd(&mut m, fader)).is_empty());

    // "外す": the engine is told to empty the slot, and hands the plugin back.
    let effects = cmd(&mut m, Command::RemoveEffect { insert });
    let emptied = EngineOp::Insert {
        strip: StripSeat::Track(seat),
        slot: 1,
        insert: None,
    };
    assert_eq!(ops(&effects), std::slice::from_ref(&emptied));
    assert!(m.plugin_tickets().is_empty());
    // Undo loads it again; undo of the fader and of the insert takes it out again.
    let again = one_load(&cmd(&mut m, Command::Undo));
    assert_ne!(again, ticket);
    loaded(&mut m, again);
    cmd(&mut m, Command::Undo);
    assert_eq!(ops(&cmd(&mut m, Command::Undo)), [emptied]);

    // A plugin can go onto a return and the master too.
    for strip in [StripRef::Master, StripRef::Return { bus: SendBus::B }] {
        cmd(&mut m, open(ShelfMode::InsertEffect { strip, slot: 3 }));
        cmd(&mut m, choose(COMP_LV2));
        let ticket = one_load(&cmd(&mut m, Command::ShelfConfirm));
        assert!(matches!(
            ops(&loaded(&mut m, ticket))[..],
            [EngineOp::Plug {
                spot: Spot::Insert(StripSeat::Master | StripSeat::Return(_), 3),
                ..
            }]
        ));
    }
    assert_eq!(m.plugin_tickets().len(), 2);
}

#[test]
fn a_plugin_that_stops_does_not_stop_the_recording() {
    let (mut m, ticket, slot) = with_surge();
    let stopped = StatusMessage::PluginStopped {
        name: "Surge XT".into(),
        slot,
    };
    assert!(
        cmd(&mut m, Command::ReloadPlugin { slot }).is_empty(),
        "nothing stopped"
    );
    assert!(cmd(&mut m, Command::RemovePlugin { slot }).is_empty());

    cmd(&mut m, Command::SetCountIn { bars: 0 });
    cmd(&mut m, Command::Record);
    started(&mut m);
    event(&mut m, on(60), 10, 0.0, 0);
    let effects = m.handle(Input::PluginStopped { ticket });
    assert!(
        effects.is_empty(),
        "nothing is told to the engine: {effects:?}"
    );
    assert!(m.view().transport.recording.is_some());
    assert_eq!(status(&m), Some(stopped.clone()), "said while recording");
    assert_eq!(plugin_state(&m, 0), Some(PluginState::Stopped));
    // An instrument is not changed under a recording.
    assert!(cmd(&mut m, Command::RemovePlugin { slot }).is_empty());

    event(&mut m, off(60), 20, 480.0, 0);
    let heard = Heard {
        tick: 960.0,
        state: TransportState::Recording,
        lap: 0,
    };
    m.set_now(100, Some(heard));
    cmd(&mut m, Command::Record);
    report(
        &mut m,
        RecordReport::Stopped {
            t_us: 100,
            tick: 960.0,
            lap: 0,
        },
    );
    let view = m.view();
    assert!(view.transport.recording.is_none());
    assert_eq!(view.song.undo_count, 2, "what was recorded is a take");
    assert_eq!(view.status.message, Some(stopped.clone()));

    // "もう一度読みこむ": a new process, the same ticket. Until it answers it is as it was.
    let reload = Effect::ReloadPlugin { ticket };
    assert_eq!(cmd(&mut m, Command::ReloadPlugin { slot }), [reload]);
    assert_eq!(status(&m), Some(stopped.clone()));
    assert!(cmd(&mut m, Command::ReloadPlugin { slot }).is_empty());
    m.handle(Input::PluginUnreadable { ticket });
    assert_eq!(status(&m), Some(stopped), "it did not come back");

    // "外す": the track sounds as a sine, its take stays, and it is one step.
    let effects = cmd(&mut m, Command::RemovePlugin { slot });
    assert!(matches!(
        &ops(&effects)[..],
        [
            EngineOp::Instrument {
                instrument: InstrumentRef::Builtin(_),
                ..
            },
            ..
        ]
    ));
    let view = m.view();
    assert_eq!(view.song.tracks[0].instrument(), InstrumentKind::Sine);
    assert_eq!(view.song.tracks[0].clips.len(), 1);
    assert_eq!(view.song.undo_count, 3);
    assert!(!matches!(
        view.status.message,
        Some(StatusMessage::PluginStopped { .. })
    ));
    assert!(cmd(&mut m, Command::RemovePlugin { slot }).is_empty());
    // Undo brings the plugin back, loaded anew.
    let again = one_load(&cmd(&mut m, Command::Undo));
    assert_ne!(again, ticket);
    assert_eq!(m.view().song.undo_count, 2);
}

#[test]
fn a_plugin_loaded_again_is_stopped_no_more() {
    let (mut m, ticket, slot) = with_surge();
    let seat = m.track_seats()[0].unwrap();
    m.handle(Input::PluginStopped { ticket });
    assert!(matches!(
        status(&m),
        Some(StatusMessage::PluginStopped { .. })
    ));
    cmd(&mut m, Command::ReloadPlugin { slot });
    assert_eq!(plugin_state(&m, 0), Some(PluginState::Loading));
    let effects = loaded(&mut m, ticket);
    let spot = Spot::Seat(seat);
    assert_eq!(ops(&effects), [EngineOp::Plug { ticket, spot }]);
    assert_eq!(plugin_state(&m, 0), Some(PluginState::Ready));
    assert!(!matches!(
        status(&m),
        Some(StatusMessage::PluginStopped { .. })
    ));
    assert!(cmd(&mut m, Command::ReloadPlugin { slot }).is_empty());
}

fn song_with_missing() -> Song {
    let mut song = Song::new_default();
    let surge = PluginRef::new(
        SURGE_VST3,
        "Surge XT",
        "Surge Synth Team",
        "/old/Surge XT.vst3",
    )
    .with_state(Arc::from([1u8, 2, 3]));
    let comp = PluginRef::new(COMP_LV2, "ZamComp", "Damien Zammit", "/old/ZamComp.lv2");
    let kit = Sf2Ref {
        file: TINY.into(),
        sha256: "".into(),
        bank: 128,
        program: 0,
        name: "Standard".into(),
        kept: Kept::NONE,
    };
    let add = |song: &mut Song, instrument| {
        let (e, id) = edit::add_track(&song.doc, None, instrument).unwrap();
        song.doc = e.doc;
        id
    };
    let surge_track = add(&mut song, InstrumentRef::Plugin(Arc::new(surge)));
    add(&mut song, InstrumentRef::Sf2(Arc::new(kit)));
    let strip = StripId::Track(surge_track);
    song.doc = mixer::add_plugin_effect(&song.doc, strip, 0, Arc::new(comp))
        .unwrap()
        .doc;
    song
}

fn opened(m: &mut Model, song: Song) -> Vec<Effect> {
    cmd(m, Command::OpenSong);
    let path = PathBuf::from("/nowhere/曲/曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    })
}

#[test]
fn what_this_pc_does_not_have_sounds_as_the_sine_wave_until_it_is_there() {
    // No pack, no SF2 file, no plugin.
    let mut m = ready();
    let effects = opened(&mut m, song_with_missing());
    assert!(loads(&effects).is_empty());
    let instruments: Vec<InstrumentRef> = (ops(&effects).into_iter())
        .filter_map(|op| match op {
            EngineOp::Instrument { instrument, .. } => Some(instrument),
            _ => None,
        })
        .collect();
    assert_eq!(
        instruments[1],
        InstrumentRef::SINE,
        "in place of the plugin"
    );
    assert!(matches!(instruments[2], InstrumentRef::Sf2(_)));
    let view = m.view();
    let stand_ins: Vec<Option<StandIn>> = view.song.tracks.iter().map(|t| t.stand_in).collect();
    assert_eq!(
        stand_ins,
        [
            Some(StandIn::PackMissing),
            Some(StandIn::PluginMissing),
            Some(StandIn::Sf2Missing)
        ]
    );
    assert_eq!(plugin_state(&m, 1), Some(PluginState::Missing));
    assert_eq!(view.song.tracks[2].drum_keys, None);
    assert!(view.library.pack_missing);
    assert_eq!(view.shelf.available, [InstrumentKind::Sine]);
    // The pack comes first: the first track of the song is the grand piano.
    assert_eq!(
        view.status.message,
        Some(StatusMessage::SoundsMissing { tracks: 1 })
    );
    assert!(matches!(
        &view.shelf.missing[..],
        [
            MissingView::Sf2 { file, tracks: 1 },
            MissingView::Plugin { key: a, shown_path: Some(path), .. },
            MissingView::Plugin { key: b, .. },
        ] if file == TINY && a == SURGE_VST3 && path == "/old/Surge XT.vst3" && b == COMP_LV2
    ));

    // The scan finds the plugins: they are loaded, with the state the song has.
    let effects = m.handle(Input::PluginsStart {
        cache: Box::new(scanned()),
        first: false,
    });
    let asked = loads(&effects);
    assert_eq!(asked.len(), 2, "{effects:?}");
    let surge = asked.iter().find(|l| l.1 == SURGE_VST3).unwrap();
    assert_eq!(surge.2, PlugState::Bytes(Arc::from([1u8, 2, 3])));
    assert_eq!(plugin_state(&m, 1), Some(PluginState::Loading));
    for (ticket, ..) in &asked {
        loaded(&mut m, *ticket);
    }
    assert_eq!(plugin_state(&m, 1), Some(PluginState::Ready));
    assert_eq!(m.view().song.tracks[1].stand_in, None);
    assert_eq!(m.view().shelf.missing.len(), 1);

    // An SF2 file of that name is added: the track is tied to it.
    let effects = m.handle(Input::Sf2Added(Box::new(tiny())));
    let key = Sf2Key {
        file: TINY.into(),
        bank: 128,
        program: 0,
    };
    assert!(
        effects.contains(&Effect::LoadSf2(key.clone())),
        "{effects:?}"
    );
    let view = m.view();
    assert_eq!(view.song.tracks[2].stand_in, None);
    assert_eq!(
        view.song.tracks[2].drum_keys,
        Some(vec![49, 42, 38, 36, 35, 81]),
        "the rows of the kit, top first"
    );
    assert!(view.shelf.missing.is_empty());
    let effects = m.handle(Input::Sf2Loaded(key));
    assert!(matches!(
        &ops(&effects)[..],
        [EngineOp::Instrument {
            instrument: InstrumentRef::Sf2(_),
            ..
        }]
    ));
}

#[test]
fn an_opened_song_says_which_sf2_file_is_not_found_and_it_can_be_taken_out() {
    let mut m = ready().with_sounds(BuiltinInstrument::all().collect());
    m.handle(Input::SoundLoaded(BuiltinInstrument::GrandPiano));
    opened(&mut m, song_with_missing());
    assert_eq!(
        status(&m),
        Some(StatusMessage::Sf2Missing {
            file: TINY.into(),
            tracks: 1
        })
    );
    assert_eq!(m.view().shelf.available.len(), 31);

    let remove = |m: &mut Model, missing| cmd(m, Command::RemoveMissing { missing });
    let file = MissingRef::Sf2 { file: TINY.into() };
    let effects = remove(&mut m, file.clone());
    assert!(matches!(
        &ops(&effects)[..],
        [
            EngineOp::Instrument {
                instrument: InstrumentRef::Builtin(_),
                ..
            },
            ..
        ]
    ));
    let view = m.view();
    assert_eq!(view.song.tracks[2].instrument(), InstrumentKind::Sine);
    assert_eq!(view.song.tracks[2].name, "ドラム", "the name stays");
    assert_eq!((view.song.undo_count, view.shelf.missing.len()), (1, 2));
    assert!(remove(&mut m, file).is_empty(), "not in the song any more");

    // A plugin: its track gets the sine wave and its slots are emptied, as one step.
    let comp = MissingRef::Plugin {
        key: COMP_LV2.into(),
    };
    remove(&mut m, comp);
    let view = m.view();
    assert_eq!(view.song.tracks[1].slots[0], None);
    assert!(matches!(view.song.tracks[1].sound, SoundView::Plugin(_)));
    assert_eq!((view.song.undo_count, view.shelf.missing.len()), (2, 1));
    cmd(&mut m, Command::Undo);
    cmd(&mut m, Command::Undo);
    assert_eq!(m.view().shelf.missing.len(), 3);
    assert_eq!(m.play().song().doc, {
        let mut doc = song_with_missing().doc;
        doc.next_id = m.play().song().doc.next_id;
        doc
    });
}

#[test]
fn an_sf2_preset_is_tried_added_as_a_track_and_its_sample_waits_for_its_samples() {
    let mut m = rigged();
    let view = m.view();
    assert_eq!(view.library.sf2.len(), 1);
    let families: Vec<GmFamily> = (view.library.sf2[0].presets.iter())
        .map(|p| p.family)
        .collect();
    assert_eq!(
        families,
        [
            GmFamily::Piano,
            GmFamily::Guitar,
            GmFamily::Guitar,
            GmFamily::DrumKit
        ]
    );
    assert_eq!(view.library.opened_sf2, None);

    cmd(&mut m, open(ShelfMode::NewTrack));
    let key = |bank, program| Sf2Key {
        file: TINY.into(),
        bank,
        program,
    };
    let effects = cmd(&mut m, select(sf2(8, 24)));
    assert!(effects.contains(&Effect::LoadSf2(key(8, 24))));
    assert_eq!(m.view().shelf.loading_sound, Some(sf2(8, 24)));
    let sound = SoundName::Sf2 {
        name: "Ukulele".into(),
        file: TINY.into(),
    };
    assert_eq!(
        status(&m),
        Some(StatusMessage::AuditionNewTrackSound {
            sound: sound.clone()
        })
    );
    let effects = m.handle(Input::Sf2Loaded(key(8, 24)));
    assert!(matches!(
        &ops(&effects)[..],
        [EngineOp::Instrument { seat: AUDITION_SEAT, instrument: InstrumentRef::Sf2(s) }]
            if &*s.sha256 == "ab12" && &*s.name == "Ukulele"
    ));
    assert_eq!(m.view().shelf.loading_sound, None);
    cmd(&mut m, Command::ShelfConfirm);
    // The short name of the family (spec 3), and the drum kit with its rows.
    cmd(&mut m, open(ShelfMode::NewTrack));
    cmd(&mut m, select(sf2(128, 0)));
    cmd(&mut m, Command::ShelfConfirm);
    assert_eq!(track_names(&m), ["ピアノ", "ギター", "ドラム"]);
    let view = m.view();
    assert_eq!(view.song.tracks[1].drum_keys, None);
    assert_eq!(
        view.song.tracks[2].drum_keys,
        Some(vec![49, 42, 38, 36, 35, 81])
    );
    assert!(matches!(
        &view.song.tracks[2].sound,
        SoundView::Sf2 {
            bank: 128,
            family: GmFamily::DrumKit,
            ..
        }
    ));
    // The keys play the kit, which is still being read.
    m.handle(Input::KeyWhileLoading);
    assert!(matches!(
        status(&m),
        Some(StatusMessage::SoundLoading {
            sound: SoundName::Sf2 { .. }
        })
    ));
    m.handle(Input::Sf2Loaded(key(128, 0)));

    // ▶ waits for the samples, then plays the phrase of the nearest group.
    cmd(&mut m, open(ShelfMode::NewTrack));
    let play = |m: &mut Model, sound| cmd(m, Command::PlaySampleOf { sound });
    let effects = play(&mut m, sf2(0, 0));
    assert!(effects.contains(&Effect::LoadSf2(key(0, 0))));
    let sample = m.view().shelf.sound_sample.unwrap();
    assert_eq!((sample.sound, sample.playing), (sf2(0, 0), None));
    let effects = m.handle(Input::Sf2Loaded(key(0, 0)));
    assert!(matches!(
        &ops(&effects)[..],
        [EngineOp::PlaySample {
            instrument: InstrumentRef::Sf2(_)
        }]
    ));
    assert!(m.view().shelf.sound_sample.unwrap().playing.is_some());
    assert_eq!(m.view().shelf.sample(), None, "no sound of meloet");
    assert!(matches!(
        status(&m),
        Some(StatusMessage::SamplePlayingSound {
            sound: SoundName::Sf2 { .. }
        })
    ));
    // A preset that does not read leaves the shelf.
    cmd(&mut m, Command::StopSample);
    cmd(&mut m, select(sf2(0, 24)));
    m.handle(Input::Sf2Unreadable(key(0, 24)));
    assert_eq!(m.view().shelf.selected_sound, None);
    assert_eq!(m.view().library.sf2[0].presets.len(), 3);
}

#[test]
fn the_sample_of_a_plugin_is_an_instance_of_its_own_with_the_sound_that_is_tried() {
    let mut m = rigged();
    cmd(&mut m, open(ShelfMode::NewTrack));
    let play = |m: &mut Model, key: &str| cmd(m, Command::PlaySampleOf { sound: plugin(key) });
    let effects = play(&mut m, DRUMS_LV2);
    let ticket = one_load(&effects);
    assert_eq!(loads(&effects)[0].2, PlugState::Fresh);
    let sample = m.view().shelf.sound_sample.unwrap();
    assert_eq!((sample.sound, sample.playing), (plugin(DRUMS_LV2), None));
    let group = meloet_model::InstrumentGroup::Drums;
    assert_eq!(
        ops(&loaded(&mut m, ticket)),
        [EngineOp::PlugSample { ticket, group }]
    );
    assert!(m.view().shelf.sound_sample.unwrap().playing.is_some());
    assert!(m.sample_playing());

    // The row that is tried plays with the sound it has now.
    let tried = one_load(&cmd(&mut m, select(plugin(SURGE_VST3))));
    loaded(&mut m, tried);
    let effects = play(&mut m, SURGE_VST3);
    assert_eq!(loads(&effects)[0].2, PlugState::Of(tried));
    assert!(ops(&effects).contains(&EngineOp::StopSample));
    // Stopped before it is loaded: the load is let go of when it answers.
    let waiting = loads(&effects)[0].0;
    cmd(&mut m, Command::StopSample);
    assert_eq!(m.view().shelf.sound_sample, None);
    assert_eq!(
        loaded(&mut m, waiting),
        [Effect::DropPlugin { ticket: waiting }]
    );
}

#[test]
fn an_sf2_file_is_added_from_the_window_or_by_dropping_it() {
    let mut m = rigged();
    assert_eq!(cmd(&mut m, Command::AddSf2), [Effect::PickSf2]);
    assert!(m.handle(Input::Sf2Picked(None)).is_empty());
    let path = PathBuf::from("/home/me/FluidR3_GM.sf2");
    let add = Effect::AddSf2(path.clone());
    assert_eq!(
        m.handle(Input::Sf2Picked(Some(path.clone()))),
        std::slice::from_ref(&add)
    );

    let mut info = tiny();
    info.file = "FluidR3_GM.sf2".into();
    m.handle(Input::Sf2Added(Box::new(info)));
    let view = m.view();
    assert_eq!(
        view.status.message,
        Some(StatusMessage::Sf2Added {
            file: "FluidR3_GM.sf2".into(),
            presets: 3,
            kits: 1
        })
    );
    assert!(view.workspace.layout.right_open.contains(&Band::Shelf));
    assert_eq!(view.shelf.mode, ShelfMode::NewTrack);
    assert_eq!(view.library.opened_sf2.as_deref(), Some("FluidR3_GM.sf2"));
    let files: Vec<&str> = view.library.sf2.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(
        files,
        ["FluidR3_GM.sf2", TINY],
        "in the order of their names"
    );

    let file = "Broken.sf2".to_owned();
    m.handle(Input::Sf2NotAdded { file: file.clone() });
    assert_eq!(status(&m), Some(StatusMessage::Sf2Unreadable { file }));

    // Held over the window: the first file is named, and says whether it can be added.
    let song = PathBuf::from("/home/me/曲.meloet");
    m.handle(Input::FilesOver(vec![path.clone(), song.clone()]));
    let held = |name: &str, addable| {
        Some(FileDropView {
            name: name.into(),
            addable,
        })
    };
    assert_eq!(m.view().workspace.file_drop, held("FluidR3_GM.sf2", true));
    assert_eq!(status(&m), Some(StatusMessage::Sf2Dragging));
    assert_eq!(
        m.handle(Input::FilesDropped(vec![path.clone(), song.clone()])),
        std::slice::from_ref(&add)
    );
    assert_eq!(m.view().workspace.file_drop, None);

    m.handle(Input::FilesOver(vec![song.clone(), path.clone()]));
    assert_eq!(m.view().workspace.file_drop, held("曲.meloet", false));
    assert_ne!(status(&m), Some(StatusMessage::Sf2Dragging));
    assert!(
        m.handle(Input::FilesDropped(vec![song, path.clone()]))
            .is_empty()
    );

    // Taken out of the window, or Esc: nothing is added.
    m.handle(Input::FilesOver(vec![path.clone()]));
    m.handle(Input::FilesLeft);
    assert_eq!(m.view().workspace.file_drop, None);
    m.handle(Input::FilesOver(vec![path.clone()]));
    cmd(&mut m, Command::CancelFileDrop);
    assert_eq!(m.view().workspace.file_drop, None);
    assert!(m.handle(Input::FilesDropped(vec![path.clone()])).is_empty());
    m.handle(Input::FilesOver(vec![path.clone()]));
    assert_eq!(m.handle(Input::FilesDropped(vec![path])), [add]);
}

#[test]
fn stars_and_the_format_of_a_row_are_settings() {
    let mut m = rigged();
    let saved = |effects: &[Effect]| {
        effects.iter().find_map(|e| match e {
            Effect::SaveSettings(s) => Some(s.shelf.clone()),
            _ => None,
        })
    };
    let star = |m: &mut Model, sound| cmd(m, Command::ToggleFavoriteSound { sound });
    let shelf = saved(&star(&mut m, sf2(8, 24))).unwrap();
    assert_eq!(shelf.favorite_sounds, [sf2(8, 24)]);
    star(&mut m, plugin(SURGE_LV2));
    let stars = m.view().shelf.favorite_sounds;
    assert_eq!(stars[2..], [sf2(8, 24), plugin(SURGE_VST3)]);
    assert!(saved(&star(&mut m, sf2(9, 9))).is_none(), "no such preset");

    let chorus = EffectRef::Builtin {
        effect: EffectKind::Chorus,
    };
    let effect = chorus.clone();
    let shelf = saved(&cmd(&mut m, Command::ToggleFavoriteEffect { effect })).unwrap();
    assert_eq!(shelf.favorite_effects.len(), 3);
    assert!(m.view().shelf.favorite_effects.contains(&chorus));

    let key = SURGE_LV2.to_owned();
    let shelf = saved(&cmd(&mut m, Command::UsePluginFormat { key })).unwrap();
    assert_eq!(shelf.plugin_formats, [SURGE_LV2]);
    assert_eq!(m.view().library.plugins[1].key, SURGE_LV2);
    assert_eq!(m.view().shelf.favorite_sounds[3], plugin(SURGE_LV2));

    // What was saved is there at the next start.
    let again = Model::new(Settings {
        shelf,
        ..Settings::default()
    })
    .with_sf2(vec![tiny()]);
    assert_eq!(again.view().shelf.favorite_sounds[2], sf2(8, 24));
}

#[test]
fn what_changes_while_a_save_goes_on_is_not_saved() {
    let (mut m, ticket, _) = with_surge();
    let effects = cmd(&mut m, Command::SaveSong);
    assert!(matches!(&effects[..], [Effect::SaveSong { .. }]));
    assert_eq!(m.plugin_tickets().len(), 1);
    assert_eq!(m.plugin_tickets()[0].1, ticket);
    // The plugins are asked for their states meanwhile; the song goes on changing.
    cmd(&mut m, Command::SetTempo { bpm: 90.0 });
    m.handle(Input::SongSaved {
        path: "/songs/曲/曲.meloet".into(),
        name: "曲".into(),
    });
    assert_eq!(m.view().song.file, SongFile::Changed);
    assert!(m.recovery_waiting());
    cmd(&mut m, Command::Undo);
    assert_eq!(m.view().song.file, SongFile::Saved);
}

#[test]
fn plugins_are_loaded_again_for_another_sample_rate_and_for_a_new_engine() {
    let (mut m, ticket, _) = with_surge();
    let effects = m.handle(Input::AudioOpened {
        node_name: "alsa_output.card.analog-stereo".into(),
        sample_rate: 44_100,
    });
    let again = loads(&effects);
    assert_eq!(again.len(), 1);
    assert_eq!(
        again[0].2,
        PlugState::Of(ticket),
        "with the sound it has now"
    );
    assert!(matches!(
        effects
            .iter()
            .find(|e| matches!(e, Effect::LoadPlugin { .. })),
        Some(Effect::LoadPlugin {
            sample_rate: 44_100,
            ..
        })
    ));
    assert_eq!(plugin_state(&m, 0), Some(PluginState::Loading));
    loaded(&mut m, again[0].0);
    assert_eq!(plugin_state(&m, 0), Some(PluginState::Ready));

    let effects = m.handle(Input::PluginsLost);
    let lost = loads(&effects);
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].2, PlugState::Fresh, "as the song has it");
}

#[test]
fn the_rows_of_the_shelf_follow_a_scan_now_and_then_and_not_bundle_by_bundle() {
    use crate::plugins::ScanNews;
    let mut m = ready().with_plugin_places(vec!["/p/lv2".into(), "/p/vst3".into()]);
    let effects = m.handle(Input::PluginsStart {
        cache: Box::default(),
        first: true,
    });
    let job = (effects.iter())
        .find_map(|e| match e {
            Effect::ScanPlugins(job) => Some(job.id),
            _ => None,
        })
        .unwrap();
    let total = scanned().bundles.len() as u32;
    let gone = Vec::new();
    m.handle(Input::PluginScan(ScanNews::Counted { job, total, gone }));
    let rows = |m: &Model| m.view().library.plugins.len();
    // The record is written every two seconds while the scan runs; the shelf takes in what
    // was found at those moments, and when the scan is over.
    let mut shown = 0;
    for (n, record) in scanned().bundles.into_iter().enumerate() {
        m.set_now(1_000_000 * n as u64, None);
        let record = Box::new(record);
        let effects = m.handle(Input::PluginScan(ScanNews::Bundle { job, record }));
        let written = effects
            .iter()
            .any(|e| matches!(e, Effect::SavePluginScan(_)));
        assert_eq!(written, n == 2 || n == 4, "bundle {n}");
        assert_eq!(rows(&m) > shown, written, "after bundle {n}");
        shown = rows(&m);
    }
    m.handle(Input::PluginScan(ScanNews::Done { job }));
    assert_eq!(rows(&m), 4);
}
