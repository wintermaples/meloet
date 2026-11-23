use super::*;
use crate::effects::{delay, limiter, reverb};
use crate::history::History;
use crate::id_book::held;
use crate::{BuiltinInstrument, edit};

const T1: StripId = StripId::Track(TrackId(1));
const A: StripId = StripId::Return(Bus::A);
const B: StripId = StripId::Return(Bus::B);

#[test]
fn gain_keeps_to_the_scale() {
    assert_eq!(Gain::from_db(0.0), Gain::UNITY);
    assert_eq!(Gain::from_db(6.0).db(), Some(6.0));
    assert_eq!(Gain::from_db(9.0).db(), Some(6.0));
    assert_eq!(Gain::from_db(f64::INFINITY).db(), Some(6.0));
    assert_eq!(Gain::from_db(-60.0).db(), Some(-60.0));
    assert_eq!(Gain::from_db(-60.01), Gain::SILENT);
    assert_eq!(Gain::from_db(f64::NEG_INFINITY), Gain::SILENT);
    assert_eq!(Gain::from_db(f64::NAN), Gain::SILENT);
    assert_eq!(Gain::SILENT.db(), None);
    // What an f32 of the screen carries comes out as hundredths.
    assert_eq!(Gain::from_db(f64::from(-0.3f32)).db(), Some(-0.3));
    assert_eq!(Gain::from_db(-15.004).db(), Some(-15.0));
    assert_eq!(Gain::UNITY.linear(), 1.0);
    assert_eq!(Gain::SILENT.linear(), 0.0);
    assert!((Gain::from_db(-6.0).linear() - 0.501).abs() < 0.001);
    assert!(Gain::SILENT < Gain::from_db(-60.0));
}

#[test]
fn pan_keeps_to_its_range() {
    assert_eq!(Pan::new(0), Pan::CENTER);
    assert_eq!(Pan::new(-20).get(), -20);
    assert_eq!(Pan::new(250).get(), 100);
    assert_eq!(Pan::new(-250).get(), -100);
}

#[test]
fn a_new_song_has_reverb_delay_and_limiter() {
    let doc = Doc::new_default();
    let first = |strip| doc.insert(strip, 0).unwrap();
    assert_eq!(first(A).kind(), Some(EffectKind::Reverb));
    assert_eq!(first(A).value(reverb::MIX), Some(100.0));
    assert_eq!(first(A).value(reverb::DECAY), Some(2.4));
    assert_eq!(first(B).kind(), Some(EffectKind::Delay));
    assert_eq!(first(B).value(delay::MIX), Some(100.0));
    assert_eq!(first(StripId::Master).kind(), Some(EffectKind::Limiter));
    assert_eq!(first(StripId::Master).value(limiter::CEILING), Some(-0.3));
    for strip in [A, B, StripId::Master] {
        let c = doc.channel(strip).unwrap();
        assert_eq!((c.gain, c.pan), (Gain::UNITY, Pan::CENTER));
        assert!(c.inserts[1..].iter().all(Option::is_none));
    }
    assert!(doc.returns.iter().all(|r| !r.muted));
    assert_eq!(doc.channel(T1), Some(&Channel::default()));
    assert_eq!(doc.tracks[0].sends, [Gain::from_db(-15.0), Gain::SILENT]);
    assert_eq!(doc.channel(StripId::Track(TrackId(9))), None);
}

#[test]
fn a_new_track_sends_by_its_group() {
    use BuiltinInstrument as I;
    let a = |i: I| new_track_sends(&InstrumentRef::Builtin(i))[0].db();
    for i in [
        I::GrandPiano,
        I::ElectricPiano,
        I::Organ,
        I::OrganReed,
        I::Marimba,
    ] {
        assert_eq!(a(i), Some(-15.0));
    }
    for i in [I::Strings, I::Pizzicato, I::WarmPad, I::SynthLead, I::Sine] {
        assert_eq!(a(i), Some(-15.0));
    }
    // Not in Y9: guitars and wind instruments send as the piano does.
    for i in [I::GuitarNylon, I::GuitarDistortion, I::Trumpet, I::Flute] {
        assert_eq!(a(i), Some(-15.0));
    }
    for i in [I::StudioKit, I::ElectroKit, I::BrushKit] {
        assert_eq!(a(i), Some(-24.0));
    }
    for i in [I::FingerBass, I::PickBass, I::WoodBass, I::SynthBass] {
        assert_eq!(a(i), None);
    }
    for i in I::all() {
        assert_eq!(new_track_sends(&InstrumentRef::Builtin(i))[1], Gain::SILENT);
    }

    let doc = Doc::new_default();
    let drums = InstrumentRef::Builtin(I::StudioKit);
    let (e, id) = edit::add_track(&doc, None, drums, crate::Language::Ja).unwrap();
    let t = e.doc.track(id).unwrap();
    assert_eq!(t.sends, [Gain::from_db(-24.0), Gain::SILENT]);
    assert_eq!(t.channel, Channel::default());
}

#[test]
fn fader_pan_and_send_are_steps() {
    let doc = Doc::new_default();
    for strip in [T1, A, B, StripId::Master] {
        let e = set_gain(&doc, strip, Gain::from_db(-3.0)).unwrap();
        assert_eq!(e.step, StepKind::SetGain { strip });
        assert_eq!(e.doc.channel(strip).unwrap().gain.db(), Some(-3.0));
        assert_eq!(
            set_gain(&e.doc, strip, Gain::from_db(-3.0)),
            Err(EditError::Unchanged)
        );
        let silent = set_gain(&e.doc, strip, Gain::SILENT).unwrap();
        assert_eq!(silent.doc.channel(strip).unwrap().gain, Gain::SILENT);
    }
    // Only the strip that changed is a new one.
    let e = set_gain(&doc, A, Gain::from_db(-3.0)).unwrap();
    assert!(Arc::ptr_eq(&doc.tracks[0], &e.doc.tracks[0]));
    assert!(Arc::ptr_eq(&doc.returns[1], &e.doc.returns[1]));
    assert!(Arc::ptr_eq(&doc.master, &e.doc.master));
    assert!(!Arc::ptr_eq(&doc.returns[0], &e.doc.returns[0]));

    for strip in [T1, A, B, StripId::Master] {
        let e = set_pan(&doc, strip, Pan::new(-20)).unwrap();
        assert_eq!(e.step, StepKind::SetPan { strip });
        assert_eq!(e.doc.channel(strip).unwrap().pan.get(), -20);
        assert_eq!(
            set_pan(&e.doc, strip, Pan::new(-20)),
            Err(EditError::Unchanged)
        );
    }

    let e = set_send(&doc, TrackId(1), Bus::B, Gain::from_db(-18.0)).unwrap();
    assert_eq!(
        e.step,
        StepKind::SetSend {
            track: TrackId(1),
            bus: Bus::B
        }
    );
    assert_eq!(
        e.doc.tracks[0].sends,
        [Gain::from_db(-15.0), Gain::from_db(-18.0)]
    );
    assert_eq!(
        set_send(&doc, TrackId(1), Bus::B, Gain::SILENT),
        Err(EditError::Unchanged)
    );

    let gone = StripId::Track(TrackId(9));
    let no_track = Err(EditError::NoSuchTrack(TrackId(9)));
    assert_eq!(set_gain(&doc, gone, Gain::SILENT), no_track);
    assert_eq!(set_pan(&doc, gone, Pan::new(1)), no_track);
    assert_eq!(set_send(&doc, TrackId(9), Bus::A, Gain::UNITY), no_track);
}

#[test]
fn parameters_stay_in_range_and_name_their_effect() {
    let doc = Doc::new_default();
    let set = |doc: &Doc, kind, param, value| set_param(doc, A, 0, kind, param, value);
    let e = set(&doc, EffectKind::Reverb, reverb::DECAY, 4.0).unwrap();
    assert_eq!(
        e.step,
        StepKind::SetParam {
            strip: A,
            slot: 0,
            kind: EffectKind::Reverb,
            param: reverb::DECAY
        }
    );
    assert_eq!(e.doc.insert(A, 0).unwrap().value(reverb::DECAY), Some(4.0));
    let e = set(&doc, EffectKind::Reverb, reverb::DECAY, 99.0).unwrap();
    assert_eq!(e.doc.insert(A, 0).unwrap().value(reverb::DECAY), Some(10.0));
    assert_eq!(
        set(&doc, EffectKind::Reverb, reverb::DECAY, 2.4),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        set(&doc, EffectKind::Reverb, reverb::MIX, 300.0),
        Err(EditError::Unchanged),
        "100 % already"
    );
    // Another effect than the one in the slot, no such parameter, an empty slot, no slot.
    let none = Err(EditError::NoSuchInsert);
    assert_eq!(set(&doc, EffectKind::Delay, delay::MIX, 50.0), none);
    assert_eq!(set(&doc, EffectKind::Reverb, 99, 1.0), none);
    assert_eq!(
        set_param(&doc, A, 1, EffectKind::Reverb, reverb::DECAY, 4.0),
        none
    );
    assert_eq!(
        set_param(&doc, A, 4, EffectKind::Reverb, reverb::DECAY, 4.0),
        none
    );
    // Steps and switches land on whole numbers.
    let e = set_param(&doc, B, 0, EffectKind::Delay, delay::INTERVAL_STEP, 3.4).unwrap();
    let d = e.doc.insert(B, 0).unwrap();
    assert_eq!(d.value(delay::INTERVAL_STEP), Some(3.0));
    let e = set_param(&doc, B, 0, EffectKind::Delay, delay::PING_PONG, 0.7).unwrap();
    assert_eq!(
        e.doc.insert(B, 0).unwrap().value(delay::PING_PONG),
        Some(1.0)
    );
}

#[test]
fn effects_go_into_empty_slots_and_come_out() {
    let doc = Doc::new_default();
    let e = add_effect(&doc, T1, 2, EffectKind::Compressor).unwrap();
    assert_eq!(
        e.step,
        StepKind::AddEffect {
            strip: T1,
            slot: 2,
            kind: EffectKind::Compressor
        }
    );
    let c = e.doc.channel(T1).unwrap();
    assert_eq!(
        c.inserts[2],
        Some(Insert::new(InsertId(doc.next_id), EffectKind::Compressor))
    );
    assert_eq!(e.doc.next_id, doc.next_id + 1);
    assert!(c.inserts[0].is_none() && c.inserts[1].is_none() && c.inserts[3].is_none());
    assert_eq!(
        add_effect(&e.doc, T1, 2, EffectKind::Eq4),
        Err(EditError::SlotTaken)
    );
    assert_eq!(
        add_effect(&doc, T1, 4, EffectKind::Eq4),
        Err(EditError::NoSuchInsert)
    );

    // A reverb or a delay starts at 30 % mix where the sound itself goes through it, and
    // all effect on a return. The returns of a new song are all effect.
    for (kind, mix) in [
        (EffectKind::Reverb, reverb::MIX),
        (EffectKind::Delay, delay::MIX),
    ] {
        for (strip, percent) in [(T1, 30.0), (StripId::Master, 30.0), (A, 100.0), (B, 100.0)] {
            let e = add_effect(&doc, strip, 3, kind).unwrap();
            let insert = e.doc.insert(strip, 3).unwrap();
            assert_eq!(insert.value(mix), Some(percent), "{kind:?} on {strip:?}");
            let mut rest = insert.clone();
            rest.set(mix, 100.0);
            assert_eq!(rest, Insert::new(insert.id, kind), "only the mix differs");
        }
    }
    assert_eq!(doc.insert(A, 0).unwrap().value(reverb::MIX), Some(100.0));
    assert_eq!(doc.insert(B, 0).unwrap().value(delay::MIX), Some(100.0));
    let chorus = add_effect(&doc, T1, 3, EffectKind::Chorus).unwrap();
    assert_eq!(
        chorus.doc.insert(T1, 3),
        Some(&Insert::new(InsertId(doc.next_id), EffectKind::Chorus))
    );

    let out = remove_effect(&e.doc, T1, 2).unwrap();
    assert_eq!(
        out.step,
        StepKind::RemoveEffect {
            strip: T1,
            slot: 2,
            kind: EffectKind::Compressor
        }
    );
    assert_eq!(held(&out.doc), held(&doc));
    assert_eq!(remove_effect(&doc, T1, 2), Err(EditError::NoSuchInsert));
    assert_eq!(remove_effect(&doc, T1, 7), Err(EditError::NoSuchInsert));
    // The reverb of return A can go too; the slot is then empty.
    let out = remove_effect(&doc, A, 0).unwrap();
    assert!(out.doc.insert(A, 0).is_none());
}

#[test]
fn passing_by_moving_and_replacing_are_steps_that_undo_takes_back() {
    let mut doc = Doc::new_default();
    let mut h = History::new();
    let kinds = |doc: &Doc| {
        let c = doc.channel(T1).unwrap();
        c.inserts.clone().map(|i| i.and_then(|i| i.kind()))
    };
    for (slot, kind) in [(0, EffectKind::Eq4), (1, EffectKind::Compressor)] {
        let e = add_effect(&doc, T1, slot, kind).unwrap();
        h.apply(&mut doc, e);
    }
    let e = add_effect(&doc, T1, 3, EffectKind::Delay).unwrap();
    h.apply(&mut doc, e);
    let start = held(&doc);

    // 「とばす」 keeps the values, and flips back.
    let e = set_param(&doc, T1, 1, EffectKind::Compressor, 1, 8.0).unwrap();
    h.apply(&mut doc, e);
    let e = toggle_bypass(&doc, T1, 1).unwrap();
    let step = StepKind::Bypass {
        strip: T1,
        slot: 1,
        bypassed: true,
    };
    assert_eq!(e.step, step);
    h.apply(&mut doc, e);
    let comp = doc.insert(T1, 1).unwrap();
    assert!(comp.bypassed && comp.value(1) == Some(8.0));
    assert!(!doc.insert(T1, 0).unwrap().bypassed);
    assert_eq!(
        toggle_bypass(&doc, T1, 2).unwrap_err(),
        EditError::NoSuchInsert
    );
    assert!(toggle_bypass(&doc, StripId::Track(TrackId(9)), 0).is_err());

    // A slot moves with what it holds; the ones between move by one, the empty one too.
    let e = move_insert(&doc, T1, 1, 3).unwrap();
    assert_eq!(
        e.step,
        StepKind::MoveInsert {
            strip: T1,
            from: 1,
            to: 3
        }
    );
    h.apply(&mut doc, e);
    use EffectKind::{Compressor, Delay, Eq4, Reverb};
    assert_eq!(
        kinds(&doc),
        [Some(Eq4), None, Some(Delay), Some(Compressor)]
    );
    let comp = doc.insert(T1, 3).unwrap();
    assert!(comp.bypassed && comp.value(1) == Some(8.0));
    let e = move_insert(&doc, T1, 3, 0).unwrap();
    h.apply(&mut doc, e);
    assert_eq!(
        kinds(&doc),
        [Some(Compressor), Some(Eq4), None, Some(Delay)]
    );
    // Past the end is the end.
    let e = move_insert(&doc, T1, 0, 9).unwrap();
    assert_eq!(doc.insert(T1, 0), e.doc.insert(T1, 3));
    assert_eq!(
        move_insert(&doc, T1, 3, 9).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        move_insert(&doc, T1, 1, 1).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        move_insert(&doc, T1, 2, 0).unwrap_err(),
        EditError::NoSuchInsert
    );
    assert_eq!(
        move_insert(&doc, T1, 4, 0).unwrap_err(),
        EditError::NoSuchInsert
    );
    for (from, to) in [(0, 3), (3, 0), (1, 2), (2, 1), (0, 1), (3, 3)] {
        let mut slots = [0, 1, 2, 3];
        move_slot(&mut slots, from, to);
        for was in 0..4 {
            assert_eq!(slots[moved_slot(was, from, to)], was, "{from} to {to}");
        }
    }

    // Another effect in its place, as a new one on that strip and heard.
    let e = replace_effect(&doc, T1, 0, Reverb).unwrap();
    assert_eq!(e.step, StepKind::ReplaceEffect { strip: T1, slot: 0 });
    let (was, new) = (doc.insert(T1, 0).unwrap().id, InsertId(doc.next_id));
    h.apply(&mut doc, e);
    assert_ne!(was, new, "what replaces an insert is another insert");
    assert_eq!(doc.insert(T1, 0), Some(&Insert::new_on(new, Reverb, T1)));
    assert_eq!(
        replace_effect(&doc, T1, 0, Reverb).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        replace_effect(&doc, T1, 2, Reverb).unwrap_err(),
        EditError::NoSuchInsert
    );
    let e = replace_plugin_effect(&doc, T1, 0, mverb()).unwrap();
    h.apply(&mut doc, e);
    assert_eq!(doc.plugin(PluginPlace::Insert(T1, 0)), Some(&mverb()));
    assert_eq!(
        replace_plugin_effect(&doc, T1, 0, mverb()).unwrap_err(),
        EditError::Unchanged
    );
    let e = replace_plugin_effect(&doc, T1, 0, surge()).unwrap();
    h.apply(&mut doc, e);
    let e = replace_effect(&doc, T1, 0, Eq4).unwrap();
    h.apply(&mut doc, e);

    // The old one comes back as it was: its values, and passed by.
    h.undo(&mut doc);
    h.undo(&mut doc);
    h.undo(&mut doc);
    h.undo(&mut doc);
    let comp = doc.insert(T1, 0).unwrap();
    assert!(comp.bypassed && comp.value(1) == Some(8.0));
    for _ in 0..4 {
        h.undo(&mut doc);
    }
    assert_eq!(held(&doc), start);
    for _ in 0..8 {
        h.redo(&mut doc);
    }
    let id = doc.insert(T1, 0).unwrap().id;
    assert_eq!(doc.insert(T1, 0), Some(&Insert::new_on(id, Eq4, T1)));
}

#[test]
fn return_mute_flips() {
    let doc = Doc::new_default();
    let e = toggle_return_mute(&doc, Bus::B);
    assert_eq!(
        e.step,
        StepKind::MuteReturn {
            bus: Bus::B,
            muted: true
        }
    );
    assert!(e.doc.returns[1].muted && !e.doc.returns[0].muted);
    let back = toggle_return_mute(&e.doc, Bus::B);
    assert_eq!(held(&back.doc), held(&doc));
}

#[test]
fn undo_and_redo_bring_the_mixer_back() {
    let start = Doc::new_default();
    let mut doc = start.clone();
    let mut history = History::new();
    let mut seen = vec![held(&doc)];
    type Step = Box<dyn Fn(&Doc) -> Edit>;
    let steps: Vec<Step> = vec![
        Box::new(|d| set_gain(d, T1, Gain::from_db(-4.8)).unwrap()),
        Box::new(|d| set_pan(d, T1, Pan::new(20)).unwrap()),
        Box::new(|d| set_send(d, TrackId(1), Bus::B, Gain::from_db(-18.0)).unwrap()),
        Box::new(|d| add_effect(d, T1, 0, EffectKind::Eq4).unwrap()),
        Box::new(|d| set_param(d, T1, 0, EffectKind::Eq4, 1, 6.0).unwrap()),
        Box::new(|d| toggle_return_mute(d, Bus::A)),
        Box::new(|d| remove_effect(d, StripId::Master, 0).unwrap()),
        Box::new(|d| set_gain(d, StripId::Master, Gain::SILENT).unwrap()),
    ];
    for step in &steps {
        let e = step(&doc);
        history.apply(&mut doc, e);
        seen.push(held(&doc));
    }
    for i in (0..steps.len()).rev() {
        assert!(history.undo(&mut doc).is_some());
        assert_eq!(held(&doc), seen[i]);
    }
    assert_eq!(held(&doc), held(&start));
    for want in &seen[1..] {
        assert!(history.redo(&mut doc).is_some());
        assert_eq!(&held(&doc), want);
    }
}

fn surge() -> Arc<PluginRef> {
    Arc::new(PluginRef::new(
        "vst3:535247455375726758545379",
        "Surge XT",
        "Surge Synth Team",
        "/usr/lib/vst3/Surge XT.vst3",
    ))
}

fn mverb() -> Arc<PluginRef> {
    Arc::new(PluginRef::new(
        "lv2:http://distrho.sf.net/plugins/MVerb",
        "MVerb",
        "DISTRHO",
        "/usr/lib/lv2/MVerb.lv2",
    ))
}

#[test]
fn a_plugin_goes_into_a_slot_and_out_again() {
    let doc = Doc::new_default();
    let e = add_plugin_effect(&doc, T1, 1, mverb()).unwrap();
    let place = PluginPlace::Insert(T1, 1);
    assert_eq!(e.step, StepKind::AddPlugin { place });
    let insert = e.doc.insert(T1, 1).unwrap();
    assert_eq!(insert.kind(), None);
    assert_eq!(insert.plugin_ref(), Some(&mverb()));
    assert_eq!(insert.values(), [] as [f32; 0]);
    assert!(!insert.bypassed);
    assert_eq!(e.doc.plugin(place), Some(&mverb()));
    assert_eq!(
        add_plugin_effect(&e.doc, T1, 1, mverb()).unwrap_err(),
        EditError::SlotTaken
    );
    assert_eq!(
        add_plugin_effect(&doc, T1, INSERT_SLOTS, mverb()).unwrap_err(),
        EditError::NoSuchInsert
    );
    // A parameter of a table is not a parameter of a plugin.
    assert_eq!(
        set_param(&e.doc, T1, 1, EffectKind::Reverb, 0, 1.0).unwrap_err(),
        EditError::NoSuchInsert
    );
    let gone = remove_effect(&e.doc, T1, 1).unwrap();
    assert_eq!(gone.step, StepKind::RemovePlugin { place });
    assert_eq!(held(&gone.doc), held(&doc));
    // An effect that comes with meloet still says which it was.
    let limiter = remove_effect(&doc, StripId::Master, 0).unwrap();
    assert_eq!(
        limiter.step,
        StepKind::RemoveEffect {
            strip: StripId::Master,
            slot: 0,
            kind: EffectKind::Limiter
        }
    );
}

#[test]
fn the_state_of_a_plugin_changes_as_one_step_and_undo_brings_the_old_state_back() {
    let start = Doc::new_default();
    let (e, track) = edit::add_track(
        &start,
        None,
        InstrumentRef::Plugin(surge()),
        crate::Language::Ja,
    )
    .unwrap();
    assert_eq!(e.doc.track(track).unwrap().name, "Surge XT");
    let doc = add_plugin_effect(&e.doc, StripId::Track(track), 0, mverb())
        .unwrap()
        .doc;
    let instrument = PluginPlace::Instrument(track);
    let insert = PluginPlace::Insert(StripId::Track(track), 0);
    let state: Arc<[u8]> = Arc::from(vec![7u8; 60_000]);

    let window = PluginChange::Window;
    let e = set_plugin_state(&doc, instrument, &surge().key, state.clone(), window).unwrap();
    assert_eq!(e.step, StepKind::PluginWindow { place: instrument });
    assert_eq!(e.doc.plugin(instrument).unwrap().state, state);
    assert_eq!(e.doc.plugin(instrument).unwrap().name, surge().name);
    assert_eq!(
        e.doc.plugin(insert),
        Some(&mverb()),
        "the insert is as it was"
    );
    // The tracks that did not change are shared, and so is the state itself.
    assert!(Arc::ptr_eq(&e.doc.tracks[0], &doc.tracks[0]));
    assert!(Arc::ptr_eq(
        &e.doc.plugin(instrument).unwrap().state,
        &state
    ));

    let mut history = History::new();
    let mut now = doc.clone();
    history.apply(&mut now, e);
    let param = PluginChange::Param(12);
    let e = set_plugin_state(&now, insert, &mverb().key, Arc::from([1u8, 2]), param).unwrap();
    assert_eq!(
        e.step,
        StepKind::SetPluginParam {
            place: insert,
            param: 12
        }
    );
    history.apply(&mut now, e);
    assert_eq!(*now.plugin(insert).unwrap().state, [1, 2]);
    assert!(!now.insert(StripId::Track(track), 0).unwrap().bypassed);
    assert!(history.undo(&mut now).is_some());
    assert_eq!(now.plugin(insert), Some(&mverb()));
    assert_eq!(now.plugin(instrument).unwrap().state, state);
    assert!(history.undo(&mut now).is_some());
    assert_eq!(held(&now), held(&doc));

    // The same state again is no step; another plugin at the place, or none, is no step.
    let again = set_plugin_state(&doc, instrument, &surge().key, Arc::from([]), window);
    assert_eq!(again.unwrap_err(), EditError::Unchanged);
    let other = set_plugin_state(&doc, instrument, &mverb().key, state.clone(), window);
    assert_eq!(other.unwrap_err(), EditError::NoSuchPlugin);
    for place in [
        PluginPlace::Instrument(TrackId(1)),
        PluginPlace::Instrument(TrackId(99)),
        PluginPlace::Insert(StripId::Master, 0),
        PluginPlace::Insert(StripId::Track(track), 3),
        PluginPlace::Insert(StripId::Track(track), 9),
    ] {
        let e = set_plugin_state(&doc, place, &surge().key, state.clone(), window);
        assert_eq!(e.unwrap_err(), EditError::NoSuchPlugin, "{place:?}");
    }
}

#[test]
fn a_track_of_a_plugin_or_an_sf2_preset_sends_as_its_group() {
    use crate::{Kept, SF2_DRUM_BANK, Sf2Ref};
    let preset = |bank, program| {
        InstrumentRef::Sf2(Arc::new(Sf2Ref {
            file: "FluidR3_GM.sf2".into(),
            sha256: "".into(),
            bank,
            program,
            name: "".into(),
            kept: Kept::NONE,
        }))
    };
    let a = |i: &InstrumentRef| new_track_sends(i)[0];
    assert_eq!(a(&InstrumentRef::Plugin(surge())), Gain::from_db(-15.0));
    assert_eq!(a(&preset(0, 27)), Gain::from_db(-15.0));
    assert_eq!(a(&preset(0, 33)), Gain::SILENT, "a bass sends nothing");
    assert_eq!(a(&preset(SF2_DRUM_BANK, 0)), Gain::from_db(-24.0));
    // The second track of the same name gets a number (spec 3 Y14).
    let doc = Doc::new_default();
    let (e, _) = edit::add_track(
        &doc,
        None,
        InstrumentRef::Plugin(surge()),
        crate::Language::Ja,
    )
    .unwrap();
    let (e, second) = edit::add_track(
        &e.doc,
        None,
        InstrumentRef::Plugin(surge()),
        crate::Language::Ja,
    )
    .unwrap();
    assert_eq!(e.doc.track(second).unwrap().name, "Surge XT 2");
    let (e, guitar) = edit::add_track(&e.doc, None, preset(0, 27), crate::Language::Ja).unwrap();
    assert_eq!(e.doc.track(guitar).unwrap().name, "ギター");
}

/// Every insert of the song, with the strip it is on.
fn insert_ids(doc: &Doc) -> Vec<InsertId> {
    let of = |c: &Channel| c.inserts.iter().flatten().map(|i| i.id).collect::<Vec<_>>();
    let mut ids: Vec<InsertId> = doc.tracks.iter().flat_map(|t| of(&t.channel)).collect();
    ids.extend(doc.returns.iter().flat_map(|r| of(&r.channel)));
    ids.extend(of(&doc.master));
    ids
}

#[test]
fn an_insert_keeps_its_id_and_no_id_is_given_twice() {
    use EffectKind::*;
    let mut doc = Doc::new_default();
    let mut h = History::new();
    assert_eq!(insert_ids(&doc), [InsertId(1), InsertId(2), InsertId(3)]);
    assert_eq!(doc.next_id, NEW_SONG_NEXT_ID);
    let mut seen: std::collections::HashSet<InsertId> = insert_ids(&doc).into_iter().collect();
    let mut step = |doc: &mut Doc, h: &mut History, e: Edit| {
        let before = insert_ids(doc);
        h.apply(doc, e);
        for id in insert_ids(doc) {
            assert!(id.0 < doc.next_id, "{id:?} is below next_id");
            assert!(
                before.contains(&id) || seen.insert(id),
                "{id:?} given twice"
            );
        }
    };

    let e = add_effect(&doc, T1, 0, Compressor).unwrap();
    step(&mut doc, &mut h, e);
    let e = add_effect(&doc, T1, 1, Eq4).unwrap();
    step(&mut doc, &mut h, e);
    let (comp, eq) = (doc.insert(T1, 0).unwrap().id, doc.insert(T1, 1).unwrap().id);
    assert_ne!(comp, eq);

    // Another order: the ids go with the effects.
    let e = move_insert(&doc, T1, 0, 3).unwrap();
    step(&mut doc, &mut h, e);
    assert_eq!(doc.insert(T1, 0).unwrap().id, eq);
    assert_eq!(doc.insert(T1, 3).unwrap().id, comp);
    let e = toggle_bypass(&doc, T1, 3).unwrap();
    step(&mut doc, &mut h, e);
    let e = set_param(&doc, T1, 3, Compressor, 0, -30.0).unwrap();
    step(&mut doc, &mut h, e);
    assert_eq!(doc.insert(T1, 3).unwrap().id, comp);

    // Replaced, taken out and put in again: new ids each time, also after an undo.
    let e = replace_effect(&doc, T1, 3, Chorus).unwrap();
    step(&mut doc, &mut h, e);
    let chorus = doc.insert(T1, 3).unwrap().id;
    assert_ne!(chorus, comp);
    h.undo(&mut doc);
    assert_eq!(doc.insert(T1, 3).unwrap().id, comp);
    let e = remove_effect(&doc, T1, 3).unwrap();
    step(&mut doc, &mut h, e);
    let e = add_effect(&doc, T1, 3, Compressor).unwrap();
    step(&mut doc, &mut h, e);
    let again = doc.insert(T1, 3).unwrap().id;
    assert!(again != comp && again != chorus);
}

/// Every way to fill the slots from the ids `1..=ids` with no id twice.
fn every_filling(ids: u32) -> Vec<[Option<u32>; INSERT_SLOTS]> {
    let mut all = vec![[None; INSERT_SLOTS]];
    for slot in 0..INSERT_SLOTS {
        let mut more = Vec::new();
        for filling in &all {
            for id in 1..=ids {
                if !filling.contains(&Some(id)) {
                    let mut with = *filling;
                    with[slot] = Some(id);
                    more.push(with);
                }
            }
        }
        all.extend(more);
    }
    all
}

fn inserts_of(ids: [Option<u32>; INSERT_SLOTS]) -> Inserts {
    ids.map(|id| id.map(|id| Insert::new(InsertId(id), EffectKind::Eq4)))
}

#[test]
fn insert_moves_bring_every_kept_insert_to_its_slot() {
    let all = every_filling(5);
    let mut most = 0;
    for have in &all {
        for want in &all {
            let moves = insert_moves(&inserts_of(*have), &inserts_of(*want));
            let mut now = *have;
            for &(from, to) in &moves {
                assert_ne!(from, to, "{have:?} -> {want:?}");
                move_slot(&mut now, from, to);
            }
            for slot in 0..INSERT_SLOTS {
                if want[slot].is_some() && have.contains(&want[slot]) {
                    assert_eq!(now[slot], want[slot], "{have:?} -> {want:?}: {moves:?}");
                }
            }
            if have == want {
                assert!(moves.is_empty());
            }
            most = most.max(moves.len());
        }
    }
    assert!(most < INSERT_SLOTS, "{most}");
}

#[test]
fn one_move_of_a_slot_is_one_insert_move() {
    for have in every_filling(4) {
        for from in 0..INSERT_SLOTS {
            for to in 0..INSERT_SLOTS {
                let mut want = have;
                move_slot(&mut want, from, to);
                let moves = insert_moves(&inserts_of(have), &inserts_of(want));
                assert!(moves.len() <= 1, "{have:?} {from} -> {to}: {moves:?}");
            }
        }
    }
}

#[test]
fn lowering_keeps_minus_infinity_and_stops_at_minus_60() {
    assert_eq!(Gain::from_db(-3.0).lowered(6.0).db(), Some(-9.0));
    assert_eq!(Gain::from_db(6.0).lowered(12.0).db(), Some(-6.0));
    assert_eq!(Gain::from_db(-57.0).lowered(6.0).db(), Some(-60.0));
    assert_eq!(Gain::from_db(-60.0).lowered(6.0).db(), Some(-60.0));
    assert_eq!(Gain::SILENT.lowered(6.0), Gain::SILENT);

    let place = |db: f64| Gain::from_db(db).position();
    let lowered = |db: f64, by: f64| Gain::from_position(Gain::lowered_place(place(db), by));
    assert_eq!(lowered(0.0, 6.0).db(), Some(-6.0));
    assert_eq!(lowered(-12.0, 6.0).db(), Some(-18.0));
    assert_eq!(Gain::lowered_place(0.0, 6.0), 0.0, "−∞ stays");
    // −60 dB is the place 0 of a lane, which is −∞: a point stops just above it.
    let floor = Gain::lowered_place(place(-57.0), 6.0);
    assert_eq!(floor, LOWEST_HEARD_PLACE);
    assert_eq!(Gain::from_position(floor).db(), Some(-60.0));
}

#[test]
fn lowering_every_track_is_one_step_that_leaves_returns_and_master() {
    use crate::automation::{AutoPoint, add_lane, put_points, set_resting};
    let piano = || InstrumentRef::Builtin(BuiltinInstrument::GrandPiano);
    let (e, t2) = edit::add_track(&Doc::new_default(), None, piano(), crate::Language::Ja).unwrap();
    let (e, t3) = edit::add_track(&e.doc, None, piano(), crate::Language::Ja).unwrap();
    let (e, t4) = edit::add_track(&e.doc, None, piano(), crate::Language::Ja).unwrap();
    let t1 = TrackId(1);
    let mut doc = e.doc;
    doc = set_gain(&doc, StripId::Track(t2), Gain::SILENT)
        .unwrap()
        .doc;
    doc = set_gain(&doc, StripId::Track(t3), Gain::from_db(-58.0))
        .unwrap()
        .doc;
    // Track 4's volume moves with a lane; track 1 has a lane that rests.
    let low = Gain::from_db(-57.0).position();
    for (track, points) in [
        (t4, vec![(0, 0.8), (1920, 0.0), (3840, low)]),
        (t1, vec![(0, 0.5)]),
    ] {
        let (e, _) = add_lane(&doc, track, AutoTarget::Volume, false).unwrap();
        let put: Vec<_> = (points.iter())
            .map(|&(tick, value)| AutoPoint { tick, value })
            .collect();
        doc = put_points(&e.doc, track, AutoTarget::Volume, &put)
            .unwrap()
            .0
            .doc;
    }
    doc = set_gain(&doc, StripId::Track(t4), Gain::from_db(-1.0))
        .unwrap()
        .doc;
    assert!(set_resting(&mut doc, t1, AutoTarget::Volume, true));

    let e = lower_all_tracks(&doc, 6.0).unwrap();
    assert_eq!(e.step, StepKind::LowerAll { count: 3 });
    let gain = |doc: &Doc, t: TrackId| doc.track(t).unwrap().channel.gain.db();
    assert_eq!(gain(&e.doc, t1), Some(-6.0), "a resting lane: the fader");
    assert_eq!(gain(&e.doc, t2), None, "−∞ stays");
    assert_eq!(gain(&e.doc, t3), Some(-60.0), "stops at −60 dB");
    assert_eq!(
        gain(&e.doc, t4),
        Some(-1.0),
        "the lane's points, not the fader"
    );
    let lane = |doc: &Doc, t: TrackId| doc.track(t).unwrap().lane(AutoTarget::Volume).cloned();
    let (before, after) = (lane(&doc, t4).unwrap(), lane(&e.doc, t4).unwrap());
    assert_ne!(before.id, after.id, "new points, a new id");
    assert!(!after.resting);
    let dbs: Vec<_> = (after.points.iter())
        .map(|p| Gain::from_position(p.value).db())
        .collect();
    assert_eq!(dbs, [Some(-6.0), None, Some(-60.0)]);
    assert_eq!(lane(&doc, t1), lane(&e.doc, t1));
    for strip in [A, B, StripId::Master] {
        assert_eq!(doc.channel(strip), e.doc.channel(strip));
    }

    let mut h = History::new();
    let mut now = doc.clone();
    h.apply(&mut now, e);
    assert!(h.undo(&mut now).is_some());
    assert_eq!(held(&now), held(&doc));

    assert_eq!(lower_all_tracks(&doc, 0.0), Err(EditError::Unchanged));
    assert_eq!(lower_all_tracks(&doc, f64::NAN), Err(EditError::Unchanged));
    // Every fader at −∞ and no lane: nothing to lower, no step.
    let mut silent = doc.clone();
    for track in &mut silent.tracks {
        let track = Arc::make_mut(track);
        track.channel.gain = Gain::SILENT;
        track.automation.clear();
    }
    assert_eq!(lower_all_tracks(&silent, 6.0), Err(EditError::Unchanged));
}
