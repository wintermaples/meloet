use std::collections::{BTreeSet, HashMap};

use meloet_model::{Doc, InstrumentRef, StripId, TrackId};
use meloet_protocol::{
    Band, Command, EffectKind, EffectRef, InsertRef, InstrumentKind, ParamRef, PluginSlotRef,
    SelectedParams, SendBus, ShelfMode, SoundRef, StripRef,
};

use super::*;
use crate::catalog::tests::{COMP_LV2, COMP_VST3, DRUMS_LV2, SURGE_LV2, SURGE_VST3};
use crate::play::seats::tests::FakeEngine;
use crate::sf2store::Sf2Key;
use crate::state::added_tests::{plugin, rigged, sf2};
use crate::state::{Input, Model};

/// What the engine holds where an instrument goes.
#[derive(Debug, Clone, PartialEq)]
enum Held {
    /// Made by the core thread from what the op names (silent for a plugin).
    Made(InstrumentRef),
    /// The plugin loaded under this ticket.
    Plug(u64),
}

/// What the engine and the plugins' thread do with the effects, as far as plugins go: where
/// each loaded plugin is, and which loads wait for an answer.
struct Mini {
    /// The strips, as far as the song can be compared with them.
    strips: FakeEngine,
    sounding: HashMap<usize, Held>,
    /// The seat's own instrument, set aside under a trial.
    aside: HashMap<usize, Held>,
    /// The insert slots that hold a plugin.
    inserts: HashMap<(StripSeat, usize), u64>,
    sample: Option<u64>,
    /// Loaded, and waiting to be put into the engine or let go of.
    waiting: BTreeSet<u64>,
    /// Loads asked for and not answered yet.
    asked: Vec<u64>,
    sf2_asked: Vec<Sf2Key>,
    /// How many plugins were put into the engine.
    plugged: u32,
}

impl Mini {
    fn new(m: &mut Model) -> Mini {
        let mut mini = Mini {
            strips: FakeEngine::new(m),
            sounding: HashMap::new(),
            aside: HashMap::new(),
            inserts: HashMap::new(),
            sample: None,
            waiting: BTreeSet::new(),
            asked: Vec::new(),
            sf2_asked: Vec::new(),
            plugged: 0,
        };
        for op in m.engine_seats() {
            mini.op(op);
        }
        mini
    }

    fn take(&mut self, effects: &[Effect]) {
        self.strips.take(effects);
        for effect in effects {
            match effect {
                Effect::Engine(op) => self.op(op.clone()),
                Effect::LoadPlugin { ticket, .. } | Effect::ReloadPlugin { ticket } => {
                    self.asked.push(*ticket);
                }
                Effect::DropPlugin { ticket } => {
                    self.waiting.remove(ticket);
                }
                Effect::LoadSf2(key) => self.sf2_asked.push(key.clone()),
                _ => {}
            }
        }
    }

    fn op(&mut self, op: EngineOp) {
        match op {
            EngineOp::Instrument { seat, instrument } => {
                self.sounding.insert(seat, Held::Made(instrument));
            }
            EngineOp::TryInstrument { seat, instrument } => {
                self.tried(seat, Held::Made(instrument))
            }
            EngineOp::EndTrial { seat, keep } => {
                if let Some(own) = self.aside.remove(&seat)
                    && !keep
                {
                    self.sounding.insert(seat, own);
                }
            }
            EngineOp::MoveInstrument { from, to } => {
                assert!(!self.sounding.contains_key(&to), "moved onto seat {to}");
                let moved = self.sounding.remove(&from).expect("nothing to move");
                self.sounding.insert(to, moved);
            }
            EngineOp::ClearSeat { seat } => {
                self.sounding.remove(&seat);
                self.aside.remove(&seat);
                self.inserts
                    .retain(|(strip, _), _| *strip != StripSeat::Track(seat));
            }
            EngineOp::Insert { strip, slot, .. } => {
                self.inserts.remove(&(strip, slot));
            }
            EngineOp::MoveInsert { strip, from, to } => {
                let held = std::mem::take(&mut self.inserts);
                self.inserts = (held.into_iter())
                    .map(|((s, slot), ticket)| match s == strip {
                        true => ((s, moved_slot(slot, from, to)), ticket),
                        false => ((s, slot), ticket),
                    })
                    .collect();
            }
            EngineOp::Plug { ticket, spot } => {
                assert!(self.waiting.remove(&ticket), "plugged what is not loaded");
                self.plugged += 1;
                match spot {
                    Spot::Seat(seat) => {
                        assert!(!self.aside.contains_key(&seat), "put under a trial");
                        self.sounding.insert(seat, Held::Plug(ticket));
                    }
                    Spot::Trial(seat) => self.tried(seat, Held::Plug(ticket)),
                    Spot::Insert(strip, slot) => {
                        self.inserts.insert((strip, slot), ticket);
                    }
                    Spot::Sample => panic!("a sample comes with its phrase"),
                }
            }
            EngineOp::PlugSample { ticket, .. } => {
                assert!(self.waiting.remove(&ticket), "plugged what is not loaded");
                self.plugged += 1;
                self.sample = Some(ticket);
            }
            EngineOp::PlaySample { .. } | EngineOp::StopSample => self.sample = None,
            _ => {}
        }
    }

    fn tried(&mut self, seat: usize, tried: Held) {
        let before = self.sounding.insert(seat, tried);
        (self.aside.entry(seat)).or_insert_with(|| before.expect("a trial on an empty seat"));
    }

    /// Every plugin in the engine, with where it is.
    fn in_engine(&self) -> Vec<(Spot, u64)> {
        let mut held = Vec::new();
        for (&seat, sounding) in &self.sounding {
            if let Held::Plug(ticket) = sounding {
                let tried = self.aside.contains_key(&seat);
                let spot = if tried {
                    Spot::Trial(seat)
                } else {
                    Spot::Seat(seat)
                };
                held.push((spot, *ticket));
            }
        }
        for (&seat, own) in &self.aside {
            if let Held::Plug(ticket) = own {
                held.push((Spot::Seat(seat), *ticket));
            }
        }
        for (&(strip, slot), &ticket) in &self.inserts {
            held.push((Spot::Insert(strip, slot), ticket));
        }
        held.extend(self.sample.map(|ticket| (Spot::Sample, ticket)));
        held
    }

    /// The book and the engine agree, and no loaded plugin is forgotten.
    fn check(&self, m: &Model, ctx: &str) {
        self.strips.check_mixer(m, ctx);
        let p = m.play();
        let book = &p.plugs;
        for ticket in &self.waiting {
            let plug = (book.list.iter().find(|plug| plug.ticket == *ticket))
                .unwrap_or_else(|| panic!("{ctx}: {ticket} is loaded and forgotten"));
            assert!(
                !plug.placed && plug.phase != Phase::Loading,
                "{ctx}: {plug:?} waits"
            );
        }
        let held = self.in_engine();
        for (spot, ticket) in &held {
            let plug = (book.at(*spot))
                .unwrap_or_else(|| panic!("{ctx}: {ticket} is at {spot:?}, unknown to the book"));
            assert_eq!(plug.ticket, *ticket, "{ctx}: at {spot:?}");
        }
        for plug in book.list.iter().filter(|plug| plug.placed) {
            assert!(
                held.contains(&(plug.spot, plug.ticket)),
                "{ctx}: {plug:?} is not in the engine"
            );
        }
        let doc = &p.song().doc;
        for (t, seat) in doc.tracks.iter().zip(p.track_seats()) {
            let seat = seat.expect("a seat for every track");
            let own = (self.aside.get(&seat))
                .or(self.sounding.get(&seat))
                .unwrap_or_else(|| panic!("{ctx}: nothing on seat {seat}"));
            match &t.instrument {
                InstrumentRef::Plugin(plugin) => {
                    let plug = (book.at(Spot::Seat(seat)))
                        .unwrap_or_else(|| panic!("{ctx}: track {} has no plugin", t.id.0));
                    assert_eq!(plug.plugin.key, plugin.key, "{ctx}");
                    if plug.placed {
                        assert_eq!(own, &Held::Plug(plug.ticket), "{ctx}: seat {seat}");
                    } else {
                        let waits = matches!(own, Held::Made(InstrumentRef::Plugin(_)))
                            || *own == Held::Plug(plug.ticket);
                        assert!(waits, "{ctx}: seat {seat} holds {own:?}");
                    }
                }
                other => assert_eq!(own, &Held::Made(other.clone()), "{ctx}: seat {seat}"),
            }
            self.check_slots(m, StripSeat::Track(seat), &t.channel.inserts, ctx);
        }
        for bus in meloet_model::Bus::ALL {
            let inserts = &doc.returns[bus.index()].channel.inserts;
            self.check_slots(m, StripSeat::Return(bus), inserts, ctx);
        }
        self.check_slots(m, StripSeat::Master, &doc.master.inserts, ctx);
        // What the panel shows is in the song, where the panel says it is.
        match m.view().params.selected {
            Some(SelectedParams::Builtin(insert)) => {
                let (strip, slot) = (strip_id(insert.insert.strip), insert.insert.slot);
                let held = doc.insert(strip, usize::from(slot));
                let kind = held
                    .and_then(|i| i.kind())
                    .map(crate::play::mixer::effect_kind);
                assert_eq!(kind, Some(insert.effect), "{ctx}: the selected insert");
            }
            Some(SelectedParams::Plugin(plugin)) => {
                let place = Play::slot_place(plugin.slot);
                assert!(doc.plugin(place).is_some(), "{ctx}: the selected plugin");
            }
            None => {}
        }
    }

    /// Every plugin the song has in a slot of the strip is the one the book has there.
    fn check_slots(&self, m: &Model, strip: StripSeat, inserts: &meloet_model::Inserts, ctx: &str) {
        for (slot, insert) in inserts.iter().enumerate() {
            let spot = Spot::Insert(strip, slot);
            let plugin = insert.as_ref().and_then(|i| i.plugin_ref());
            assert_eq!(
                m.play().plugs.at(spot).map(|plug| &plug.plugin.key),
                plugin.map(|plugin| &plugin.key),
                "{ctx}: {spot:?}"
            );
        }
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn of<T: Clone>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize].clone()
    }
}

/// Every insert slot of the song that holds something, with the effect of meloet it holds.
fn filled_slots(doc: &Doc) -> Vec<(InsertRef, Option<EffectKind>)> {
    let tracks = doc.tracks.iter().map(|t| {
        let strip = StripRef::Track { id: t.id.0 };
        (strip, &t.channel.inserts)
    });
    let returns = [SendBus::A, SendBus::B]
        .into_iter()
        .zip(&doc.returns)
        .map(|(bus, r)| (StripRef::Return { bus }, &r.channel.inserts));
    let mut filled = Vec::new();
    for (strip, inserts) in tracks
        .chain(returns)
        .chain([(StripRef::Master, &doc.master.inserts)])
    {
        for (slot, insert) in inserts.iter().enumerate() {
            if let Some(insert) = insert {
                let at = InsertRef {
                    strip,
                    slot: slot as u8,
                };
                let kind = insert.kind().map(crate::play::mixer::effect_kind);
                filled.push((at, kind));
            }
        }
    }
    filled
}

fn held(doc: &Doc) -> Doc {
    Doc {
        next_id: 0,
        ..doc.clone()
    }
}

fn go(m: &mut Model, mini: &mut Mini, input: Input, ctx: &str) {
    let effects = m.handle(input.clone());
    mini.take(&effects);
    mini.check(m, &format!("{ctx}: {input:?}"));
}

/// Answers the load asked for at `at`: the plugin is loaded, or with `fails` it is not.
fn answer(m: &mut Model, mini: &mut Mini, at: usize, fails: bool, ctx: &str) {
    let ticket = mini.asked.remove(at);
    if fails {
        go(m, mini, Input::PluginUnreadable { ticket }, ctx);
    } else {
        mini.waiting.insert(ticket);
        go(m, mini, Input::PluginLoaded { ticket }, ctx);
    }
}

#[test]
fn random_plugins_and_presets_with_undo_and_redo_keep_the_engine_with_the_book() {
    let sounds = [
        SoundRef::Builtin {
            instrument: InstrumentKind::Sine,
        },
        SoundRef::Builtin {
            instrument: InstrumentKind::SynthLead,
        },
        sf2(0, 0),
        sf2(8, 24),
        sf2(128, 0),
        plugin(SURGE_VST3),
        plugin(SURGE_VST3),
        plugin(SURGE_LV2),
        plugin(DRUMS_LV2),
    ];
    let effects = [
        Some(EffectRef::Plugin {
            key: COMP_LV2.into(),
        }),
        Some(EffectRef::Plugin {
            key: COMP_VST3.into(),
        }),
        Some(EffectRef::Builtin {
            effect: EffectKind::Reverb,
        }),
    ];
    let (mut plugged, mut plugin_tracks, mut plugin_inserts, mut steps) = (0, 0, 0, 0);
    let (mut passed, mut moved, mut replaced, mut typed, mut other) = (0, 0, 0, 0, 0);
    let texts = ["4", "−3 dB", "999", "x", "0.5 s", "1"];
    for seed in 1..=10u64 {
        let rng = &mut Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let m = &mut rigged();
        let mini = &mut Mini::new(m);
        mini.check(m, "first");
        for round in 0..500 {
            let ctx = format!("seed {seed}, round {round}");
            let ids: Vec<u32> = (m.play().song().doc.tracks.iter())
                .map(|t| t.id.0)
                .collect();
            let track = match (ids.len(), rng.below(10)) {
                (0, _) | (_, 0) => 9_999,
                _ => rng.of(&ids),
            };
            let strip = match rng.below(5) {
                0 => StripRef::Master,
                1 => StripRef::Return { bus: SendBus::A },
                _ => StripRef::Track { id: track },
            };
            let insert = InsertRef {
                strip,
                slot: rng.below(4) as u8,
            };
            let slot = match rng.below(2) {
                0 => PluginSlotRef::Instrument { track },
                _ => PluginSlotRef::Insert { insert },
            };
            // For what is done to a slot that holds something: mostly one that does.
            let filled = filled_slots(&m.play().song().doc);
            let (held, held_kind) = match rng.below(4) {
                0 => (insert, None),
                _ if filled.is_empty() => (insert, None),
                _ => rng.of(&filled),
            };
            let mut commands = Vec::new();
            match rng.below(30) {
                0..=2 => {
                    // A row is tried for a new track or in place of a track's sound; now and
                    // then its load is answered before Enter, and now and then Enter is not
                    // pressed.
                    let mode = match rng.below(2) {
                        0 => ShelfMode::NewTrack,
                        _ => ShelfMode::Replace { track },
                    };
                    go(m, mini, Input::Command(Command::OpenShelf { mode }), &ctx);
                    let sound = Some(rng.of(&sounds));
                    let select = Command::ShelfSelectSound { sound };
                    go(m, mini, Input::Command(select), &ctx);
                    if rng.below(3) > 0 && !mini.asked.is_empty() {
                        let last = mini.asked.len() - 1;
                        answer(m, mini, last, rng.below(8) == 0, &ctx);
                    }
                    if rng.below(4) > 0 {
                        commands.push(Command::ShelfConfirm);
                    }
                }
                3 => commands.push(match rng.below(3) {
                    0 => Command::ShelfCancel,
                    1 => Command::Back,
                    _ => Command::ToggleBand { band: Band::Shelf },
                }),
                4 | 5 => {
                    let mode = ShelfMode::InsertEffect {
                        strip,
                        slot: insert.slot,
                    };
                    commands.push(Command::OpenShelf { mode });
                    let effect = rng.of(&effects);
                    commands.push(Command::ShelfChooseEffect { effect });
                    commands.push(Command::ShelfConfirm);
                }
                6 => commands.push(Command::RemoveEffect { insert }),
                7 => commands.push(Command::DeleteTrack { track }),
                8..=10 => commands.push(Command::Undo),
                11 => commands.push(Command::Redo),
                12 | 13 => {
                    if !mini.asked.is_empty() {
                        let at = rng.below(mini.asked.len() as u64) as usize;
                        answer(m, mini, at, rng.below(6) == 0, &ctx);
                    }
                }
                14 => {
                    // A plugin in the engine stops, with every other one of its process (here:
                    // only itself).
                    let held = mini.in_engine();
                    if !held.is_empty() {
                        let (_, ticket) = rng.of(&held);
                        go(m, mini, Input::PluginStopped { ticket }, &ctx);
                    }
                }
                15 => commands.push(Command::ReloadPlugin { slot }),
                16 => commands.push(Command::RemovePlugin { slot }),
                17 => {
                    for key in std::mem::take(&mut mini.sf2_asked) {
                        go(m, mini, Input::Sf2Loaded(key), &ctx);
                    }
                }
                18 => commands.push(match rng.below(3) {
                    0 => Command::StopSample,
                    _ => Command::PlaySampleOf {
                        sound: rng.of(&sounds),
                    },
                }),
                20..=22 => commands.push(Command::ToggleBypass { insert: held }),
                23..=25 => commands.push(Command::MoveInsert {
                    insert: held,
                    to: rng.below(5) as u8,
                }),
                26 => {
                    let insert = held;
                    let mode = ShelfMode::ReplaceEffect {
                        strip: insert.strip,
                        slot: insert.slot,
                    };
                    commands.push(Command::OpenShelf { mode });
                    let effect = match rng.below(3) {
                        0 => Some(EffectRef::Builtin {
                            effect: EffectKind::Chorus,
                        }),
                        _ => rng.of(&effects),
                    };
                    commands.push(Command::ShelfChooseEffect { effect });
                    // Now and then the slot moves before Enter.
                    if rng.below(3) == 0 {
                        let to = rng.below(4) as u8;
                        commands.push(Command::MoveInsert { insert, to });
                    }
                    commands.push(Command::ShelfConfirm);
                }
                27 => {
                    let effect = held_kind.unwrap_or(EffectKind::Reverb);
                    let param = ParamRef::Builtin {
                        insert: held,
                        effect,
                        param: rng.below(8) as u32,
                    };
                    let text = rng.of(&texts).to_owned();
                    commands.push(Command::TypeParam { param, text });
                }
                28 => commands.push(match rng.below(3) {
                    0 => Command::SelectInstrument { track },
                    1 => Command::SelectInsert { insert: None },
                    _ => Command::SelectInsert { insert: Some(held) },
                }),
                _ => {
                    let fader = Command::SetGain {
                        strip,
                        db: Some(-(rng.below(12) as f32)),
                        commit: true,
                    };
                    commands.push(fader);
                }
            }
            for command in commands {
                let before = m.view();
                let replaces = matches!(before.shelf.mode, ShelfMode::ReplaceEffect { .. });
                let count = match command {
                    Command::ToggleBypass { .. } => &mut passed,
                    Command::MoveInsert { .. } => &mut moved,
                    Command::TypeParam { .. } => &mut typed,
                    Command::ShelfConfirm if replaces => &mut replaced,
                    _ => &mut other,
                };
                go(m, mini, Input::Command(command), &ctx);
                if m.view().song.undo_count > before.song.undo_count {
                    *count += 1;
                }
            }
        }
        // Everything is answered, then every step is taken back and made again.
        go(m, mini, Input::Command(Command::ShelfCancel), "end");
        while !mini.asked.is_empty() {
            answer(m, mini, 0, false, "end");
        }
        let doc = &m.play().song().doc;
        plugin_tracks += (doc.tracks.iter())
            .filter(|t| matches!(t.instrument, InstrumentRef::Plugin(_)))
            .count();
        plugin_inserts += m.plugin_tickets().len();
        let last = held(doc);
        let count = m.view().song.undo_count;
        steps += count;
        for _ in 0..count {
            go(m, mini, Input::Command(Command::Undo), "undo all");
        }
        assert_eq!(m.view().song.undo_count, 0);
        for _ in 0..count {
            go(m, mini, Input::Command(Command::Redo), "redo all");
        }
        assert_eq!(held(&m.play().song().doc), last, "seed {seed}");
        while !mini.asked.is_empty() {
            answer(m, mini, 0, false, "after redo");
        }
        // Every plugin of the song that this PC has is loaded and in the engine again.
        let p = m.play();
        for plug in &p.plugs.list {
            assert!(
                plug.placed || plug.phase != Phase::Ready,
                "seed {seed}: {plug:?} waits for nothing"
            );
        }
        assert!(mini.waiting.is_empty(), "seed {seed}: {:?}", mini.waiting);
        plugged += mini.plugged;
    }
    // The test did what it is for.
    assert!(plugged > 300, "{plugged} plugins were put into the engine");
    assert!(plugin_tracks > 5 && plugin_inserts > 10 && steps > 300);
    assert!(
        passed > 100 && moved > 100 && replaced > 20 && typed > 10 && other > 300,
        "{passed} passed by, {moved} moved, {replaced} replaced, {typed} typed"
    );
    let _ = (StripId::Master, TrackId(0));
}
