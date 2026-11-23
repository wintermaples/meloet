use std::collections::BTreeMap;

use meloet_model::drums::DrumPiece;
use meloet_protocol::PatternGroup;

use super::*;

#[test]
fn the_twelve_patterns_of_the_spec_in_their_groups() {
    let ids: Vec<(&str, PatternGroup)> = all().iter().map(|p| (&p.id[..], p.group)).collect();
    use PatternGroup::{Basic, Fill};
    assert_eq!(
        ids,
        [
            ("eightBeat", Basic),
            ("eightBeatOpen", Basic),
            ("sixteenBeat", Basic),
            ("fourOnTheFloor", Basic),
            ("shuffle", Basic),
            ("halfTime", Basic),
            ("rideEightBeat", Basic),
            ("ballad", Basic),
            ("snareRoll", Fill),
            ("tomFill", Fill),
            ("crashStop", Fill),
            ("smallFill", Fill),
        ]
    );
    assert!(find("tomFill").is_some());
    assert!(find("nope").is_none());
}

#[test]
fn every_pattern_is_one_bar_on_its_grid_with_keys_of_the_kit() {
    for p in all() {
        let id = &p.id;
        assert_eq!(p.bars, 1, "{id}");
        assert_eq!(p.ticks(), 4 * meloet_model::TICKS_PER_QUARTER, "{id}");
        let grid = if id == "shuffle" { 640 } else { 480 };
        assert_eq!(
            p.grid, grid,
            "{id}: sixteenths, or triplets for the shuffle"
        );
        assert!(!p.notes.is_empty(), "{id}");
        for h in &p.notes {
            assert_eq!(h.at % p.grid, 0, "{id}: {h:?} is on the grid");
            assert!(h.length > 0 && h.at + h.length <= p.ticks(), "{id}: {h:?}");
            assert!((1..=127).contains(&h.velocity), "{id}: {h:?}");
            assert_eq!(DrumPiece::from_note(h.piece.note()), Some(h.piece));
        }
        let mut seen: Vec<(u64, DrumPiece)> = p.notes.iter().map(|h| (h.at, h.piece)).collect();
        seen.dedup();
        assert_eq!(seen.len(), p.notes.len(), "{id}: a sound once at a time");
        for n in p.notes_at(7_680) {
            assert!(n.start >= 7_680 && n.end() <= 2 * 7_680, "{id}");
            assert!(DrumPiece::from_note(n.key).is_some(), "{id}");
        }
    }
}

/// The feet play the kick and the pedal hi-hat; the hands everything else.
#[test]
fn a_person_can_play_each_one_two_hands_and_two_feet() {
    for p in all() {
        let mut at: BTreeMap<u64, (usize, usize)> = BTreeMap::new();
        for h in &p.notes {
            let (hands, feet) = at.entry(h.at).or_default();
            match h.piece {
                DrumPiece::Kick | DrumPiece::PedalHiHat => *feet += 1,
                _ => *hands += 1,
            }
        }
        for (tick, (hands, feet)) in at {
            assert!(hands <= 2 && feet <= 2, "{} at {tick}", p.id);
        }
    }
}

#[test]
fn they_have_strong_and_weak_hits() {
    // 「クラッシュで止める」 is one blow.
    for p in all().iter().filter(|p| p.id != "crashStop") {
        let low = p.notes.iter().map(|h| h.velocity).min().unwrap();
        let high = p.notes.iter().map(|h| h.velocity).max().unwrap();
        assert!(high >= low + 10, "{}: {low}..{high}", p.id);
        // The head of the bar is not weaker than what comes after it.
        let head = p.notes.iter().filter(|h| h.at == 0).map(|h| h.velocity);
        assert!(head.max().unwrap() >= 96, "{}", p.id);
    }
}

#[test]
fn a_bad_file_is_not_taken() {
    let one = |piece: &str| {
        format!(
            r#"{{"format": "meloet-drum-patterns", "version": 1, "patterns": [{{"id": "a",
            "group": "fill", "bars": 1, "grid": 480,
            "notes": [{{"piece": "{piece}", "at": 0, "length": 240, "velocity": 100}}]}}]}}"#
        )
    };
    assert_eq!(
        parse(&one("kick")).unwrap()[0].notes[0].piece,
        DrumPiece::Kick
    );
    assert!(parse(&one("cowbell")).is_err());
    assert!(parse(&one("kick").replace("\"version\": 1", "\"version\": 2")).is_err());
}
