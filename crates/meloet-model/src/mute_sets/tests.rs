use super::*;
use crate::InstrumentRef;
use crate::edit::{add_track, delete_track, toggle_mute};
use crate::history::History;
use crate::mixer::toggle_return_mute;

const T1: TrackId = TrackId(1);

/// Three tracks: 1, 2 and 3.
fn three_tracks() -> Doc {
    let mut doc = Doc::new_default();
    for _ in 0..2 {
        let below = doc.tracks.last().map(|t| t.id);
        doc = add_track(&doc, below, InstrumentRef::SINE).unwrap().0.doc;
    }
    doc
}

fn ids(doc: &Doc) -> Vec<TrackId> {
    doc.tracks.iter().map(|t| t.id).collect()
}

fn muted(doc: &Doc) -> Vec<TrackId> {
    (doc.tracks.iter())
        .filter(|t| t.muted)
        .map(|t| t.id)
        .collect()
}

fn mute(doc: &Doc, track: TrackId) -> Doc {
    toggle_mute(doc, track).unwrap().doc
}

fn none() -> BTreeSet<TrackId> {
    BTreeSet::new()
}

#[test]
fn a_set_keeps_the_mutes_the_returns_and_the_solo() {
    let doc = three_tracks();
    let [_, t2, t3] = ids(&doc)[..] else {
        panic!("three tracks")
    };
    let doc = mute(&mute(&doc, t2), t3);
    let doc = toggle_return_mute(&doc, Bus::B).doc;
    let soloed = BTreeSet::from([T1]);
    let e = save_mute_set(&doc, &soloed, "  爽快セット ", false).unwrap();
    assert_eq!(e.step, StepKind::SaveMuteSet { index: 0 });
    assert_eq!(
        e.doc.mute_sets[..],
        [MuteSet {
            name: "爽快セット".into(),
            tracks: vec![T1, t2, t3],
            muted: vec![t2, t3],
            muted_returns: [false, true],
            soloed: vec![T1],
        }]
    );
    assert_eq!(e.doc.mute_set("爽快セット"), e.doc.mute_sets.first());
    assert_eq!(
        Doc {
            mute_sets: doc.mute_sets.clone(),
            ..e.doc.clone()
        },
        doc,
        "nothing else changes"
    );

    // The same name again only when it is to be written over.
    let doc = e.doc;
    assert_eq!(
        save_mute_set(&doc, &soloed, "爽快セット", false),
        Err(MuteSetError::Taken)
    );
    assert_eq!(
        save_mute_set(&doc, &soloed, "爽快セット", true),
        Err(MuteSetError::Unchanged)
    );
    assert_eq!(
        save_mute_set(&doc, &soloed, " ", true),
        Err(MuteSetError::BadName)
    );
    let other = mute(&doc, T1);
    let e = save_mute_set(&other, &none(), "クールセット", false).unwrap();
    assert_eq!(e.step, StepKind::SaveMuteSet { index: 1 });
    let e = save_mute_set(&e.doc, &none(), "爽快セット", true).unwrap();
    assert_eq!(e.step, StepKind::SaveMuteSet { index: 0 }, "in its place");
    let names: Vec<&str> = e.doc.mute_sets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["爽快セット", "クールセット"]);
    assert_eq!(e.doc.mute_sets[0].muted, vec![T1, t2, t3]);
    assert!(e.doc.mute_sets[0].soloed.is_empty());
}

#[test]
fn a_recall_mutes_as_the_set_says_and_gives_the_solo() {
    let doc = three_tracks();
    let [_, t2, t3] = ids(&doc)[..] else {
        panic!("three tracks")
    };
    // 「爽快」: 2 muted, 3 soloed. 「クール」: 1 and 3 muted, return A muted.
    let fresh = save_mute_set(&mute(&doc, t2), &BTreeSet::from([t3]), "爽快", false)
        .unwrap()
        .doc;
    let cool = toggle_return_mute(&mute(&mute(&mute(&fresh, t2), T1), t3), Bus::A).doc;
    let doc = save_mute_set(&cool, &none(), "クール", false).unwrap().doc;

    let r = recall_mute_set(&doc, &none(), "爽快").unwrap();
    let e = r.edit.expect("the mutes change");
    assert_eq!(e.step, StepKind::RecallMuteSet { index: 0 });
    assert_eq!(muted(&e.doc), [t2]);
    assert!(!e.doc.returns[0].muted && !e.doc.returns[1].muted);
    assert_eq!(r.soloed, BTreeSet::from([t3]));
    assert!(r.gone.is_empty() && r.not_in_set.is_empty());
    assert_eq!(e.doc.mute_sets, doc.mute_sets);

    let back = recall_mute_set(&e.doc, &r.soloed, " クール ").unwrap();
    let e2 = back.edit.unwrap();
    assert_eq!(muted(&e2.doc), [T1, t3]);
    assert!(e2.doc.returns[0].muted);
    assert!(back.soloed.is_empty());

    // As it is already: no edit, and the solo as the set has it.
    let same = recall_mute_set(&e2.doc, &BTreeSet::from([t2]), "クール").unwrap();
    assert_eq!(same.edit, None);
    assert!(same.soloed.is_empty());
    assert_eq!(
        recall_mute_set(&doc, &none(), "ない"),
        Err(MuteSetError::NoSuchSet)
    );
}

#[test]
fn a_track_gone_is_skipped_and_a_new_one_is_left_as_it_is() {
    let doc = three_tracks();
    let [_, t2, t3] = ids(&doc)[..] else {
        panic!("three tracks")
    };
    let doc = save_mute_set(&mute(&doc, t3), &BTreeSet::from([t3]), "A", false)
        .unwrap()
        .doc;
    let doc = delete_track(&doc, t3).unwrap().doc;
    let (e, t4) = add_track(&doc, Some(t2), InstrumentRef::SINE).unwrap();
    assert!(t4 > t3, "ids are not given again");
    let doc = mute(&mute(&e.doc, t4), T1);
    let r = recall_mute_set(&doc, &BTreeSet::from([t4]), "A").unwrap();
    assert_eq!(r.gone, [t3]);
    assert_eq!(r.not_in_set, [t4]);
    let e = r.edit.unwrap();
    assert_eq!(muted(&e.doc), [t4], "1 unmuted, the new track as it was");
    assert!(
        Arc::ptr_eq(&e.doc.tracks[2], &doc.tracks[2]),
        "untouched: shared"
    );
    assert_eq!(
        r.soloed,
        BTreeSet::from([t4]),
        "the gone track's solo is not kept"
    );
}

#[test]
fn delete_and_each_step_undoes_and_redoes() {
    let mut doc = three_tracks();
    let t2 = doc.tracks[1].id;
    let mut h = History::new();
    let start = doc.clone();

    let e = toggle_mute(&doc, t2).unwrap();
    h.apply(&mut doc, e);
    let e = save_mute_set(&doc, &none(), "A", false).unwrap();
    h.apply(&mut doc, e);
    let saved = doc.clone();
    let e = toggle_mute(&doc, t2).unwrap();
    h.apply(&mut doc, e);
    let e = save_mute_set(&doc, &none(), "B", false).unwrap();
    h.apply(&mut doc, e);
    let both = doc.clone();
    let r = recall_mute_set(&doc, &none(), "A").unwrap();
    h.apply(&mut doc, r.edit.unwrap());
    let recalled = doc.clone();
    assert_eq!(muted(&recalled), [t2]);
    let e = delete_mute_set(&doc, "A").unwrap();
    assert_eq!(e.step, StepKind::DeleteMuteSet { index: 0 });
    h.apply(&mut doc, e);
    assert_eq!(doc.mute_sets.len(), 1);
    assert_eq!(doc.mute_sets[0].name, "B");
    assert_eq!(delete_mute_set(&doc, "A"), Err(MuteSetError::NoSuchSet));

    let same = |a: &Doc, b: &Doc| {
        assert_eq!(
            Doc {
                next_id: 0,
                ..a.clone()
            },
            Doc {
                next_id: 0,
                ..b.clone()
            }
        )
    };
    h.undo(&mut doc);
    same(&doc, &recalled);
    h.undo(&mut doc);
    same(&doc, &both);
    h.undo(&mut doc);
    h.undo(&mut doc);
    same(&doc, &saved);
    h.undo(&mut doc);
    h.undo(&mut doc);
    same(&doc, &start);
    assert_eq!(h.jump(&mut doc, 6), 6);
    assert_eq!(doc.mute_sets.len(), 1);
    assert_eq!(muted(&doc), [t2]);
}

#[test]
fn a_song_holds_a_limited_number_of_sets() {
    let mut doc = Doc::new_default();
    for i in 0..MAX_MUTE_SETS {
        doc = save_mute_set(&doc, &none(), &format!("{i}"), false)
            .unwrap()
            .doc;
    }
    assert_eq!(
        save_mute_set(&doc, &none(), "one more", false),
        Err(MuteSetError::TooMany)
    );
    let over = mute(&doc, T1);
    assert!(
        save_mute_set(&over, &none(), "0", true).is_ok(),
        "written over"
    );
}

#[test]
fn a_mute_lane_that_moves_the_mute_rests_when_the_recall_changes_it() {
    use crate::automation::{AutoPoint, add_lane, put_points};
    let doc = three_tracks();
    let [_, t2, t3] = ids(&doc)[..] else {
        panic!("three tracks")
    };
    let doc = save_mute_set(&mute(&doc, t2), &none(), "A", false)
        .unwrap()
        .doc;
    let mut doc = mute(&doc, t2);
    for t in [t2, t3] {
        doc = add_lane(&doc, t, AutoTarget::Mute, true).unwrap().0.doc;
        let point = AutoPoint {
            tick: 0,
            value: 1.0,
        };
        doc = put_points(&doc, t, AutoTarget::Mute, &[point])
            .unwrap()
            .0
            .doc;
    }
    let r = recall_mute_set(&doc, &none(), "A").unwrap();
    assert_eq!(r.rested, [t2], "3 keeps its mute: its lane goes on");
    let e = r.edit.unwrap();
    let resting = |d: &Doc, t| d.track(t).unwrap().lane(AutoTarget::Mute).unwrap().resting;
    assert!(resting(&e.doc, t2) && !resting(&e.doc, t3));
    assert_eq!(muted(&e.doc), [t2]);
}
