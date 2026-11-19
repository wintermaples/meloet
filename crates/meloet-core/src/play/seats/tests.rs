use std::collections::{HashMap, HashSet};

use meloet_model::mixer::new_track_sends;
use meloet_model::playback::{PlayEvent, track_events};
use meloet_model::{Bus, Channel, Doc, Gain, Insert, InstrumentRef, Pan, Sends, StripId, TrackId};
use meloet_protocol::{
    Band, Command, EffectKind, InsertRef, InstrumentKind, SendBus, ShelfMode, StripRef,
};

use super::{AUDITION_SEAT, MAX_TRACKS};
use crate::added::builtin_or_sine;
use crate::instruments;
use crate::play::mix::tests::three_tracks;
use crate::play::mixer::{effect_kind, strip_ref};
use crate::play::tests::{cmd, ready, record_once};
use crate::play::{EngineOp, Preview, StripSeat};
use crate::songfile;
use crate::state::{Effect, Input, Model};

/// An instrument as the engine holds it: which one it is (every one made gets the next
/// number) and its kind.
type Made = (u32, InstrumentRef);

#[derive(Debug, Default, Clone)]
struct Seat {
    instrument: Option<Made>,
    aside: Option<Made>,
    events: Option<Vec<PlayEvent>>,
    strip: Strip,
}

/// A strip as the engine holds it. Every effect made gets the next number.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Strip {
    pub gain: Gain,
    pub pan: Pan,
    pub sends: Sends,
    pub inserts: [Option<(u32, Insert)>; 4],
}

impl Default for Strip {
    fn default() -> Strip {
        Strip {
            gain: Gain::UNITY,
            pan: Pan::CENTER,
            sends: [Gain::SILENT; 2],
            inserts: Default::default(),
        }
    }
}

type StripValues = (Gain, Pan, Sends, [Option<Insert>; 4]);

impl Strip {
    fn values(&self) -> StripValues {
        let inserts = self.inserts.clone().map(|i| i.map(|(_, insert)| insert));
        (self.gain, self.pan, self.sends, inserts)
    }
}

/// What a strip is to sound like: the song's values under the previews.
fn wanted(m: &Model, id: StripId, channel: &Channel, sends: Sends, muted: bool) -> StripValues {
    let (mut gain, mut pan, mut sends) = (channel.gain, channel.pan, sends);
    let mut inserts = channel.inserts.clone();
    for p in m.play().mix_previews() {
        match *p {
            Preview::Gain(strip, g) if strip == id => gain = g,
            Preview::Pan(strip, p) if strip == id => pan = p,
            Preview::Send(track, bus, g) if StripId::Track(track) == id => {
                sends[bus.index()] = g;
            }
            Preview::Param {
                strip,
                slot,
                kind,
                param,
                value,
            } if strip == id => {
                if let Some(insert) = inserts[slot].as_mut().filter(|i| i.kind() == Some(kind)) {
                    insert.set(param, value);
                }
            }
            _ => {}
        }
    }
    if muted {
        gain = Gain::SILENT;
    }
    (gain, pan, sends, inserts)
}

/// What the engine does with the ops, as far as the song can be compared with it.
pub(crate) struct FakeEngine {
    seats: Vec<Seat>,
    returns: [Strip; 2],
    master: Strip,
    target: Option<usize>,
    silent: Vec<usize>,
    made: u32,
    /// Effects made or put away.
    pub effects_made: u32,
}

impl FakeEngine {
    /// The engine as the core thread sets it up for `m`, with a stream just opened.
    pub fn new(m: &mut Model) -> FakeEngine {
        let mut engine = FakeEngine {
            seats: vec![Seat::default(); MAX_TRACKS + 1],
            returns: Default::default(),
            master: Strip::default(),
            target: Some(0),
            silent: Vec::new(),
            made: 0,
            effects_made: 0,
        };
        for op in m.engine_seats() {
            engine.op(op);
        }
        let opened = m.handle(Input::AudioOpened {
            node_name: "alsa_output.card.analog-stereo".into(),
            sample_rate: 48_000,
        });
        engine.take(&opened);
        engine
    }

    pub fn take(&mut self, effects: &[Effect]) {
        for e in effects {
            if let Effect::Engine(op) = e {
                self.op(op.clone());
            }
        }
    }

    fn op(&mut self, op: EngineOp) {
        match op {
            EngineOp::Instrument { seat, instrument } => {
                self.made += 1;
                self.seats[seat].instrument = Some((self.made, instrument));
            }
            EngineOp::TryInstrument { seat, instrument } => {
                self.made += 1;
                let slot = &mut self.seats[seat];
                let old = slot.instrument.replace((self.made, instrument));
                if slot.aside.is_none() {
                    slot.aside = old;
                }
            }
            EngineOp::EndTrial { seat, keep } => {
                let slot = &mut self.seats[seat];
                if let Some(own) = slot.aside.take()
                    && !keep
                {
                    slot.instrument = Some(own);
                }
            }
            EngineOp::MoveInstrument { from, to } => {
                assert!(self.seats[to].instrument.is_none(), "moved onto seat {to}");
                assert!(self.seats[from].instrument.is_some(), "nothing on {from}");
                self.seats[to].instrument = self.seats[from].instrument.take();
                if self.target == Some(from) {
                    self.target = Some(to);
                }
            }
            EngineOp::ClearSeat { seat } => self.seats[seat] = Seat::default(),
            EngineOp::PlayTarget { seat } => self.target = seat,
            EngineOp::Sequence { track, events } => self.seats[track].events = Some(events),
            EngineOp::SilentTracks { tracks } => self.silent = tracks,
            EngineOp::Strip { strip, gain, pan } => {
                let strip = self.strip_mut(strip);
                assert_ne!((strip.gain, strip.pan), (gain, pan), "told what it has");
                (strip.gain, strip.pan) = (gain, pan);
            }
            EngineOp::Sends { seat, sends } => {
                assert_ne!(self.seats[seat].strip.sends, sends, "told what it has");
                self.seats[seat].strip.sends = sends;
            }
            EngineOp::Insert {
                strip,
                slot,
                insert,
            } => {
                self.effects_made += 1;
                let made = self.effects_made;
                let place = &mut self.strip_mut(strip).inserts[slot];
                let (before, now) = (place.as_ref().map(|(_, i)| i.kind()), insert.as_ref());
                assert_ne!(before, now.map(Insert::kind), "made anew for no reason");
                *place = insert.map(|i| (made, i));
            }
            EngineOp::Param {
                strip,
                slot,
                param,
                value,
            } => {
                let (_, insert) = self.strip_mut(strip).inserts[slot]
                    .as_mut()
                    .expect("a parameter for an empty slot");
                assert_ne!(insert.value(param), Some(value), "told what it has");
                assert!(insert.set(param, value));
                assert_eq!(insert.value(param), Some(value), "sent out of range");
            }
            _ => {}
        }
    }

    fn strip_mut(&mut self, strip: StripSeat) -> &mut Strip {
        match strip {
            StripSeat::Track(seat) => &mut self.seats[seat].strip,
            StripSeat::Return(bus) => &mut self.returns[bus.index()],
            StripSeat::Master => &mut self.master,
        }
    }

    /// The strip of a track, of a return or of the master, as the engine holds it.
    pub fn strip(&self, m: &Model, id: StripId) -> &Strip {
        match id {
            StripId::Track(track) => &self.seats[m.play().seats.of(track).unwrap()].strip,
            StripId::Return(bus) => &self.returns[bus.index()],
            StripId::Master => &self.master,
        }
    }

    pub fn audition_strip(&self) -> &Strip {
        &self.seats[AUDITION_SEAT].strip
    }

    /// The number of the track's own instrument: the one set aside while another is tried.
    fn own(&self, m: &Model, track: TrackId) -> Option<u32> {
        let slot = &self.seats[m.play().seats.of(track)?];
        slot.aside
            .as_ref()
            .or(slot.instrument.as_ref())
            .map(|i| i.0)
    }

    /// The number and the kind of the own instrument of every track of the song.
    pub fn owns(&self, m: &Model) -> HashMap<TrackId, (u32, InstrumentRef)> {
        let doc = &m.play().song().doc;
        doc.tracks
            .iter()
            .filter_map(|t| Some((t.id, (self.own(m, t.id)?, t.instrument.clone()))))
            .collect()
    }

    /// The seat the keys play.
    pub fn target(&self) -> Option<usize> {
        self.target
    }

    /// The kind of what sounds on a seat.
    pub fn sounding(&self, seat: usize) -> Option<InstrumentKind> {
        let (_, instrument) = self.seats[seat].instrument.as_ref()?;
        Some(instruments::kind_of(builtin_or_sine(instrument)))
    }

    /// The engine has what the song and the shelf say, and nothing else.
    pub fn check(&self, m: &Model, ctx: &str) {
        let p = m.play();
        let song = p.song();
        let doc = &song.doc;
        let shelf = m.view().shelf;
        let kind = |made: &Option<Made>| {
            made.as_ref()
                .map(|(_, i)| instruments::kind_of(builtin_or_sine(i)))
        };
        let of = |i: &InstrumentRef| Some(instruments::kind_of(builtin_or_sine(i)));
        let mut used = HashSet::new();
        for (t, seat) in doc.tracks.iter().zip(p.track_seats()) {
            let seat = seat.unwrap_or_else(|| panic!("{ctx}: track {} has no seat", t.id.0));
            assert!(seat < MAX_TRACKS && used.insert(seat), "{ctx}: seat {seat}");
            let slot = &self.seats[seat];
            let tried = shelf
                .selected
                .filter(|_| shelf.mode == ShelfMode::Replace { track: t.id.0 });
            match tried {
                Some(tried) => {
                    assert_eq!(kind(&slot.instrument), Some(tried), "{ctx}: tried");
                    assert_eq!(kind(&slot.aside), of(&t.instrument), "{ctx}: set aside");
                }
                None => {
                    assert_eq!(
                        kind(&slot.instrument),
                        of(&t.instrument),
                        "{ctx}: seat {seat}"
                    );
                    assert!(slot.aside.is_none(), "{ctx}: nothing set aside on {seat}");
                }
            }
            assert_eq!(
                slot.events.clone().unwrap_or_default(),
                track_events(doc, t.id),
                "{ctx}: the notes of track {}",
                t.id.0
            );
            assert_eq!(
                slot.strip.values(),
                wanted(m, StripId::Track(t.id), &t.channel, t.sends, false),
                "{ctx}: the strip of track {}",
                t.id.0
            );
        }
        let none = [Gain::SILENT; 2];
        for bus in Bus::ALL {
            let r = &doc.returns[bus.index()];
            assert_eq!(
                self.returns[bus.index()].values(),
                wanted(m, StripId::Return(bus), &r.channel, none, r.muted),
                "{ctx}: return {bus:?}"
            );
        }
        assert_eq!(
            self.master.values(),
            wanted(m, StripId::Master, &doc.master, none, false),
            "{ctx}: the master"
        );
        for (seat, slot) in self.seats.iter().enumerate().take(MAX_TRACKS) {
            if !used.contains(&seat) {
                assert!(
                    slot.instrument.is_none() && slot.aside.is_none() && slot.events.is_none(),
                    "{ctx}: seat {seat} is empty"
                );
                assert_eq!(slot.strip, Strip::default(), "{ctx}: seat {seat} is empty");
            }
        }
        let audition = &self.seats[AUDITION_SEAT];
        assert!(
            audition.aside.is_none() && audition.events.is_none(),
            "{ctx}"
        );
        let target = match (shelf.mode, shelf.selected) {
            (ShelfMode::NewTrack, Some(tried)) => {
                assert_eq!(kind(&audition.instrument), Some(tried), "{ctx}: auditioned");
                let tried = InstrumentRef::Builtin(instruments::builtin_of(tried));
                let as_a_track = Strip {
                    sends: new_track_sends(&tried),
                    ..Strip::default()
                };
                assert_eq!(audition.strip, as_a_track, "{ctx}: auditioned");
                Some(AUDITION_SEAT)
            }
            (mode, tried) => {
                assert!(audition.instrument.is_none(), "{ctx}: the audition seat");
                assert_eq!(audition.strip, Strip::default(), "{ctx}: the audition seat");
                let track = match (mode, tried) {
                    (ShelfMode::Replace { track }, Some(_)) => Some(TrackId(track)),
                    _ => song.session.play_target,
                };
                track.and_then(|t| p.seats.of(t))
            }
        };
        assert_eq!(self.target, target, "{ctx}: where the keys sound");
        let mut silent: Vec<usize> = doc
            .tracks
            .iter()
            .filter(|t| !doc.plays(t, &song.session.soloed))
            .filter_map(|t| p.seats.of(t.id))
            .collect();
        silent.sort_unstable();
        assert_eq!(self.silent, silent, "{ctx}: the silent tracks");
    }
}

fn track_ids(m: &Model) -> Vec<u32> {
    m.view().song.tracks.iter().map(|t| t.id).collect()
}

#[test]
fn a_track_keeps_its_seat_and_its_instrument_when_others_move_or_go() {
    let m = &mut ready();
    let mut engine = FakeEngine::new(m);
    engine.take(&three_tracks(m));
    engine.check(m, "opened");
    assert_eq!(m.track_seats(), [Some(0), Some(1), Some(2)]);
    let first = engine.owns(m);
    let step = |m: &mut Model, engine: &mut FakeEngine, command: Command| {
        let ctx = format!("{command:?}");
        let effects = cmd(m, command);
        engine.take(&effects);
        engine.check(m, &ctx);
    };

    step(m, &mut engine, Command::MoveTrack { track: 3, to: 0 });
    assert_eq!(track_ids(m), [3, 1, 2]);
    assert_eq!(m.track_seats(), [Some(2), Some(0), Some(1)]);
    assert_eq!(engine.owns(m), first, "the same instruments, not new ones");

    step(m, &mut engine, Command::DeleteTrack { track: 1 });
    assert_eq!(m.track_seats(), [Some(2), Some(1)]);
    for command in [
        Command::RenameTrack {
            track: 2,
            name: "うら".into(),
        },
        Command::SetTrackColor { track: 3, color: 5 },
        Command::ToggleMute { track: 2 },
        Command::ToggleSolo { track: 3 },
        Command::MoveTrack { track: 2, to: 0 },
    ] {
        step(m, &mut engine, command);
    }
    let left = engine.owns(m);
    assert_eq!(left[&TrackId(2)], first[&TrackId(2)]);
    assert_eq!(left[&TrackId(3)], first[&TrackId(3)]);

    // Undo back to the three tracks: the two that stayed still have their instruments, the
    // one that was gone has a new one, on the seat that was free.
    for _ in 0..6 {
        step(m, &mut engine, Command::Undo);
    }
    assert_eq!(track_ids(m), [1, 2, 3]);
    assert_eq!(m.track_seats(), [Some(0), Some(1), Some(2)]);
    let back = engine.owns(m);
    assert_eq!(back[&TrackId(2)], first[&TrackId(2)]);
    assert_eq!(back[&TrackId(3)], first[&TrackId(3)]);
    assert_ne!(back[&TrackId(1)].0, first[&TrackId(1)].0);
    assert_eq!(back[&TrackId(1)].1, first[&TrackId(1)].1);
}

#[test]
fn the_keys_sound_on_the_seat_of_the_selected_track() {
    let m = &mut ready();
    let mut engine = FakeEngine::new(m);
    engine.check(m, "a new song");
    // Another song: its tracks take the first seats, with new instruments.
    let before = engine.owns(m);
    engine.take(&three_tracks(m));
    engine.check(m, "another song");
    assert_eq!(m.track_seats(), [Some(0), Some(1), Some(2)]);
    assert!(
        engine
            .owns(m)
            .values()
            .all(|now| before.values().all(|b| b.0 != now.0))
    );
    assert_eq!(engine.target(), Some(0));
    let select = |track| Command::SelectTrack {
        track,
        from_arm: false,
    };
    let effects = cmd(m, select(3));
    assert_eq!(
        effects,
        [Effect::Engine(EngineOp::PlayTarget { seat: Some(2) })]
    );
    engine.take(&effects);
    // The track moves; its seat, and so the target, stays.
    let effects = cmd(m, Command::MoveTrack { track: 3, to: 0 });
    assert_eq!(effects, []);
    // The selected track goes: the one that takes its place is played.
    engine.take(&cmd(m, Command::DeleteTrack { track: 3 }));
    engine.check(m, "deleted");
    assert_eq!(engine.target(), Some(0));
    engine.take(&cmd(m, Command::DeleteTrack { track: 1 }));
    engine.take(&cmd(m, Command::DeleteTrack { track: 2 }));
    engine.check(m, "no tracks");
    assert_eq!(
        engine.target(),
        None,
        "a song with no tracks: the keys play nothing"
    );
    engine.take(&cmd(m, Command::Undo));
    engine.check(m, "one back");
    assert_eq!(engine.target(), Some(0));

    // A stream that opens is told the target again.
    let opened = m.handle(Input::AudioOpened {
        node_name: "alsa_output.card.analog-stereo".into(),
        sample_rate: 48_000,
    });
    assert!(opened.contains(&Effect::Engine(EngineOp::PlayTarget { seat: Some(0) })));
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
}

/// The doc with `next_id` left out: undo and redo bring the rest back, never a lower `next_id`.
fn held(doc: &Doc) -> Doc {
    Doc {
        next_id: 0,
        ..doc.clone()
    }
}

#[test]
fn random_track_edits_undo_and_redo_keep_the_engine_with_the_song() {
    instruments::tests::EVERY_SOUND.set(true);
    const ACTIONS: usize = 21;
    let mut made = [0usize; ACTIONS];
    let mut most_tracks = 0;
    let mut auditioned_checks = 0;
    for seed in 1..=12u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let m = &mut ready();
        let mut engine = FakeEngine::new(m);
        engine.take(&three_tracks(m));
        engine.check(m, "first");
        let first = m.play().song().doc.clone();
        let mut line = vec![first.clone()];
        // Every track id ever in the song: a new track never gets one of them.
        let mut ever: HashSet<TrackId> = first.tracks.iter().map(|t| t.id).collect();
        let kinds: Vec<InstrumentKind> = InstrumentKind::ALL.to_vec();
        for round in 0..400 {
            let ctx = format!("seed {seed}, round {round}");
            let ids = track_ids(m);
            let some_track = |rng: &mut Rng| match (ids.len(), rng.below(12)) {
                (0, _) | (_, 0) => 9_999,
                (n, _) => ids[rng.below(n as u64) as usize],
            };
            let some_kind = |rng: &mut Rng| kinds[rng.below(kinds.len() as u64) as usize];
            let steps = m.view().song.undo_count;
            let before = m.play().song().doc.clone();
            let owned = engine.owns(m);
            let action = rng.below(ACTIONS as u64) as usize;
            let effects_made = engine.effects_made;
            let some_strip = |rng: &mut Rng| match rng.below(6) {
                0 => StripRef::Master,
                1 => StripRef::Return {
                    bus: [SendBus::A, SendBus::B][rng.below(2) as usize],
                },
                _ => StripRef::Track {
                    id: some_track(rng),
                },
            };
            let some_db = |rng: &mut Rng| match rng.below(8) {
                0 => None,
                n => Some(n as f32 * 3.0 - 18.0),
            };
            let mut commands = Vec::new();
            let mut go = |command: Command| commands.push(command);
            let mut record_at = None;
            match action {
                0 | 1 => {
                    // A track from the shelf, now and then with a look at another row first.
                    go(Command::OpenShelf {
                        mode: ShelfMode::NewTrack,
                    });
                    if rng.below(3) == 0 {
                        go(Command::ShelfSelect {
                            instrument: Some(some_kind(&mut rng)),
                        });
                    }
                    go(Command::ShelfSelect {
                        instrument: Some(some_kind(&mut rng)),
                    });
                    go(Command::ShelfConfirm);
                }
                2 => go(Command::DeleteTrack {
                    track: some_track(&mut rng),
                }),
                3 => go(Command::MoveTrack {
                    track: some_track(&mut rng),
                    to: rng.below(ids.len() as u64 + 1) as u32,
                }),
                4 => go(Command::RenameTrack {
                    track: some_track(&mut rng),
                    name: format!("名前 {}", rng.below(5)),
                }),
                5 => go(Command::SetTrackColor {
                    track: some_track(&mut rng),
                    color: rng.below(7) as u8,
                }),
                6 | 7 => {
                    // Another instrument from the shelf.
                    let track = some_track(&mut rng);
                    go(Command::OpenShelf {
                        mode: ShelfMode::Replace { track },
                    });
                    go(Command::ShelfSelect {
                        instrument: Some(some_kind(&mut rng)),
                    });
                    if rng.below(3) == 0 {
                        go(Command::ShelfSelect {
                            instrument: Some(some_kind(&mut rng)),
                        });
                    }
                    go(Command::ShelfConfirm);
                }
                8 | 9 => go(Command::Undo),
                10 => go(Command::Redo),
                11 => {
                    let track = some_track(&mut rng);
                    go(match rng.below(3) {
                        0 => Command::ToggleMute { track },
                        1 => Command::ToggleSolo { track },
                        _ => Command::SelectTrack {
                            track,
                            from_arm: false,
                        },
                    });
                }
                12 => {
                    // A recording on the selected track; the shelf lets go first.
                    go(Command::ShelfCancel);
                    if !ids.is_empty() {
                        let at = 1920.0 * rng.below(8) as f64;
                        go(Command::Locate { tick: at as u64 });
                        record_at = Some(at);
                    }
                }
                13 => {
                    // A row is auditioned and left that way, for the next rounds.
                    let mode = match rng.below(2) {
                        0 => ShelfMode::NewTrack,
                        _ => ShelfMode::Replace {
                            track: some_track(&mut rng),
                        },
                    };
                    go(Command::OpenShelf { mode });
                    go(Command::ShelfSelect {
                        instrument: Some(some_kind(&mut rng)),
                    });
                }
                14 => go(match rng.below(4) {
                    0 => Command::ShelfCancel,
                    1 => Command::Back,
                    2 => Command::ToggleBand { band: Band::Shelf },
                    _ => Command::ShelfSelect { instrument: None },
                }),
                15 => go(Command::SetTempo {
                    bpm: 60.0 + rng.below(120) as f64,
                }),
                16 => {
                    // A fader, a pan or a send: a move of a drag, or its end.
                    let commit = rng.below(2) == 0;
                    go(match rng.below(3) {
                        0 => Command::SetGain {
                            strip: some_strip(&mut rng),
                            db: some_db(&mut rng),
                            commit,
                        },
                        1 => Command::SetPan {
                            strip: some_strip(&mut rng),
                            pan: rng.below(9) as i32 * 25 - 100,
                            commit,
                        },
                        _ => Command::SetSend {
                            track: some_track(&mut rng),
                            bus: [SendBus::A, SendBus::B][rng.below(2) as usize],
                            db: some_db(&mut rng),
                            commit,
                        },
                    });
                }
                17 => {
                    // A parameter of an effect that is there, now and then of one that is not.
                    let mut there = Vec::new();
                    let strips = (before.tracks.iter().map(|t| StripId::Track(t.id))).chain([
                        StripId::Return(Bus::A),
                        StripId::Return(Bus::B),
                        StripId::Master,
                    ]);
                    for strip in strips {
                        for slot in 0..4 {
                            if let Some(kind) = before.insert(strip, slot).and_then(Insert::kind) {
                                there.push((strip, slot as u8, kind));
                            }
                        }
                    }
                    let absent = (
                        StripId::Master,
                        3,
                        meloet_model::effects::EffectKind::Chorus,
                    );
                    let (strip, slot, kind) = match (there.len(), rng.below(10)) {
                        (0, _) | (_, 0) => absent,
                        (n, _) => there[rng.below(n as u64) as usize],
                    };
                    let params = kind.params();
                    let param = rng.below(params.len() as u64) as usize;
                    let spec = &params[param];
                    go(Command::SetParam {
                        insert: InsertRef {
                            strip: strip_ref(strip),
                            slot,
                        },
                        effect: effect_kind(kind),
                        param: param as u32,
                        value: spec.min + (spec.max - spec.min) * rng.below(5) as f32 / 4.0,
                        commit: rng.below(2) == 0,
                    });
                }
                18 | 19 => {
                    // An effect goes in from the shelf, or comes out.
                    let insert = InsertRef {
                        strip: some_strip(&mut rng),
                        slot: rng.below(4) as u8,
                    };
                    if rng.below(3) == 0 {
                        go(Command::RemoveEffect { insert });
                    } else {
                        go(Command::OpenShelf {
                            mode: ShelfMode::InsertEffect {
                                strip: insert.strip,
                                slot: insert.slot,
                            },
                        });
                        go(Command::ShelfSelectEffect {
                            effect: Some(EffectKind::ALL[rng.below(6) as usize]),
                        });
                        go(Command::ShelfConfirm);
                    }
                }
                _ => go(match rng.below(3) {
                    0 => Command::CancelMixPreview,
                    n => Command::ToggleReturnMute {
                        bus: [SendBus::A, SendBus::B][n as usize - 1],
                    },
                }),
            }
            for command in commands {
                engine.take(&cmd(m, command));
            }
            if let Some(at) = record_at {
                let notes = [(60, at + 10.0, at + 200.0), (64, at + 300.0, at + 700.0)];
                engine.take(&record_once(m, &notes, at + 960.0));
            }
            engine.check(m, &ctx);
            if m.view().shelf.selected.is_some() {
                auditioned_checks += 1;
            }
            if matches!(action, 16 | 17 | 20) {
                assert_eq!(
                    engine.effects_made, effects_made,
                    "{ctx}: no effect is made"
                );
            }

            let doc = m.play().song().doc.clone();
            let now = m.view().song.undo_count;
            let stepped = !matches!(action, 8..=10);
            if stepped && now == steps + 1 {
                made[action] += 1;
                for t in &doc.tracks {
                    if before.track(t.id).is_none() {
                        assert!(ever.insert(t.id), "{ctx}: id {} was given before", t.id.0);
                    }
                }
                line.truncate(steps as usize + 1);
                line.push(doc.clone());
            } else if stepped {
                assert_eq!(now, steps, "{ctx}: one command, one step at most");
            }
            assert_eq!(held(&doc), held(&line[now as usize]), "{ctx}");
            // A track that was there before and after, with the same instrument, has the very
            // instrument it had.
            let owns = engine.owns(m);
            for (id, was) in &owned {
                if let Some(is) = owns.get(id)
                    && is.1 == was.1
                {
                    assert_eq!(is.0, was.0, "{ctx}: the instrument of track {}", id.0);
                }
            }
            most_tracks = most_tracks.max(doc.tracks.len());
            if round % 50 == 49 {
                // Solo is not saved; the rest of the song is.
                let song = m.play().song();
                let back = songfile::decode(&songfile::encode(song)).unwrap();
                assert_eq!(back.doc, song.doc, "{ctx}: saved and opened");
                assert_eq!(back.session.play_target, song.session.play_target, "{ctx}");
            }
        }
        while m.view().song.undo_count > 0 {
            engine.take(&cmd(m, Command::Undo));
        }
        engine.check(m, "all undone");
        assert_eq!(
            held(&m.play().song().doc),
            held(&first),
            "seed {seed}: all undone"
        );
        while m.view().song.redo_count > 0 {
            engine.take(&cmd(m, Command::Redo));
        }
        engine.check(m, "all redone");
        assert_eq!(
            held(&m.play().song().doc),
            held(line.last().unwrap()),
            "seed {seed}: all redone"
        );
    }
    // Adding, deleting, moving, renaming, coloring, replacing, muting and recording all made
    // steps, and the engine was looked at while rows were auditioned.
    for action in [0, 1, 2, 3, 4, 5, 6, 7, 11, 12, 15, 16, 17, 18, 19, 20] {
        assert!(made[action] > 20, "action {action}: {made:?}");
    }
    assert!(most_tracks >= 8, "{most_tracks} tracks");
    assert!(auditioned_checks > 200, "{auditioned_checks}");
}

#[test]
fn a_song_takes_no_more_tracks_than_the_engine_has_seats() {
    instruments::tests::EVERY_SOUND.set(true);
    let m = &mut ready();
    let mut engine = FakeEngine::new(m);
    cmd(
        m,
        Command::OpenShelf {
            mode: ShelfMode::NewTrack,
        },
    );
    let add = |m: &mut Model, engine: &mut FakeEngine| {
        let kind = InstrumentKind::ALL[m.view().song.tracks.len() % 11];
        engine.take(&cmd(
            m,
            Command::ShelfSelect {
                instrument: Some(kind),
            },
        ));
        engine.take(&cmd(m, Command::ShelfConfirm));
    };
    for _ in 1..MAX_TRACKS {
        add(m, &mut engine);
    }
    engine.check(m, "full");
    assert_eq!(m.view().song.tracks.len(), MAX_TRACKS);
    let mut seats = m.track_seats();
    seats.sort_unstable();
    assert_eq!(seats, (0..MAX_TRACKS).map(Some).collect::<Vec<_>>());
    let steps = m.view().song.undo_count;

    // The 65th: the row is still auditioned, and nothing is added or said.
    add(m, &mut engine);
    engine.check(m, "one too many");
    assert_eq!(m.view().song.tracks.len(), MAX_TRACKS);
    assert_eq!(m.view().song.undo_count, steps);
    assert!(m.view().shelf.selected.is_some());
    assert!(!matches!(
        m.view().status.message,
        Some(meloet_protocol::StatusMessage::TrackAdded { .. })
    ));
    // With one track gone there is room again, on the seat that came free.
    engine.take(&cmd(m, Command::DeleteTrack { track: 1 }));
    engine.take(&cmd(m, Command::ShelfConfirm));
    engine.check(m, "room again");
    assert_eq!(m.view().song.tracks.len(), MAX_TRACKS);
    assert!(
        m.track_seats().contains(&Some(0)),
        "the seat of the deleted track is used again"
    );
}
