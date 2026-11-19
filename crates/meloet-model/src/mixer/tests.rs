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
    let (e, id) = edit::add_track(&doc, None, drums).unwrap();
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
    assert_eq!(c.inserts[2], Some(Insert::new(EffectKind::Compressor)));
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
            assert_eq!(rest, Insert::new(kind), "only the mix differs");
        }
    }
    assert_eq!(doc.insert(A, 0).unwrap().value(reverb::MIX), Some(100.0));
    assert_eq!(doc.insert(B, 0).unwrap().value(delay::MIX), Some(100.0));
    let chorus = add_effect(&doc, T1, 3, EffectKind::Chorus).unwrap();
    assert_eq!(
        chorus.doc.insert(T1, 3),
        Some(&Insert::new(EffectKind::Chorus))
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
    h.apply(&mut doc, e);
    assert_eq!(doc.insert(T1, 0), Some(&Insert::new_on(Reverb, T1)));
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
    assert_eq!(doc.insert(T1, 0), Some(&Insert::new_on(Eq4, T1)));
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
    assert_eq!(gone.doc, doc);
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
    let (e, track) = edit::add_track(&start, None, InstrumentRef::Plugin(surge())).unwrap();
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
    let (e, _) = edit::add_track(&doc, None, InstrumentRef::Plugin(surge())).unwrap();
    let (e, second) = edit::add_track(&e.doc, None, InstrumentRef::Plugin(surge())).unwrap();
    assert_eq!(e.doc.track(second).unwrap().name, "Surge XT 2");
    let (e, guitar) = edit::add_track(&e.doc, None, preset(0, 27)).unwrap();
    assert_eq!(e.doc.track(guitar).unwrap().name, "ギター");
}
