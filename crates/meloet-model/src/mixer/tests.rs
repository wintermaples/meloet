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
    assert_eq!(first(A).kind(), EffectKind::Reverb);
    assert_eq!(first(A).value(reverb::MIX), Some(100.0));
    assert_eq!(first(A).value(reverb::DECAY), Some(2.4));
    assert_eq!(first(B).kind(), EffectKind::Delay);
    assert_eq!(first(B).value(delay::MIX), Some(100.0));
    assert_eq!(first(StripId::Master).kind(), EffectKind::Limiter);
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
    for i in [I::GrandPiano, I::ElectricPiano, I::Organ] {
        assert_eq!(a(i), Some(-15.0));
    }
    for i in [I::Strings, I::WarmPad, I::SynthLead, I::Sine] {
        assert_eq!(a(i), Some(-15.0));
    }
    for i in [I::StudioKit, I::ElectroKit] {
        assert_eq!(a(i), Some(-24.0));
    }
    for i in [I::FingerBass, I::SynthBass] {
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

    for strip in [T1, A, B] {
        let e = set_pan(&doc, strip, Pan::new(-20)).unwrap();
        assert_eq!(e.step, StepKind::SetPan { strip });
        assert_eq!(e.doc.channel(strip).unwrap().pan.get(), -20);
        assert_eq!(
            set_pan(&e.doc, strip, Pan::new(-20)),
            Err(EditError::Unchanged)
        );
    }
    assert_eq!(
        set_pan(&doc, StripId::Master, Pan::new(5)),
        Err(EditError::NoPan)
    );

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
