use meloet_devices::{Availability, ClockSettings, DevicesPicture, Endpoint, LinkKind};
use meloet_midi::Keyboard;
use meloet_model::record::RecEventKind;
use meloet_protocol::{Command, SelectMode, StatusMessage, TakeNotes, TransportState};

use super::*;
use crate::settings::Settings;
use crate::state::{Effect, Input, Model};

const HP: &str = "card:analog-output-headphones";
const HP_NODE: &str = "alsa_output.card.analog-stereo";
const BAR: u64 = 1920;

fn headphones(availability: Availability) -> Endpoint {
    Endpoint {
        id: HP.into(),
        name: "アナログヘッドフォン".into(),
        friendly: None,
        availability,
        link: LinkKind::Plug,
        active: true,
        node_name: Some(HP_NODE.into()),
        device_name: None,
        device_description: Some("card device".into()),
    }
}

fn picture(availability: Availability) -> Input {
    Input::Devices(DevicesPicture {
        outputs: vec![headphones(availability)],
        inputs: vec![],
        clock: ClockSettings::default(),
    })
}

fn piano() -> TrackRef {
    TrackRef {
        number: 1,
        name: "ピアノ".into(),
    }
}

/// What a new song is on a PC without the sound pack, as the models of these tests are
/// unless they say otherwise: the first track is the sine wave.
pub(crate) fn new_song_without_pack() -> Song {
    let mut song = Song::new_default();
    let track = Arc::make_mut(&mut song.doc.tracks[0]);
    track.instrument = InstrumentRef::SINE;
    track.name = "シンセ".to_owned();
    song
}

/// The headphones chosen before, a keyboard, and the stream open: ready to play and record.
pub(crate) fn ready() -> Model {
    let mut m = Model::new(Settings {
        output: Some(HP.into()),
        output_name: Some("アナログヘッドフォン".into()),
        ..Settings::default()
    });
    m.handle(Input::KeyboardAdded(Keyboard {
        id: "kb1".into(),
        name: "Keystation 49".into(),
        port_name: "MIDI 1".into(),
        client: 24,
        port: 0,
    }));
    m.handle(picture(Availability::Yes));
    m.handle(Input::AudioOpened {
        node_name: HP_NODE.into(),
        sample_rate: 48_000,
    });
    m
}

pub(crate) fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn ops(effects: &[Effect]) -> Vec<EngineOp> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Engine(op) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

fn live(effects: &[Effect]) -> Vec<LiveNotes> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::LiveNotes(n) => Some(n.clone()),
            _ => None,
        })
        .collect()
}

pub(crate) fn hear(m: &mut Model, t_us: u64, tick: f64, state: TransportState, lap: u32) {
    m.set_now(t_us, Some(Heard { tick, state, lap }));
    m.handle(Input::Tick);
}

pub(crate) fn report(m: &mut Model, r: RecordReport) -> Vec<Effect> {
    m.handle(Input::Record(r))
}

pub(crate) fn on(key: u8) -> RecEventKind {
    RecEventKind::NoteOn { key, velocity: 100 }
}

pub(crate) fn off(key: u8) -> RecEventKind {
    RecEventKind::NoteOff { key }
}

pub(crate) fn event(
    m: &mut Model,
    kind: RecEventKind,
    t_us: u64,
    tick: f64,
    lap: u32,
) -> Vec<Effect> {
    report(
        m,
        RecordReport::Event {
            kind,
            t_us,
            tick,
            lap,
        },
    )
}

/// The engine took the record command, at the place core asked for.
pub(crate) fn started(m: &mut Model) -> Vec<Effect> {
    let from = m.view().transport.recording.unwrap().from;
    report(
        m,
        RecordReport::Started {
            from,
            held: HeldControls::default(),
        },
    )
}

pub(crate) fn status(m: &Model) -> Option<StatusMessage> {
    m.view().status.message
}

/// Records `notes` (key, start, end) from where the transport is, with no count-in, and stops
/// at `stop`.
pub(crate) fn record_once(m: &mut Model, notes: &[(u8, f64, f64)], stop: f64) -> Vec<Effect> {
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    for &(key, a, b) in notes {
        event(m, on(key), 10, a, 0);
        event(m, off(key), 20, b, 0);
    }
    m.set_now(
        100,
        Some(Heard {
            tick: stop,
            state: TransportState::Recording,
            lap: 0,
        }),
    );
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 100,
            tick: stop,
            lap: 0,
        },
    )
}

#[test]
fn play_and_stop_need_a_running_output() {
    let mut m = Model::new(Settings::default());
    assert!(ops(&cmd(&mut m, Command::PlayStop)).is_empty());
    assert!(ops(&cmd(&mut m, Command::Record)).is_empty());
    assert_eq!(
        status(&m),
        Some(StatusMessage::SearchingKeyboard),
        "no text"
    );
    let t = m.view().transport;
    assert!(!t.can_play && !t.can_record);

    let mut m = ready();
    let t = m.view().transport;
    assert!(t.can_play && t.can_record);
    assert_eq!(
        ops(&cmd(&mut m, Command::PlayStop)),
        [EngineOp::Play { from: 0 }]
    );
    assert_eq!(m.view().transport.state, TransportState::Playing);
    m.set_now(
        5_000,
        Some(Heard {
            tick: 1000.7,
            state: TransportState::Playing,
            lap: 0,
        }),
    );
    assert_eq!(
        ops(&cmd(&mut m, Command::PlayStop)),
        [EngineOp::Stop { t_us: 5_000 }]
    );
    let t = m.view().transport;
    assert_eq!((t.state, t.position), (TransportState::Stopped, 1000));
    // Play goes on from where it stopped.
    assert_eq!(
        ops(&cmd(&mut m, Command::PlayStop)),
        [EngineOp::Play { from: 1000 }]
    );
}

#[test]
fn the_stream_opening_gets_everything() {
    let m = &mut ready();
    cmd(m, Command::ToggleMetronome);
    let effects = m.handle(Input::AudioOpened {
        node_name: HP_NODE.into(),
        sample_rate: 48_000,
    });
    let o = ops(&effects);
    assert!(matches!(o[0], EngineOp::Stop { .. }));
    assert_eq!(o[1], EngineOp::Locate { tick: 0 });
    assert_eq!(
        o[2],
        EngineOp::Tempo {
            ticks_per_beat: 480,
            beats_per_bar: 4,
            bpm_centi: 9600
        }
    );
    assert_eq!(
        o[3],
        EngineOp::Loop {
            start: 0,
            end: 0,
            on: false
        }
    );
    assert_eq!(o[4], EngineOp::Metronome { on: true });
    assert_eq!(
        o[5],
        EngineOp::Sequence {
            track: 0,
            events: vec![]
        }
    );
    assert_eq!(o[6], EngineOp::SilentTracks { tracks: vec![] });
    assert_eq!(o[7], EngineOp::PlayTarget { seat: Some(0) });
    assert_eq!(o.len(), 8, "the instruments stay as they are in the engine");
}

#[test]
fn play_starts_at_the_loop_when_the_position_is_outside_it() {
    let m = &mut ready();
    cmd(
        m,
        Command::SetLoop {
            from_bar: 9,
            to_bar: 5,
        },
    );
    assert_eq!(
        m.view().song.loop_range,
        Some(TickSpan {
            start: 4 * BAR,
            end: 8 * BAR
        })
    );
    assert!(m.view().song.loop_on);
    assert_eq!(
        ops(&cmd(m, Command::PlayStop)),
        [EngineOp::Play { from: 4 * BAR }]
    );
    cmd(m, Command::PlayStop);
    cmd(
        m,
        Command::Locate {
            tick: 5 * BAR + 700,
        },
    );
    assert_eq!(
        m.view().transport.position,
        5 * BAR + 700,
        "where it was put"
    );
    assert_eq!(
        ops(&cmd(m, Command::PlayStop)),
        [EngineOp::Play {
            from: 5 * BAR + 700
        }],
        "inside the loop: from there"
    );
}

#[test]
fn the_loop_turned_on_with_no_range_takes_four_bars_from_the_current_one() {
    let m = &mut ready();
    cmd(m, Command::Locate { tick: 2 * BAR + 5 });
    let o = ops(&cmd(m, Command::ToggleLoop));
    assert_eq!(
        o,
        [EngineOp::Loop {
            start: 2 * BAR,
            end: 6 * BAR,
            on: true
        }]
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::Summary {
            loop_bars: Some(BarSpan { from: 3, to: 7 }),
            nothing_selected: true,
            notes: None,
        })
    );
    // Off keeps the range.
    cmd(m, Command::ToggleLoop);
    assert!(!m.view().song.loop_on);
    assert!(m.view().song.loop_range.is_some());
    cmd(m, Command::GoToStart);
    assert_eq!(m.view().transport.position, 0);
}

#[test]
fn record_in_a_song_with_no_tracks_says_so() {
    let m = &mut ready();
    assert!(m.view().transport.can_record);
    cmd(m, Command::DeleteTrack { track: 1 });
    assert_eq!(m.view().song.play_target, None);
    assert!(!m.view().transport.can_record);
    assert!(ops(&cmd(m, Command::Record)).is_empty());
    assert_eq!(status(m), Some(StatusMessage::NoArmedTrack));
    cmd(m, Command::Undo);
    assert_eq!(m.view().song.play_target, Some(1));
    assert!(m.view().transport.can_record);
}

#[test]
fn record_counts_in_then_records() {
    let m = &mut ready();
    let o = ops(&cmd(m, Command::Record));
    assert_eq!(
        o,
        [EngineOp::Record {
            from: 0,
            count_in_beats: 4,
            muted: 0
        }]
    );
    let t = m.view().transport;
    assert_eq!(t.state, TransportState::CountIn);
    assert_eq!(t.count_in_beats, 4);
    let rec = t.recording.unwrap();
    assert_eq!((rec.track, rec.from, rec.take, rec.lap), (1, 0, 1, 0));
    assert_eq!(status(m), Some(StatusMessage::CountingIn { beats: 4 }));
    // Undo, tempo and the loop wait.
    assert!(ops(&cmd(m, Command::SetTempo { bpm: 120.0 })).is_empty());
    assert!(ops(&cmd(m, Command::ToggleLoop)).is_empty());

    hear(m, 1, 12.0, TransportState::Recording, 0);
    assert_eq!(m.view().transport.state, TransportState::Recording);
    assert_eq!(
        status(m),
        Some(StatusMessage::Recording {
            track: piano(),
            take: 1
        })
    );

    // Record again stops.
    m.set_now(9, None);
    assert_eq!(ops(&cmd(m, Command::Record)), [EngineOp::Stop { t_us: 9 }]);
}

#[test]
fn record_starts_at_the_loop_when_it_is_on() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 2 });
    cmd(m, Command::Locate { tick: 7 * BAR });
    cmd(
        m,
        Command::SetLoop {
            from_bar: 3,
            to_bar: 5,
        },
    );
    cmd(m, Command::ToggleLoop);
    assert_eq!(
        ops(&cmd(m, Command::Record)),
        [EngineOp::Record {
            from: 7 * BAR,
            count_in_beats: 8,
            muted: 0
        }],
        "the loop is off: from the position"
    );
    m.set_now(1, None);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 1,
            tick: 0.0,
            lap: 0,
        },
    );
    cmd(m, Command::ToggleLoop);
    assert_eq!(
        ops(&cmd(m, Command::Record)),
        [EngineOp::Record {
            from: 2 * BAR,
            count_in_beats: 8,
            muted: 0
        }]
    );
    assert_eq!(m.view().transport.position, 2 * BAR);
}

#[test]
fn record_while_playing_starts_where_the_listener_is() {
    let m = &mut ready();
    cmd(m, Command::PlayStop);
    hear(m, 1, 3000.9, TransportState::Playing, 0);
    assert_eq!(
        ops(&cmd(m, Command::Record)),
        [EngineOp::RecordHere { muted: 0 }]
    );
    let t = m.view().transport;
    assert_eq!(t.state, TransportState::Recording);
    assert_eq!(
        t.recording.unwrap().from,
        3000,
        "a guess until the engine says"
    );
    // The engine says where the listener was when it took the command.
    report(
        m,
        RecordReport::Started {
            from: 3012,
            held: HeldControls::default(),
        },
    );
    let t = m.view().transport;
    assert_eq!((t.position, t.recording.unwrap().from), (3012, 3012));
    event(m, on(60), 10, 3100.0, 0);
    event(m, off(60), 11, 3200.0, 0);
    m.set_now(12, None);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 12,
            tick: 3300.0,
            lap: 0,
        },
    );
    let c = &m.view().song.tracks[0].clips[0];
    assert_eq!((c.start, c.end), (3012, 3300));
}

#[test]
fn a_recording_becomes_a_clip_selected_in_the_piano_roll() {
    let m = &mut ready();
    let effects = record_once(m, &[(60, 100.4, 600.0), (64, 480.0, 960.0)], 1000.0);
    let view = m.view();
    let clips = &view.song.tracks[0].clips;
    assert_eq!(clips.len(), 1);
    let c = &clips[0];
    // The take starts where recording started.
    assert_eq!(
        (c.start, c.end, c.take, c.takes, c.note_count),
        (0, 1000, 1, 1, 2)
    );
    assert_eq!(c.name, "ピアノ");
    assert_eq!(view.song.undo_count, 1);
    assert_eq!(view.song.note_count, 2);
    assert_eq!(view.arrange.selected_clip, Some(c.id));
    assert_eq!(view.arrange.piano_roll_clip, Some(c.id));
    assert!(view.transport.recording.is_none());
    assert_eq!(view.transport.position, 1000);
    let seq = ops(&effects);
    assert!(
        matches!(&seq[..], [EngineOp::Sequence { track: 0, events }] if events.len() == 4),
        "{seq:?}"
    );

    // The take's notes, as the screen fetches them.
    let take = TakeNotes::decode(&m.take_notes(c.take_id).unwrap()).unwrap();
    assert_eq!(take.id, c.take_id);
    assert_eq!(
        take.notes
            .iter()
            .map(|n| (n.start, n.length, n.key))
            .collect::<Vec<_>>(),
        [(100, 500, 60), (480, 480, 64)]
    );
    assert!(m.take_notes(9999).is_none());

    // A second take over the same place goes in the same clip.
    cmd(m, Command::Locate { tick: 0 });
    record_once(m, &[(67, 200.0, 300.0)], 900.0);
    let c = &m.view().song.tracks[0].clips[0];
    assert_eq!((c.take, c.takes), (2, 2));
}

#[test]
fn live_notes_go_out_as_keys_come_and_drop_what_came_after_stop() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    let id = m.view().transport.recording.unwrap().id;
    started(m);
    let a = live(&event(m, on(60), 10, 100.4, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].recording, id);
    assert_eq!(a[0].notes[0].length, None);
    assert_eq!(a[0].notes[0].start, 100);
    let b = live(&event(m, off(60), 20, 340.0, 0));
    assert_eq!(b[0].seq, a[0].seq + 1);
    assert_eq!(b[0].notes[0].length, Some(240));
    // A pedal is not a note.
    assert!(
        live(&event(
            m,
            RecEventKind::Cc {
                number: 64,
                value: 127
            },
            21,
            350.0,
            0
        ))
        .is_empty()
    );
    let snap = m.live_notes();
    assert_eq!((snap.seq, snap.recording), (b[0].seq, Some(id)));
    assert_eq!(snap.notes.len(), 1);

    m.set_now(50, None);
    cmd(m, Command::Record);
    assert!(
        live(&event(m, on(62), 51, 400.0, 0)).is_empty(),
        "after stop"
    );
    report(
        m,
        RecordReport::Stopped {
            t_us: 50,
            tick: 380.0,
            lap: 0,
        },
    );
    assert_eq!(m.view().song.tracks[0].clips[0].note_count, 1);
    let snap = m.live_notes();
    assert_eq!((snap.seq, snap.recording), (b[0].seq, None));
}

#[test]
fn nothing_played_makes_no_clip_and_no_step() {
    let m = &mut ready();
    let effects = record_once(m, &[], 800.0);
    assert!(ops(&effects).is_empty());
    let view = m.view();
    assert!(view.song.tracks[0].clips.is_empty());
    assert_eq!(view.song.undo_count, 0);
    assert!(view.transport.recording.is_none());
}

#[test]
fn a_loop_recording_makes_a_take_per_lap() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(
        m,
        Command::SetLoop {
            from_bar: 1,
            to_bar: 2,
        },
    );
    cmd(m, Command::Record);
    started(m);
    hear(m, 1, 10.0, TransportState::Recording, 0);
    assert_eq!(
        status(m),
        Some(StatusMessage::LoopRecording {
            track: piano(),
            from_bar: 1,
            to_bar: 2,
            lap: 1
        })
    );
    event(m, on(60), 10, 100.0, 0);
    event(m, off(60), 11, 200.0, 0);
    // Held over the end of the lap: cut there.
    event(m, on(62), 12, 1800.0, 0);
    let cut = live(&report(m, RecordReport::LapChanged { lap: 1 }));
    assert_eq!(cut[0].notes[0].length, Some(120));
    assert_eq!(m.view().transport.recording.unwrap().lap, 1);
    assert_eq!(
        status(m),
        Some(StatusMessage::LoopRecording {
            track: piano(),
            from_bar: 1,
            to_bar: 2,
            lap: 2
        })
    );
    assert_eq!(
        m.view().transport.recording.unwrap().take,
        2,
        "lap 1 had notes"
    );
    let next = live(&event(m, on(64), 13, 300.0, 1));
    assert_eq!((next[0].notes[0].lap, next[0].notes[0].index), (1, 0));
    event(m, off(62), 14, 320.0, 1);
    event(m, off(64), 15, 500.0, 1);
    // Lap 2 gets take 3; it has no notes, so lap 3 gets take 3 too.
    report(m, RecordReport::LapChanged { lap: 2 });
    assert_eq!(m.view().transport.recording.unwrap().take, 3);
    report(m, RecordReport::LapChanged { lap: 3 });
    assert_eq!(
        m.view().transport.recording.unwrap().take,
        3,
        "lap 2 made no take"
    );
    m.set_now(16, None);
    cmd(m, Command::PlayStop);
    report(
        m,
        RecordReport::Stopped {
            t_us: 16,
            tick: 900.0,
            lap: 3,
        },
    );
    let c = &m.view().song.tracks[0].clips[0];
    assert_eq!((c.start, c.end, c.take, c.takes), (0, BAR, 2, 2));
    assert_eq!(m.view().song.undo_count, 1, "one step for all the laps");
}

#[test]
fn the_output_going_away_keeps_what_was_recorded_and_says_so() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    event(m, on(60), 10, 100.0, 0);
    hear(m, 11, 700.0, TransportState::Recording, 0);
    m.set_now(12, None);
    m.handle(picture(Availability::No));
    let view = m.view();
    assert!(view.transport.recording.is_none());
    assert_eq!(view.transport.state, TransportState::Stopped);
    assert_eq!(view.transport.position, 700);
    let c = &view.song.tracks[0].clips[0];
    assert_eq!((c.start, c.end), (0, 700));
    assert_eq!(
        view.status.message,
        Some(StatusMessage::RecordingStoppedOutputMissing {
            output: "アナログヘッドフォン".into(),
            take: 1
        }),
        "over the output being missing"
    );
    // A report that comes late changes nothing.
    report(
        m,
        RecordReport::Stopped {
            t_us: 12,
            tick: 720.0,
            lap: 0,
        },
    );
    assert_eq!(m.view().song.undo_count, 1);
}

#[test]
fn pipewire_going_away_keeps_what_was_recorded() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    event(m, on(60), 10, 100.0, 0);
    hear(m, 11, 500.0, TransportState::Recording, 0);
    m.set_now(12, None);
    m.handle(Input::AudioSystemLost);
    assert_eq!(
        status(m),
        Some(StatusMessage::RecordingStoppedAudioSystem { take: 1 })
    );
    // The next thing done takes the notice away; the stopped system is said again.
    cmd(m, Command::SelectClip { clip: None });
    assert_eq!(status(m), Some(StatusMessage::AudioSystemStopped));
}

#[test]
fn keyboard_input_stopping_lets_the_keys_go_and_records_on() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    event(m, on(60), 10, 100.0, 0);
    m.set_now(
        11,
        Some(Heard {
            tick: 400.2,
            state: TransportState::Recording,
            lap: 0,
        }),
    );
    let effects = m.handle(Input::MidiStopped);
    let notes = live(&effects);
    assert_eq!(notes[0].notes[0].length, Some(300));
    assert!(m.view().transport.recording.is_some(), "still recording");
    event(m, on(62), 12, 500.0, 0);
    event(m, off(62), 13, 600.0, 0);
    m.set_now(14, None);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 14,
            tick: 700.0,
            lap: 0,
        },
    );
    assert_eq!(m.view().song.tracks[0].clips[0].note_count, 2);
}

#[test]
fn the_piano_roll_section_follows_the_shown_take() {
    let m = &mut ready();
    let empty = m.view().piano_roll;
    assert_eq!(empty.take, None);
    assert_eq!(
        (empty.grid, empty.scale, empty.quantize),
        (
            Grid {
                division: 16,
                triplet: false
            },
            MusicalScale {
                root: 0,
                mode: ScaleMode::Major,
                lock: false
            },
            QuantizeSettings {
                division: 16,
                triplet: false,
                strength: 100,
                swing: 0,
                align: QuantizeAlign::Start
            }
        )
    );
    assert!(empty.selected_notes.is_empty() && empty.selected_controls.is_empty());
    assert_eq!(empty.clipboard_notes, 0);

    record_once(m, &[(60, 0.0, 100.0)], 400.0);
    cmd(m, Command::Locate { tick: 0 });
    record_once(m, &[(62, 0.0, 100.0)], 400.0);
    let c = m.view().song.tracks[0].clips[0].clone();
    assert_eq!(m.view().piano_roll.take, Some(c.take_id));
    let take = TakeNotes::decode(&m.take_notes(c.take_id).unwrap()).unwrap();
    assert_eq!(take.origin, None);
    assert!(take.notes.iter().all(|n| n.from.is_none()));

    cmd(m, Command::StepTake { delta: 1 });
    let shown = m.view().song.tracks[0].clips[0].take_id;
    assert_ne!(shown, c.take_id);
    assert_eq!(m.view().piano_roll.take, Some(shown));

    // Selecting and the grid change the piano roll only.
    let before = m.view();
    cmd(
        m,
        Command::SelectNotes {
            take: shown,
            notes: vec![0],
            mode: SelectMode::Replace,
        },
    );
    cmd(
        m,
        Command::SetGrid {
            division: 8,
            triplet: true,
        },
    );
    let after = m.view().piano_roll;
    assert_eq!(after.selected_notes, [IndexRange { start: 0, end: 1 }]);
    assert_eq!(
        after.grid,
        Grid {
            division: 8,
            triplet: true
        }
    );
    assert_eq!(m.view().song, before.song);
}

#[test]
fn takes_step_round_and_undo_and_redo_come_back_to_the_same_song() {
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], 400.0);
    cmd(m, Command::Locate { tick: 0 });
    record_once(m, &[(62, 0.0, 100.0)], 400.0);
    cmd(m, Command::Locate { tick: 0 });
    record_once(m, &[(64, 0.0, 100.0), (65, 100.0, 200.0)], 400.0);
    let c = m.view().song.tracks[0].clips[0].clone();
    assert_eq!((c.take, c.takes), (3, 3));
    assert_eq!(m.view().arrange.selected_clip, Some(c.id));

    let effects = cmd(m, Command::StepTake { delta: 1 });
    assert!(matches!(
        &ops(&effects)[..],
        [EngineOp::Sequence { track: 0, .. }]
    ));
    assert_eq!(
        m.view().song.tracks[0].clips[0].take,
        1,
        "past the last: the first"
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::TakeSwitched {
            track: piano(),
            clip: "ピアノ".into(),
            take: 1,
            takes: 3
        })
    );
    let last_step = ops(&cmd(m, Command::StepTake { delta: -1 }));
    assert_eq!(m.view().song.tracks[0].clips[0].take, 3);
    assert_eq!(m.view().song.note_count, 2);

    let before = m.view();
    assert_eq!(before.song.undo_count, 5);
    let back = cmd(m, Command::Undo);
    assert_eq!(m.view().song.tracks[0].clips[0].take, 1);
    assert_eq!(m.view().song.redo_count, 1);
    let forth = cmd(m, Command::Redo);
    let after = m.view();
    assert_eq!(after.song, before.song);
    assert_eq!(after.arrange, before.arrange);
    assert_eq!(ops(&back).len(), 1);
    assert_eq!(ops(&forth), last_step, "the engine plays the same again");

    // Delete: one step, the selection goes; undo brings the clip back, selected again.
    let effects = cmd(m, Command::DeleteSelected);
    assert_eq!(
        ops(&effects),
        [EngineOp::Sequence {
            track: 0,
            events: vec![]
        }]
    );
    let view = m.view();
    assert!(view.song.tracks[0].clips.is_empty());
    assert_eq!(view.arrange.selected_clip, None);
    assert_eq!(view.arrange.piano_roll_clip, None);
    cmd(m, Command::Undo);
    assert_eq!(m.view().song.tracks[0].clips.len(), 1);
    assert_eq!(m.view().arrange.selected_clip, Some(c.id));

    // Undoing the first recording takes the clip away, and the selection with it.
    cmd(m, Command::SelectClip { clip: Some(c.id) });
    for _ in 0..10 {
        cmd(m, Command::Undo);
    }
    let view = m.view();
    assert!(view.song.tracks[0].clips.is_empty());
    assert_eq!(view.arrange, ArrangeView::default());
    assert_eq!(view.song.undo_count, 0);
}

#[test]
fn deleting_a_take_is_one_step_and_undo_puts_it_back_in_its_place() {
    let m = &mut ready();
    for key in [60, 62, 64] {
        cmd(m, Command::Locate { tick: 0 });
        record_once(m, &[(key, 0.0, 100.0)], 400.0);
    }
    cmd(m, Command::StepTake { delta: -1 });
    let ids = |m: &Model| {
        let c = &m.play().song().doc.tracks[0].clips[0];
        (
            c.takes.iter().map(|t| t.take.id.0).collect::<Vec<_>>(),
            c.active,
        )
    };
    let c = m.view().song.tracks[0].clips[0].clone();
    let (all, shown) = ids(m);
    assert_eq!((shown, c.take, c.takes), (1, 2, 3));
    let before = m.view();

    let effects = cmd(m, Command::DeleteTake { clip: c.id });
    assert!(matches!(
        &ops(&effects)[..],
        [EngineOp::Sequence { track: 0, .. }]
    ));
    assert_eq!(ids(m), (vec![all[0], all[2]], 0), "the take before shows");
    let v = m.view();
    let after = v.song.tracks[0].clips[0].clone();
    assert_eq!((after.take, after.takes, after.take_id), (1, 2, all[0]));
    assert_eq!(v.song.undo_count, before.song.undo_count + 1);
    assert_eq!(v.piano_roll.take, Some(all[0]));
    assert_eq!(
        m.play().steps().last(),
        Some(&StepKind::DeleteTake {
            clip: ClipId(c.id),
            index: 1
        })
    );
    let after_delete = m.view();

    cmd(m, Command::Undo);
    assert_eq!(ids(m), (all.clone(), 1));
    let back = m.view();
    assert_eq!(back.song.tracks, before.song.tracks);
    assert_eq!(back.piano_roll.take, Some(all[1]));
    assert!(m.take_notes(all[1]).is_some(), "its notes are still there");

    cmd(m, Command::Redo);
    assert_eq!(m.view().song.tracks, after_delete.song.tracks);

    // The first take: the next one shows. Then one take is left, and nothing happens.
    cmd(m, Command::DeleteTake { clip: c.id });
    assert_eq!(ids(m), (vec![all[2]], 0));
    let one = m.view();
    assert!(ops(&cmd(m, Command::DeleteTake { clip: c.id })).is_empty());
    assert!(ops(&cmd(m, Command::DeleteTake { clip: 999 })).is_empty());
    assert_eq!(m.view().song, one.song);
}

#[test]
fn deleting_a_take_waits_while_recording_and_counting_in() {
    let m = &mut ready();
    for key in [60, 62] {
        cmd(m, Command::Locate { tick: 0 });
        record_once(m, &[(key, 0.0, 100.0)], 400.0);
    }
    let clip = m.view().song.tracks[0].clips[0].id;
    cmd(m, Command::Locate { tick: 4 * BAR });
    cmd(m, Command::Record);
    assert_eq!(m.view().transport.state, TransportState::Recording);
    let before = m.view().song;
    assert!(ops(&cmd(m, Command::DeleteTake { clip })).is_empty());
    assert_eq!(m.view().song, before);

    let m = &mut ready();
    for key in [60, 62] {
        cmd(m, Command::Locate { tick: 0 });
        record_once(m, &[(key, 0.0, 100.0)], 400.0);
    }
    cmd(m, Command::SetCountIn { bars: 1 });
    cmd(m, Command::Record);
    assert_eq!(m.view().transport.state, TransportState::CountIn);
    let before = m.view().song;
    assert!(ops(&cmd(m, Command::DeleteTake { clip })).is_empty());
    assert_eq!(m.view().song, before);
}

#[test]
fn undo_waits_while_recording() {
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], 400.0);
    cmd(m, Command::Record);
    assert!(ops(&cmd(m, Command::Undo)).is_empty());
    assert_eq!(m.view().song.undo_count, 1);
    assert!(ops(&cmd(m, Command::DeleteSelected)).is_empty());
}

#[test]
fn tempo_and_meter_in_range_are_steps() {
    let m = &mut ready();
    assert!(cmd(m, Command::SetTempo { bpm: 29.99 }).is_empty());
    assert!(cmd(m, Command::SetTempo { bpm: 300.01 }).is_empty());
    assert!(cmd(m, Command::SetTempo { bpm: f64::NAN }).is_empty());
    assert!(cmd(m, Command::SetMeter { num: 13, den: 4 }).is_empty());
    assert!(cmd(m, Command::SetMeter { num: 3, den: 3 }).is_empty());
    assert_eq!(
        ops(&cmd(m, Command::SetTempo { bpm: 120.004 })),
        [EngineOp::Tempo {
            ticks_per_beat: 480,
            beats_per_bar: 4,
            bpm_centi: 12_000
        }]
    );
    assert_eq!(m.view().timeline.tempo_bpm, 120.0);
    assert_eq!(
        ops(&cmd(m, Command::SetMeter { num: 6, den: 8 })),
        [EngineOp::Tempo {
            ticks_per_beat: 240,
            beats_per_bar: 6,
            bpm_centi: 12_000
        }]
    );
    assert_eq!(m.view().transport.count_in_beats, 6);
    assert_eq!(m.view().song.undo_count, 2);
    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    assert_eq!(m.view().timeline.tempo_bpm, 96.0);
    // The position goes where it is put, not to the head of a bar.
    cmd(m, Command::Redo);
    cmd(m, Command::Redo);
    cmd(m, Command::Locate { tick: 1500 });
    assert_eq!(m.view().transport.position, 1500);
}

#[test]
fn metronome_and_count_in_are_settings() {
    let m = &mut ready();
    let effects = cmd(m, Command::ToggleMetronome);
    assert!(effects.contains(&Effect::Engine(EngineOp::Metronome { on: true })));
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::SaveSettings(s) if s.metronome && s.count_in_bars == 1
    )));
    let effects = cmd(m, Command::SetCountIn { bars: 2 });
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::SaveSettings(s) if s.metronome && s.count_in_bars == 2
    )));
    assert!(cmd(m, Command::SetCountIn { bars: 3 }).is_empty());
    assert!(cmd(m, Command::SetCountIn { bars: 2 }).is_empty());
    let t = m.view().transport;
    assert!(t.metronome);
    assert_eq!((t.count_in_bars, t.count_in_beats), (2, 8));

    let again = Model::new(Settings {
        metronome: true,
        count_in_bars: 0,
        ..Settings::default()
    });
    let t = again.view().transport;
    assert!(t.metronome);
    assert_eq!(t.count_in_bars, 0);
}

#[test]
fn the_status_line_at_rest() {
    let m = &mut ready();
    assert_eq!(
        status(m),
        Some(StatusMessage::NewSong {
            tempo_bpm: 96.0,
            meter_num: 4,
            meter_den: 4,
            track: Some(piano())
        })
    );
    assert_eq!(
        m.view().workspace.hint,
        Some(meloet_protocol::Hint::EmptyArrange)
    );
    cmd(m, Command::SelectClip { clip: None });
    assert_eq!(
        status(m),
        Some(StatusMessage::Summary {
            loop_bars: None,
            nothing_selected: true,
            notes: None,
        })
    );
    // Not while counting in or recording; back after a recording with no notes.
    cmd(m, Command::Record);
    assert_eq!(m.view().workspace.hint, None, "counting in");
    started(m);
    hear(m, 1, 10.0, TransportState::Recording, 0);
    assert_eq!(m.view().workspace.hint, None, "recording");
    m.set_now(2, None);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 2,
            tick: 20.0,
            lap: 0,
        },
    );
    assert_eq!(
        m.view().workspace.hint,
        Some(meloet_protocol::Hint::EmptyArrange)
    );
    cmd(m, Command::GoToStart);
    record_once(m, &[(60, 0.0, 100.0)], 400.0);
    assert_eq!(status(m), None, "a clip is selected and the loop is off");
    assert_eq!(m.view().workspace.hint, None, "a clip came");
    cmd(m, Command::DeleteSelected);
    assert_eq!(m.view().workspace.hint, None, "not again for this song");
    cmd(m, Command::PlayStop);
    assert_eq!(status(m), None, "playing");
}
