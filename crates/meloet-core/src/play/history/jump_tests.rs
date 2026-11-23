//! A jump through the history against the same way walked one step at a time
//! (docs/architecture.md 21-12).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Instant;

use meloet_model::playback::PlayEvent;
use meloet_model::{InsertId, PluginPlace, Timeline};
use meloet_protocol::{
    AutoTarget, Command, EffectKind, EffectRef, InsertRef, InstrumentKind, ParamRef, PluginSlotRef,
    SelectedParams, ShelfMode, SoundRef, StatusMessage, StripRef,
};

use super::*;
use crate::catalog::tests::{COMP_LV2, COMP_VST3, SURGE_LV2, SURGE_VST3};
use crate::play::EngineOp;
use crate::play::autosync::SeatLane;
use crate::play::plugs::tests::{Mini, Rng, filled_slots, held};
use crate::play::tests::{cmd, ready};
use crate::play::windows::tests::Rig;
use crate::state::added_tests::{plugin, rigged};
use crate::state::{Effect, Input, Model, PluginSaid};

const Q: u64 = meloet_model::TICKS_PER_QUARTER;

/// A model with the fake engine and the fake plugins behind it, and what the engine was
/// told last of the things the fake engine does not keep.
struct World {
    m: Model,
    mini: Mini,
    timeline: Option<Arc<Timeline>>,
    events: HashMap<usize, Vec<PlayEvent>>,
    lanes: HashMap<usize, Vec<SeatLane>>,
}

/// What a jump or a walk sent.
#[derive(Default)]
struct Sent {
    /// How often each plugin was given a state.
    states: BTreeMap<u64, u32>,
    timelines: u32,
    /// How often each seat was given its notes, and its lanes.
    sequences: BTreeMap<usize, u32>,
    automations: BTreeMap<usize, u32>,
    moves: u32,
    loads: u32,
}

impl Sent {
    fn take(&mut self, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::SetPluginState { ticket, .. } => {
                    *self.states.entry(*ticket).or_default() += 1
                }
                Effect::LoadPlugin { .. } => self.loads += 1,
                Effect::Engine(EngineOp::Timeline { .. }) => self.timelines += 1,
                Effect::Engine(EngineOp::Sequence { track, .. }) => {
                    *self.sequences.entry(*track).or_default() += 1;
                }
                Effect::Engine(EngineOp::Automation { seat, .. }) => {
                    *self.automations.entry(*seat).or_default() += 1;
                }
                Effect::Engine(EngineOp::MoveInsert { .. }) => self.moves += 1,
                _ => {}
            }
        }
    }
}

impl World {
    fn new() -> World {
        let mut m = rigged();
        let mini = Mini::new(&mut m);
        World {
            m,
            mini,
            timeline: None,
            events: HashMap::new(),
            lanes: HashMap::new(),
        }
    }

    fn go(&mut self, input: Input, ctx: &str) -> Vec<Effect> {
        self.mini.host.now_us += 700;
        self.m.set_now(self.mini.host.now_us, None);
        let effects = self.m.handle(input.clone());
        self.mini.take(&effects);
        for effect in &effects {
            match effect {
                Effect::Engine(EngineOp::Timeline { timeline }) => {
                    self.timeline = Some(timeline.clone());
                }
                Effect::Engine(EngineOp::Sequence { track, events }) => {
                    self.events.insert(*track, events.clone());
                }
                Effect::Engine(EngineOp::Automation { seat, lanes }) => {
                    self.lanes.insert(*seat, lanes.clone());
                }
                Effect::Engine(EngineOp::ClearSeat { seat }) => {
                    self.events.remove(seat);
                    self.lanes.remove(seat);
                }
                _ => {}
            }
        }
        self.mini.check(&self.m, &format!("{ctx}: {input:?}"));
        effects
    }

    fn cmd(&mut self, command: Command, ctx: &str) -> Vec<Effect> {
        self.go(Input::Command(command), ctx)
    }

    /// Every load is answered and every answer of a plugin has come.
    fn rest(&mut self, ctx: &str) -> Vec<Effect> {
        let mut all = Vec::new();
        loop {
            if !self.mini.asked.is_empty() {
                let ticket = self.mini.asked.remove(0);
                self.mini.waiting.insert(ticket);
                all.extend(self.go(Input::PluginLoaded { ticket }, ctx));
            } else if let Some(input) = self.mini.host.answers.pop_front() {
                all.extend(self.go(input, ctx));
            } else {
                return all;
            }
        }
    }

    fn total(&self) -> u32 {
        self.m.view().history.total
    }

    /// What the engine was told of each track, by the track: its notes and its lanes (a
    /// lane of an insert by the slot the insert is in).
    fn told(&self) -> Vec<(u32, Vec<PlayEvent>, Vec<SeatLane>)> {
        let p = self.m.play();
        let tracks = p.song().doc.tracks.iter().zip(p.track_seats());
        tracks
            .map(|(t, seat)| {
                let seat = seat.expect("a seat");
                let events = self.events.get(&seat).cloned().unwrap_or_default();
                let lanes = self.lanes.get(&seat).cloned().unwrap_or_default();
                (t.id.0, events, lanes)
            })
            .collect()
    }

    /// The state every plugin of the song has, by its place.
    fn plugin_states(&self) -> Vec<(PluginPlace, [f64; 3])> {
        let mut states: Vec<_> = (self.mini.of_song(&self.m).into_iter())
            .map(|(place, ticket)| (place, self.mini.host.plugins[&ticket]))
            .collect();
        states.sort_by_key(|(place, _)| format!("{place:?}"));
        states
    }

    /// Every insert the song has, by its id, with its strip.
    fn insert_ids(&self) -> Vec<(StripRef, InsertId)> {
        let doc = &self.m.play().song().doc;
        (filled_slots(doc).into_iter())
            .map(|(at, _)| {
                let strip = crate::play::mixer::strip_id(at.strip);
                (
                    at.strip,
                    doc.insert(strip, usize::from(at.slot)).unwrap().id,
                )
            })
            .collect()
    }
}

/// One thing a person does, at random; then everything comes to rest.
fn act(w: &mut World, rng: &mut Rng, round: usize, ctx: &str) {
    let view = w.m.view();
    let tracks: Vec<u32> = view.song.tracks.iter().map(|t| t.id).collect();
    let track = if tracks.is_empty() {
        9_999
    } else {
        rng.of(&tracks)
    };
    let strip = match rng.below(6) {
        0 => StripRef::Master,
        1 => StripRef::Return {
            bus: meloet_protocol::SendBus::A,
        },
        _ => StripRef::Track { id: track },
    };
    let insert = InsertRef {
        strip,
        slot: rng.below(4) as u8,
    };
    let filled = filled_slots(&w.m.play().song().doc);
    let (held_at, held_kind) = match filled.is_empty() {
        true => (insert, None),
        false => rng.of(&filled),
    };
    let clips: Vec<u32> = (view.song.tracks.iter())
        .flat_map(|t| t.clips.iter().map(|c| c.id))
        .collect();
    let lanes: Vec<u32> = (view.song.tracks.iter())
        .flat_map(|t| t.lanes.iter().map(|l| l.lane_id))
        .collect();
    let take = view.piano_roll.take;
    let sounds = [
        SoundRef::Builtin {
            instrument: InstrumentKind::Sine,
        },
        SoundRef::Builtin {
            instrument: InstrumentKind::SynthLead,
        },
        plugin(SURGE_VST3),
        plugin(SURGE_LV2),
    ];
    let effects = [
        EffectRef::Plugin {
            key: COMP_LV2.into(),
        },
        EffectRef::Plugin {
            key: COMP_VST3.into(),
        },
        EffectRef::Builtin {
            effect: EffectKind::Reverb,
        },
        EffectRef::Builtin {
            effect: EffectKind::Compressor,
        },
        EffectRef::Builtin {
            effect: EffectKind::Chorus,
        },
    ];
    let mut commands = Vec::new();
    match rng.below(44) {
        0..=3 => commands.push(Command::AddNote {
            take,
            start: rng.below(32) * (Q / 2),
            length: Q / 2,
            key: 48 + rng.below(24) as u8,
            velocity: 90,
        }),
        4 | 5 => {
            if let Some(take) = take {
                commands.push(Command::MoveNotes {
                    take,
                    notes: vec![0],
                    ticks: (rng.below(5) as i64 - 2) * (Q / 4) as i64,
                    keys: rng.below(5) as i32 - 2,
                    copy: rng.below(4) == 0,
                });
            }
        }
        6 => {
            if let Some(take) = take {
                commands.push(Command::DeleteNotes {
                    take,
                    notes: vec![0],
                });
            }
        }
        7 | 8 => {
            if !clips.is_empty() {
                commands.push(Command::MoveClips {
                    clips: vec![rng.of(&clips)],
                    ticks: (rng.below(9) as i64 - 4) * (4 * Q) as i64,
                    tracks: 0,
                    copy: rng.below(3) == 0,
                });
            }
        }
        9 => {
            if !clips.is_empty() {
                commands.push(Command::SelectClip {
                    clip: Some(rng.of(&clips)),
                });
            }
        }
        10 => commands.push(Command::SetTempo {
            bpm: (60 + rng.below(120)) as f64,
        }),
        11 => commands.push(Command::AddTempoPoint {
            tick: (1 + rng.below(16)) * (4 * Q),
            bpm: (60 + rng.below(120)) as f64,
        }),
        12 => {
            commands.push(Command::Locate {
                tick: rng.below(32) * Q,
            });
            commands.push(Command::AddMarker {
                name: format!("m{round}"),
                tick: None,
            });
            commands.push(Command::CloseMarkerName);
        }
        13 => {
            if !view.timeline.markers.is_empty() {
                commands.push(Command::MoveMarker {
                    id: rng.of(&view.timeline.markers).id,
                    tick: rng.below(64) * Q,
                });
            }
        }
        14..=16 => commands.push(Command::SetGain {
            strip,
            db: Some(-(rng.below(24) as f32)),
            commit: true,
        }),
        17 => commands.push(Command::SetPan {
            strip,
            pan: rng.below(201) as i32 - 100,
            commit: true,
        }),
        18 => commands.push(Command::SetSend {
            track,
            bus: meloet_protocol::SendBus::A,
            db: Some(-(rng.below(30) as f32)),
            commit: true,
        }),
        19 => commands.push(match rng.below(2) {
            0 => Command::ToggleMute { track },
            _ => Command::ToggleReturnMute {
                bus: meloet_protocol::SendBus::B,
            },
        }),
        20..=22 => {
            let mode = ShelfMode::InsertEffect {
                strip,
                slot: insert.slot,
            };
            commands.push(Command::OpenShelf { mode });
            let effect = Some(rng.of(&effects));
            commands.push(Command::ShelfChooseEffect { effect });
            commands.push(Command::ShelfConfirm);
        }
        23 => commands.push(Command::RemoveEffect { insert: held_at }),
        24..=27 => commands.push(Command::MoveInsert {
            insert: held_at,
            to: rng.below(4) as u8,
        }),
        28 => commands.push(Command::ToggleBypass { insert: held_at }),
        29 => {
            let mode = ShelfMode::ReplaceEffect {
                strip: held_at.strip,
                slot: held_at.slot,
            };
            commands.push(Command::OpenShelf { mode });
            let effect = Some(rng.of(&effects));
            commands.push(Command::ShelfChooseEffect { effect });
            commands.push(Command::ShelfConfirm);
        }
        30 => {
            let param = ParamRef::Builtin {
                insert: held_at,
                effect: held_kind.unwrap_or(EffectKind::Reverb),
                param: rng.below(4) as u32,
            };
            let text = rng.of(&["4", "−3 dB", "0.5 s", "1", "30"]).to_owned();
            commands.push(Command::TypeParam { param, text });
        }
        31 => {
            let mode = match rng.below(3) {
                0 => ShelfMode::NewTrack,
                _ => ShelfMode::Replace { track },
            };
            commands.push(Command::OpenShelf { mode });
            let sound = Some(rng.of(&sounds));
            commands.push(Command::ShelfSelectSound { sound });
            commands.push(Command::ShelfConfirm);
        }
        32 => {
            if tracks.len() > 2 && rng.below(3) == 0 {
                commands.push(Command::DeleteTrack { track });
            } else {
                commands.push(Command::MoveTrack {
                    track,
                    to: rng.below(tracks.len() as u64) as u32,
                });
            }
        }
        33..=35 => {
            // A visit to the window of a plugin: something is turned, and they leave.
            let of_song = w.mini.of_song(&w.m);
            if !of_song.is_empty() {
                let (_, ticket) = rng.of(&of_song);
                let said = PluginSaid::WindowEntered;
                w.go(Input::PluginSaid { ticket, said }, ctx);
                for _ in 0..=rng.below(2) {
                    let value = rng.below(11) as f64 / 10.0;
                    let turned = w.mini.host.turn(ticket, rng.below(3) as u32, value);
                    w.go(turned, ctx);
                }
                let said = PluginSaid::WindowLeft;
                w.go(Input::PluginSaid { ticket, said }, ctx);
            }
        }
        36 => {
            // A row of the parameters panel is dragged and let go.
            let of_song = w.mini.of_song(&w.m);
            if !of_song.is_empty() {
                let (place, _) = rng.of(&of_song);
                let slot = Play::slot_ref(place);
                let select = match slot {
                    PluginSlotRef::Instrument { track } => Command::SelectInstrument { track },
                    PluginSlotRef::Insert { insert } => Command::SelectInsert {
                        insert: Some(insert),
                    },
                };
                w.cmd(select, ctx);
                w.rest(ctx);
                let table = match w.m.view().params.selected {
                    Some(SelectedParams::Plugin(plugin)) => plugin.table.unwrap_or(0),
                    _ => 0,
                };
                commands.push(Command::SetPluginParam {
                    slot,
                    table,
                    param: rng.below(3) as u32,
                    value: rng.below(11) as f64 / 10.0,
                    commit: true,
                });
            }
        }
        37 => commands.push(Command::AddLane {
            track,
            target: match rng.below(2) {
                0 => AutoTarget::Volume,
                _ => AutoTarget::Pan,
            },
        }),
        38 | 39 => {
            if !lanes.is_empty() {
                commands.push(Command::AddAutoPoint {
                    lane: rng.of(&lanes),
                    tick: rng.below(32) * Q,
                    value: Some(rng.below(11) as f32 / 10.0),
                });
            }
        }
        40 | 41 => commands.push(Command::Undo),
        42 => commands.push(Command::Redo),
        _ => commands.push(Command::SelectInsert {
            insert: Some(held_at),
        }),
    }
    for command in commands {
        w.cmd(command, ctx);
    }
    w.cmd(Command::ShelfCancel, ctx);
    w.rest(ctx);
}

/// The steps of a song made at random, by kind of thing.
fn kinds(m: &Model) -> [usize; 9] {
    let mut count = [0; 9];
    for step in m.play().steps() {
        use StepKind::*;
        let at = match step {
            WriteNote { .. } | MoveNotes { .. } | DeleteNotes { .. } => 0,
            MoveClips { .. } => 1,
            SetGain { .. } | SetPan { .. } | SetSend { .. } | Mute { .. } | MuteReturn { .. } => 2,
            MoveInsert { .. } => 3,
            ReplaceEffect { .. } | AddEffect { .. } | RemoveEffect { .. } | AddPlugin { .. } => 4,
            PluginWindow { .. } | SetPluginParam { .. } => 5,
            Tempo { .. } | AddTempoPoint { .. } | SetTempoPoint { .. } => 6,
            AddMarker { .. } | MoveMarker { .. } => 7,
            AddLane { .. } | AddAutoPoint { .. } => 8,
            _ => continue,
        };
        count[at] += 1;
    }
    count
}

#[test]
fn a_jump_over_random_steps_ends_where_single_steps_do() {
    const ROUNDS: usize = 260;
    let mut seen = [0usize; 9];
    let (mut jumps, mut far, mut states_put, mut moved) = (0, 0, 0, 0);
    for seed in 1..=8u64 {
        // The same song with the same history twice: one world jumps, the other walks.
        let mut worlds = [World::new(), World::new()];
        for w in &mut worlds {
            let rng = &mut Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            for round in 0..ROUNDS {
                act(w, rng, round, &format!("seed {seed}, round {round}"));
            }
            w.mini.check_states(&w.m, "made");
        }
        let [a, b] = &mut worlds;
        assert_eq!(
            held(&a.m.play().song().doc),
            held(&b.m.play().song().doc),
            "seed {seed}: the same song twice"
        );
        for (seen, n) in seen.iter_mut().zip(kinds(&a.m)) {
            *seen += n;
        }
        let rng = &mut Rng(seed.wrapping_mul(0xD6E8_FEB8_6659_FD93) | 1);
        for jump in 0..40 {
            let ctx = format!("seed {seed}, jump {jump}");
            let total = a.total();
            let done = a.m.view().history.current;
            let to = 1 + rng.below(u64::from(total)) as u32;
            let row = a.m.history_rows(to, to).remove(0);
            let ids_before = a.insert_ids();
            let made_before = a.mini.strips.effects_made;

            // A row named by the number of another step is not jumped to.
            let before = held(&a.m.play().song().doc);
            let stale = Command::HistoryJump {
                index: to,
                step: row.step + 1_000_000,
            };
            assert!(a.cmd(stale, &ctx).is_empty(), "{ctx}: a stale row");
            assert_eq!(held(&a.m.play().song().doc), before, "{ctx}: a stale row");
            assert_eq!(a.m.view().history.current, done, "{ctx}: a stale row");

            let mut sent = Sent::default();
            let command = Command::HistoryJump {
                index: to,
                step: row.step,
            };
            sent.take(&a.cmd(command, &ctx));
            let said = a.m.view().status.message;
            a.rest(&ctx);
            for _ in to..done {
                b.cmd(Command::Undo, &ctx);
            }
            for _ in done..to {
                b.cmd(Command::Redo, &ctx);
            }
            b.rest(&ctx);

            if to == done {
                assert_eq!(sent.timelines + sent.moves + sent.loads, 0, "{ctx}");
            } else {
                let steps = done.abs_diff(to);
                let jumped = StatusMessage::HistoryJumped {
                    index: to,
                    name: row.name.clone(),
                    steps,
                    forward: to > done,
                };
                // A window that takes long to open is said before any notice.
                let opening = matches!(said, Some(StatusMessage::PluginWindowOpening { .. }));
                assert!(opening || said == Some(jumped), "{ctx}: {said:?}");
                jumps += 1;
                far += usize::from(steps > 20);
            }
            // Everything once.
            assert!(
                sent.states.values().all(|n| *n == 1),
                "{ctx}: {:?}",
                sent.states
            );
            assert!(sent.timelines <= 1, "{ctx}: {} timelines", sent.timelines);
            assert!(sent.sequences.values().all(|n| *n == 1), "{ctx}");
            assert!(sent.automations.values().all(|n| *n <= 2), "{ctx}");
            states_put += sent.states.len();
            moved += sent.moves;
            // An effect is made or put away only for an insert that came or went, and for a
            // plugin that goes back to how it starts and was never asked what that is: it is
            // loaded anew, and its slot is empty meanwhile.
            let ids_after = a.insert_ids();
            let came_or_went = (ids_before
                .iter()
                .filter(|id| !ids_after.contains(id))
                .count())
                + (ids_after
                    .iter()
                    .filter(|id| !ids_before.contains(id))
                    .count());
            let made = (a.mini.strips.effects_made - made_before) as usize;
            assert!(
                made <= came_or_went + sent.loads as usize,
                "{ctx}: {made} effects made or put away, {came_or_went} inserts came or went"
            );

            // The song, what the engine was told, and what the plugins have.
            let (doc_a, doc_b) = (&a.m.play().song().doc, &b.m.play().song().doc);
            assert_eq!(held(doc_a), held(doc_b), "{ctx}");
            assert_eq!(doc_a.next_id, doc_b.next_id, "{ctx}");
            a.mini.check_states(&a.m, &ctx);
            b.mini.check_states(&b.m, &ctx);
            assert_eq!(a.plugin_states(), b.plugin_states(), "{ctx}");
            assert_eq!(a.told(), b.told(), "{ctx}");
            assert_eq!(a.timeline, b.timeline, "{ctx}");
            let (va, vb) = (a.m.view(), b.m.view());
            assert_eq!(va.song, vb.song, "{ctx}");
            assert_eq!(va.timeline, vb.timeline, "{ctx}");
            assert_eq!(va.history, vb.history, "{ctx}");
            assert_eq!(va.arrange, vb.arrange, "{ctx}");
            assert_eq!(va.piano_roll, vb.piano_roll, "{ctx}");
            assert_eq!(va.automation, vb.automation, "{ctx}");
            assert_eq!(
                a.m.history_rows(1, total),
                b.m.history_rows(1, total),
                "{ctx}"
            );
        }
    }
    // The test did what it is for.
    assert!(seen.iter().all(|n| *n > 20), "{seen:?}");
    assert!(jumps > 250 && far > 100, "{jumps} jumps, {far} of them far");
    assert!(
        states_put > 100 && moved > 50,
        "{states_put} states, {moved} moves"
    );
}

#[test]
fn a_jump_over_moves_of_inserts_makes_no_effect_anew() {
    let mut w = World::new();
    let strip = StripRef::Track { id: 1 };
    for (slot, effect) in [EffectKind::Compressor, EffectKind::Chorus]
        .iter()
        .enumerate()
    {
        let effect = EffectRef::Builtin { effect: *effect };
        let effects = {
            let mode = ShelfMode::InsertEffect {
                strip,
                slot: slot as u8,
            };
            w.cmd(Command::OpenShelf { mode }, "made");
            w.cmd(
                Command::ShelfChooseEffect {
                    effect: Some(effect),
                },
                "made",
            );
            w.cmd(Command::ShelfConfirm, "made")
        };
        assert!(!effects.is_empty());
    }
    let mode = ShelfMode::InsertEffect { strip, slot: 2 };
    w.cmd(Command::OpenShelf { mode }, "made");
    let effect = Some(EffectRef::Plugin {
        key: COMP_LV2.into(),
    });
    w.cmd(Command::ShelfChooseEffect { effect }, "made");
    w.cmd(Command::ShelfConfirm, "made");
    w.rest("made");
    let start = w.total();
    let order_at_start = w.insert_ids();

    // The slots go round and round, a mark is set on the way, and the last slot is used.
    let moves = [
        (0, 3),
        (1, 0),
        (3, 1),
        (2, 0),
        (0, 2),
        (1, 3),
        (3, 0),
        (2, 1),
        (0, 1),
    ];
    for (n, (from, to)) in moves.into_iter().enumerate() {
        let insert = InsertRef { strip, slot: from };
        w.cmd(Command::MoveInsert { insert, to }, "moved");
        if n == 4 {
            let filled = filled_slots(&w.m.play().song().doc);
            let at = filled.iter().find(|(at, _)| at.strip == strip).unwrap().0;
            w.cmd(Command::ToggleBypass { insert: at }, "moved");
        }
    }
    let end = w.total();
    assert_eq!(end, start + moves.len() as u32 + 1);
    assert_ne!(w.insert_ids(), order_at_start);
    let order_at_end = w.insert_ids();
    let loaded = w.mini.of_song(&w.m);
    assert_eq!(loaded.len(), 1);

    for (to, order) in [
        (start, &order_at_start),
        (end, &order_at_end),
        (start + 3, &Vec::new()),
    ] {
        let made = w.mini.strips.effects_made;
        let step = w.m.history_rows(to, to)[0].step;
        let mut sent = Sent::default();
        sent.take(&w.cmd(Command::HistoryJump { index: to, step }, "jump"));
        sent.take(&w.rest("jump"));
        assert_eq!(
            w.mini.strips.effects_made, made,
            "row {to}: no effect made anew"
        );
        assert_eq!((sent.loads, sent.states.len()), (0, 0), "row {to}");
        assert!(
            (1..=3).contains(&sent.moves),
            "row {to}: {} moves",
            sent.moves
        );
        if !order.is_empty() {
            assert_eq!(&w.insert_ids(), order, "row {to}");
        }
        // The plugin is the one loaded at the start, wherever its slot is now.
        let now = w.mini.of_song(&w.m);
        assert_eq!(now.len(), 1);
        assert_eq!(now[0].1, loaded[0].1, "row {to}: not loaded anew");
    }
}

#[test]
fn undo_redo_and_a_jump_say_what_the_rows_say() {
    let mut m = ready();
    assert_eq!(
        crate::play::tests::status(m_after(&mut m, Command::Undo)),
        Some(StatusMessage::NothingToUndo)
    );
    assert_eq!(
        crate::play::tests::status(m_after(&mut m, Command::Redo)),
        Some(StatusMessage::NothingToRedo)
    );
    cmd(&mut m, Command::SetTempo { bpm: 120.0 });
    cmd(&mut m, Command::ToggleMute { track: 1 });
    cmd(&mut m, Command::SetTempo { bpm: 90.0 });
    cmd(&mut m, Command::SetTempo { bpm: 100.0 });
    let rows = m.history_rows(1, 9);
    assert_eq!(rows.len(), 4);
    let status = |m: &Model| m.view().status.message;

    cmd(&mut m, Command::Undo);
    let undone = StatusMessage::Undone {
        name: rows[3].name.clone(),
        what: rows[3].what.clone(),
    };
    assert_eq!(status(&m), Some(undone));
    cmd(&mut m, Command::Redo);
    let redone = StatusMessage::Redone {
        name: rows[3].name.clone(),
        what: rows[3].what.clone(),
    };
    assert_eq!(status(&m), Some(redone));

    // Back to row 1, forward to row 3, and to where it is already.
    let jump = |index: u32| Command::HistoryJump {
        index,
        step: rows[index as usize - 1].step,
    };
    cmd(&mut m, jump(1));
    let jumped = |index: u32, steps, forward| StatusMessage::HistoryJumped {
        index,
        name: rows[index as usize - 1].name.clone(),
        steps,
        forward,
    };
    assert_eq!(status(&m), Some(jumped(1, 3, false)));
    assert_eq!(m.view().history.current, 1);
    assert_eq!(m.view().timeline.tempo_bpm, 120.0);
    assert!(!m.view().song.tracks[0].muted);
    cmd(&mut m, jump(3));
    assert_eq!(status(&m), Some(jumped(3, 2, true)));
    assert_eq!(m.view().timeline.tempo_bpm, 90.0);
    assert!(m.view().song.tracks[0].muted);
    let before = m.view();
    assert!(cmd(&mut m, jump(3)).is_empty());
    assert_eq!(m.view().history, before.history);
    assert!(!matches!(
        status(&m),
        Some(StatusMessage::HistoryJumped { .. })
    ));
    // The rows are the same ones wherever 「いまここ」 is.
    assert_eq!(m.history_rows(1, 9), rows);

    // A row that is no row, and a row that is another step by now.
    for (index, step) in [(0, rows[0].step), (5, rows[3].step), (2, rows[3].step)] {
        assert!(cmd(&mut m, Command::HistoryJump { index, step }).is_empty());
        assert_eq!(m.view().history, before.history);
    }
    cmd(&mut m, jump(2));
    cmd(&mut m, Command::SetTempo { bpm: 70.0 });
    assert_eq!(m.view().history.total, 3);
    assert!(cmd(&mut m, jump(3)).is_empty(), "row 3 is another step now");
    assert_eq!(m.view().history.current, 3);
}

fn m_after(m: &mut Model, command: Command) -> &Model {
    cmd(m, command);
    m
}

#[test]
fn no_jump_while_counting_in_or_recording() {
    let mut m = ready();
    cmd(&mut m, Command::SetTempo { bpm: 120.0 });
    cmd(&mut m, Command::SetTempo { bpm: 90.0 });
    let rows = m.history_rows(1, 2);
    let jump = Command::HistoryJump {
        index: 1,
        step: rows[0].step,
    };
    cmd(&mut m, Command::Record);
    assert_eq!(
        m.view().transport.state,
        meloet_protocol::TransportState::CountIn
    );
    cmd(&mut m, jump.clone());
    assert_eq!(m.view().history.current, 2);
    assert_eq!(m.view().timeline.tempo_bpm, 90.0);
    crate::play::tests::started(&mut m);
    let recording = meloet_protocol::TransportState::Recording;
    crate::play::tests::hear(&mut m, 1, 12.0, recording, 0);
    assert_eq!(m.view().transport.state, recording);
    cmd(&mut m, jump.clone());
    assert_eq!(m.view().history.current, 2);
    cmd(&mut m, Command::Record);
    let stopped = crate::play::RecordReport::Stopped {
        t_us: 100,
        tick: 400.0,
        lap: 0,
    };
    crate::play::tests::report(&mut m, stopped);
    assert_eq!(m.view().history.current, 2, "nothing was played");
    cmd(&mut m, jump);
    assert_eq!(m.view().history.current, 1);
    assert_eq!(m.view().timeline.tempo_bpm, 120.0);
}

#[test]
fn the_step_of_a_plugins_parameter_is_named_with_the_parameter() {
    let mut r = Rig::new();
    let (_, track, slot) = r.surge();
    r.cmd(Command::SelectInstrument { track });
    r.settle();
    let table = r.selected().table.expect("its table");
    let first = r.m.plugin_params(table).unwrap().groups[0].params[0].clone();
    let steps = r.m.view().history.total;
    let set = Command::SetPluginParam {
        slot,
        table,
        param: first.id,
        value: 0.9,
        commit: true,
    };
    r.cmd(set);
    r.settle();
    assert_eq!(r.m.view().history.total, steps + 1);
    let row = r.m.history_rows(steps + 1, steps + 1).remove(0);
    let named = StepName::SetPluginParam {
        plugin: "Surge XT".into(),
        param: Some(first.name),
    };
    assert_eq!(row.name, named);
    // Undo says it as the row does.
    r.cmd(Command::Undo);
    let undone = StatusMessage::Undone {
        name: named.clone(),
        what: StepWhat::None,
    };
    assert_eq!(r.m.view().status.message, Some(undone));
    assert_eq!(r.m.history_rows(steps + 1, steps + 1)[0].name, named);
    // When another step takes its place, what was noted of it goes.
    r.settle();
    r.cmd(Command::SetTempo { bpm: 77.0 });
    assert_eq!(
        r.m.history_rows(steps + 1, steps + 1)[0].name,
        StepName::SetTempo
    );
    assert!(r.m.play().step_notes.param_names.is_empty());
}

/// A song with 5000 steps: how long the rows take, and a jump over 500 steps.
/// `cargo test --release -p meloet-core --lib -- --ignored --nocapture five_thousand`
#[test]
#[ignore = "a measurement; run it in release"]
fn five_thousand_steps_rows_and_a_jump_over_500() {
    let mut m = ready();
    // A clip with 2000 notes, then steps of every day: notes moved, faders, markers, tempo.
    for n in 0..2000u64 {
        let take = m.view().piano_roll.take;
        cmd(
            &mut m,
            Command::AddNote {
                take,
                start: (n % 64) * (Q / 4),
                length: Q / 4,
                key: 36 + (n % 48) as u8,
                velocity: 90,
            },
        );
    }
    let mut n = 0u64;
    while m.view().history.total < 5000 {
        n += 1;
        let take = m.view().piano_roll.take.expect("the clip");
        match n % 5 {
            0 | 1 => cmd(
                &mut m,
                Command::MoveNotes {
                    take,
                    notes: vec![(n % 1500) as u32],
                    ticks: 0,
                    keys: if n % 4 < 2 { 1 } else { -1 },
                    copy: false,
                },
            ),
            2 => cmd(
                &mut m,
                Command::SetGain {
                    strip: StripRef::Track { id: 1 },
                    db: Some(-((n % 30) as f32)),
                    commit: true,
                },
            ),
            3 => cmd(
                &mut m,
                Command::SetTempo {
                    bpm: 60.0 + (n % 100) as f64,
                },
            ),
            _ => {
                cmd(&mut m, Command::Locate { tick: (n % 64) * Q });
                cmd(
                    &mut m,
                    Command::AddMarker {
                        name: format!("m{n}"),
                        tick: None,
                    },
                );
                cmd(&mut m, Command::CloseMarkerName)
            }
        };
    }
    let total = m.view().history.total;
    assert_eq!(total, 5000);

    let time = |what: &str, mut f: Box<dyn FnMut() + '_>| {
        let mut times = Vec::new();
        for _ in 0..9 {
            let at = Instant::now();
            f();
            times.push(at.elapsed());
        }
        times.sort();
        eprintln!("{what}: median {:?}, max {:?}", times[4], times[8]);
    };
    time(
        "history_rows, the last 100 of 5000",
        Box::new(|| {
            assert_eq!(m.history_rows(4901, 5000).len(), 100);
        }),
    );
    time(
        "history_rows, 100 in the middle",
        Box::new(|| {
            assert_eq!(m.history_rows(2401, 2500).len(), 100);
        }),
    );
    let step_of = |m: &Model, index: u32| m.history_rows(index, index)[0].step;
    let (low, high) = (step_of(&m, 4500), step_of(&m, 5000));
    let mut back = true;
    time(
        "a jump over 500 steps (back, forward in turn)",
        Box::new(|| {
            let (index, step) = if back { (4500, low) } else { (5000, high) };
            back = !back;
            let effects = cmd(&mut m, Command::HistoryJump { index, step });
            assert!(!effects.is_empty());
            assert_eq!(m.view().history.current, index);
        }),
    );
    assert_eq!(m.view().history.current, 4500);
    let at = Instant::now();
    for _ in 0..500 {
        cmd(&mut m, Command::Redo);
    }
    eprintln!("500 single steps (Redo): {:?}", at.elapsed());
    let at = Instant::now();
    for _ in 0..500 {
        cmd(&mut m, Command::Undo);
    }
    eprintln!("500 single steps (Undo): {:?}", at.elapsed());
    let at = Instant::now();
    let _ = m.view();
    eprintln!("one view of the state: {:?}", at.elapsed());
}
