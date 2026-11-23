//! 「（何を）」 of the rows, as the tenth edition lists it (spec 3, 92-3): the values come from
//! the `Doc` before the step and the one after it.

use meloet_model::controls::{self, ControlKind as ModelControl};
use meloet_model::edit::{self, Edit};
use meloet_model::notes::{self, WriteAt};
use meloet_model::{
    BuiltinInstrument, ClipId, Doc, InstrumentRef, Note, QuantizeSettings, TrackColor, VocalInput,
    arrange, quantize,
};
use meloet_protocol::{ControlKind, InstrumentKind, SoundName, VocalInput as InputView};

use super::*;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;

fn note(start: u64, key: u8, velocity: u8) -> Note {
    Note {
        start,
        length: Q / 2,
        key,
        velocity,
        from: None,
    }
}

/// What the row of `edit`, made on `before`, says.
fn what(before: &Doc, edit: &Edit) -> StepWhat {
    let (_, what, _) = step_text(&StepRow {
        kind: edit.step,
        number: 1,
        before,
        after: &edit.doc,
    });
    what
}

/// The default song with a clip of three notes on its track: C4 at 96, E4 at 96 a little
/// late, G4 at 80.
fn three_notes() -> (Doc, ClipId) {
    let mut doc = Doc::new_default();
    let mut clip = None;
    for n in [note(0, 60, 96), note(Q + 70, 64, 96), note(2 * Q, 67, 80)] {
        let at = clip.map_or(WriteAt::Track(TrackId(1)), WriteAt::Clip);
        let made = notes::write_note(&doc, at, n).unwrap();
        clip = Some(made.clip);
        doc = made.edit.doc;
    }
    (doc, clip.unwrap())
}

fn piano() -> TrackRef {
    TrackRef {
        number: 1,
        name: "ピアノ".into(),
    }
}

#[test]
fn one_note_is_named_and_more_are_counted() {
    let empty = Doc::new_default();
    let written = notes::write_note(&empty, WriteAt::Track(TrackId(1)), note(0, 60, 96)).unwrap();
    assert_eq!(what(&empty, &written.edit), StepWhat::Note { key: 60 });

    let (doc, clip) = three_notes();
    // The note as it is after the step: moved up two keys.
    let moved = notes::move_notes(&doc, clip, &[1], 0, 2, false).unwrap();
    assert_eq!(what(&doc, &moved.edit), StepWhat::Note { key: 66 });
    let deleted = notes::delete_notes(&doc, clip, &[2], false).unwrap();
    assert_eq!(what(&doc, &deleted.edit), StepWhat::Note { key: 67 });
    let two = notes::delete_notes(&doc, clip, &[0, 2], false).unwrap();
    assert_eq!(what(&doc, &two.edit), StepWhat::Notes { count: 2 });
}

#[test]
fn a_velocity_step_says_from_and_to_when_the_notes_agree() {
    let (doc, clip) = three_notes();
    let same = notes::change_velocity(&doc, clip, &[0, 1], 14).unwrap();
    assert_eq!(
        what(&doc, &same.edit),
        StepWhat::Velocity {
            count: 2,
            from: Some(96),
            to: Some(110)
        }
    );
    let mixed = notes::change_velocity(&doc, clip, &[0, 1, 2], 4).unwrap();
    assert_eq!(
        what(&doc, &mixed.edit),
        StepWhat::Velocity {
            count: 3,
            from: None,
            to: None
        }
    );
}

#[test]
fn a_quantize_says_its_grid() {
    let (doc, clip) = three_notes();
    let settings = QuantizeSettings::default();
    let (quantized, _) = quantize::quantize(&doc, clip, &[], settings).unwrap();
    let StepKind::Quantize { count: n, .. } = quantized.edit.step else {
        panic!("a quantize step");
    };
    assert_eq!(
        what(&doc, &quantized.edit),
        StepWhat::Quantized {
            division: 16,
            triplet: false,
            count: n as u32
        }
    );
}

#[test]
fn control_points_say_which_control() {
    let (doc, clip) = three_notes();
    let sustain = controls::add_control(&doc, clip, Q, ModelControl::Cc(64), 127).unwrap();
    assert_eq!(
        what(&doc, &sustain.edit),
        StepWhat::ControlPoints {
            control: Some(ControlKind::Cc { number: 64 }),
            count: 1
        }
    );
    let with = sustain.edit.doc;
    let bend = controls::add_control(&with, clip, 2 * Q, ModelControl::PitchBend, 100).unwrap();
    assert_eq!(
        what(&with, &bend.edit),
        StepWhat::ControlPoints {
            control: Some(ControlKind::PitchBend),
            count: 1
        }
    );
    // Points of two controls in one step: no one name.
    let both = bend.edit.doc;
    let gone = controls::delete_controls(&both, clip, &[0, 1]).unwrap();
    assert_eq!(
        what(&both, &gone.edit),
        StepWhat::ControlPoints {
            control: None,
            count: 2
        }
    );
}

#[test]
fn one_clip_is_named() {
    let (doc, clip) = three_notes();
    let name = || StepWhat::Name {
        name: "ピアノ".into(),
    };
    let bar = 4 * Q as i64;
    let moved = arrange::move_clips(&doc, &[clip], 8 * bar, 0, false).unwrap();
    assert_eq!(what(&doc, &moved.edit), name());
    let copied = arrange::duplicate_clips(&doc, &[clip]).unwrap();
    assert_eq!(what(&doc, &copied.edit), name());
    let deleted = edit::delete_clips(&doc, &[clip], false).unwrap();
    assert_eq!(what(&doc, &deleted), name());

    let two = copied.edit.doc;
    let ids: Vec<ClipId> = two.tracks[0].clips.iter().map(|c| c.id).collect();
    let both = edit::delete_clips(&two, &ids, true).unwrap();
    assert_eq!(what(&two, &both), StepWhat::Clips { count: 2 });
}

#[test]
fn a_color_an_instrument_and_an_input_say_what_they_became() {
    let doc = Doc::new_default();
    let colored = edit::set_track_color(&doc, TrackId(1), TrackColor::new(3).unwrap()).unwrap();
    assert_eq!(
        what(&doc, &colored),
        StepWhat::Color {
            track: piano(),
            color: 3
        }
    );
    let organ = InstrumentRef::Builtin(BuiltinInstrument::Organ);
    let replaced = edit::set_instrument(&doc, TrackId(1), organ).unwrap();
    assert_eq!(
        what(&doc, &replaced),
        StepWhat::SoundReplaced {
            track: piano(),
            from: SoundName::Builtin {
                instrument: InstrumentKind::GrandPiano
            },
            to: SoundName::Builtin {
                instrument: InstrumentKind::Organ
            },
        }
    );

    let (added, vocal) = edit::add_vocal_track(&doc, None, meloet_model::Language::Ja).unwrap();
    assert_eq!(
        what(&doc, &added),
        StepWhat::Track {
            track: TrackRef {
                number: 2,
                name: "歌".into()
            }
        }
    );
    let with = added.doc;
    let input = edit::set_vocal_input(&with, vocal, VocalInput::Stereo12).unwrap();
    let (name, what, place) = step_text(&StepRow {
        kind: input.step,
        number: 1,
        before: &with,
        after: &input.doc,
    });
    assert_eq!(name, StepName::SetVocalInput);
    assert_eq!(
        what,
        StepWhat::VocalInput {
            track: TrackRef {
                number: 2,
                name: "歌".into()
            },
            from: InputView::Mono1,
            to: InputView::Stereo12,
        }
    );
    assert_eq!(place, Some(StepPlace::Arrange));
}
