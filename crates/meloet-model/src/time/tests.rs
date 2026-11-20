use super::*;
use crate::tempo_map::{TempoMap, Timeline};

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn bbt(tick: u64, num: u8, den: u8) -> (u64, u32, u32) {
    let head = Tempo::from_centi(9_600).unwrap();
    let map = TempoMap::new(&Timeline::new(head, Meter::new(num, den).unwrap()));
    let b = map.bar_beat_tick(tick);
    (b.bar, b.beat, b.tick)
}

#[test]
fn four_four() {
    assert_eq!(bbt(0, 4, 4), (1, 1, 0));
    assert_eq!(bbt(Q - 1, 4, 4), (1, 1, (Q - 1) as u32));
    assert_eq!(bbt(Q, 4, 4), (1, 2, 0));
    assert_eq!(bbt(BAR - 1, 4, 4), (1, 4, (Q - 1) as u32));
    assert_eq!(bbt(BAR, 4, 4), (2, 1, 0));
    assert_eq!(bbt(BAR + 2 * Q + 123, 4, 4), (2, 3, 123));
}

#[test]
fn three_four() {
    assert_eq!(bbt(3 * Q - 1, 3, 4), (1, 3, (Q - 1) as u32));
    assert_eq!(bbt(3 * Q, 3, 4), (2, 1, 0));
    assert_eq!(bbt(3 * Q * 4 + Q + 20, 3, 4), (5, 2, 20));
}

#[test]
fn six_eight_counts_eighths() {
    // One beat is an eighth note: Q / 2 ticks.
    assert_eq!(bbt(Q / 2, 6, 8), (1, 2, 0));
    assert_eq!(bbt(Q / 2 + 10, 6, 8), (1, 2, 10));
    assert_eq!(bbt(3 * Q - 1, 6, 8), (1, 6, (Q / 2 - 1) as u32));
    assert_eq!(bbt(3 * Q, 6, 8), (2, 1, 0));
}

#[test]
fn two_two_counts_halves() {
    // One beat is a half note: 2 Q ticks.
    assert_eq!(bbt(2 * Q - 1, 2, 2), (1, 1, (2 * Q - 1) as u32));
    assert_eq!(bbt(2 * Q, 2, 2), (1, 2, 0));
    assert_eq!(bbt(2 * Q + 1, 2, 2), (1, 2, 1));
    assert_eq!(bbt(BAR, 2, 2), (2, 1, 0));
}

#[test]
fn seven_sixteen() {
    // One beat is a sixteenth: Q / 4 ticks.
    assert_eq!(bbt(Q / 4 + 5, 7, 16), (1, 2, 5));
    assert_eq!(bbt(7 * Q / 4 - 1, 7, 16), (1, 7, (Q / 4 - 1) as u32));
    assert_eq!(bbt(7 * Q / 4, 7, 16), (2, 1, 0));
}

#[test]
fn meter_range() {
    assert!(Meter::new(0, 4).is_none());
    assert!(Meter::new(1, 4).is_some());
    assert!(Meter::new(12, 16).is_some());
    assert!(Meter::new(13, 4).is_none());
    for den in [0, 1, 3, 32] {
        assert!(Meter::new(4, den).is_none(), "{den}");
    }
    assert_eq!(Meter::new(4, 4).unwrap().ticks_per_bar(), BAR);
    assert_eq!(Meter::new(6, 8).unwrap().ticks_per_beat(), Q / 2);
}

#[test]
fn clicks_of_the_metronome() {
    let clicks = |num, den| {
        let m = Meter::new(num, den).unwrap();
        (m.ticks_per_click(), m.clicks_per_bar())
    };
    assert_eq!(clicks(4, 4), (Q, 4));
    assert_eq!(clicks(3, 4), (Q, 3));
    assert_eq!(clicks(6, 8), (3 * Q / 2, 2));
    assert_eq!(clicks(9, 8), (3 * Q / 2, 3));
    assert_eq!(clicks(12, 8), (3 * Q / 2, 4));
    assert_eq!(clicks(5, 8), (Q / 2, 5));
    assert_eq!(clicks(7, 8), (Q / 2, 7));
    assert_eq!(clicks(3, 8), (Q / 2, 3));
    assert_eq!(clicks(2, 2), (2 * Q, 2));
    assert_eq!(clicks(6, 4), (Q, 6));
    assert_eq!(clicks(6, 16), (Q / 4, 6));
    assert_eq!(clicks(12, 16), (Q / 4, 12));
}

#[test]
fn tempo_range_and_steps() {
    assert!(Tempo::from_centi(2_999).is_none());
    assert_eq!(Tempo::from_centi(3_000).unwrap().bpm(), 30.0);
    assert_eq!(Tempo::from_centi(30_000).unwrap().bpm(), 300.0);
    assert!(Tempo::from_centi(30_001).is_none());
    assert_eq!(Tempo::from_bpm(96.004).unwrap().centi(), 9_600);
    assert_eq!(Tempo::from_bpm(120.256).unwrap().centi(), 12_026);
    assert!(Tempo::from_bpm(f64::NAN).is_none());
    assert!(Tempo::from_bpm(-96.0).is_none());
    assert!(Tempo::from_bpm(1e30).is_none());
}

#[test]
fn seconds() {
    let four = Meter::FOUR_FOUR;
    let t96 = Tempo::from_centi(9_600).unwrap();
    let seconds = |tick: u64, tempo, meter| {
        TempoMap::new(&Timeline::new(tempo, meter)).seconds_at(tick as f64)
    };
    assert!((seconds(Q, t96, four) - 0.625).abs() < 1e-12);
    assert!((seconds(BAR, t96, four) - 2.5).abs() < 1e-12);
    let t120 = Tempo::from_centi(12_000).unwrap();
    let six_eight = Meter::new(6, 8).unwrap();
    assert!((seconds(Q / 2, t120, six_eight) - 0.25).abs() < 1e-12);
    assert!((seconds(six_eight.ticks_per_bar(), t120, six_eight) - 1.5).abs() < 1e-12);
    assert_eq!(seconds(0, t120, six_eight), 0.0);
}

#[test]
fn ranges() {
    assert!(TickRange::new(5, 5).is_none());
    assert!(TickRange::new(6, 5).is_none());
    let a = TickRange::new(0, 10).unwrap();
    let b = TickRange::new(10, 20).unwrap();
    let c = TickRange::new(9, 11).unwrap();
    assert!(!a.overlaps(b), "touching ends do not overlap");
    assert!(a.overlaps(c) && b.overlaps(c));
    assert_eq!(a.union(b), TickRange::new(0, 20).unwrap());
    assert_eq!((c.start(), c.end()), (9, 11));
}
