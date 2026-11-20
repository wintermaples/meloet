use super::*;
use crate::tempo_map::{TempoMap, Timeline};

fn bbt(tick: u64, num: u8, den: u8) -> (u64, u32, u32) {
    let head = Tempo::from_centi(9_600).unwrap();
    let map = TempoMap::new(&Timeline::new(head, Meter::new(num, den).unwrap()));
    let b = map.bar_beat_tick(tick);
    (b.bar, b.beat, b.tick)
}

#[test]
fn four_four() {
    assert_eq!(bbt(0, 4, 4), (1, 1, 0));
    assert_eq!(bbt(479, 4, 4), (1, 1, 479));
    assert_eq!(bbt(480, 4, 4), (1, 2, 0));
    assert_eq!(bbt(1919, 4, 4), (1, 4, 479));
    assert_eq!(bbt(1920, 4, 4), (2, 1, 0));
    assert_eq!(bbt(1920 + 960 + 123, 4, 4), (2, 3, 123));
}

#[test]
fn three_four() {
    assert_eq!(bbt(1439, 3, 4), (1, 3, 479));
    assert_eq!(bbt(1440, 3, 4), (2, 1, 0));
    assert_eq!(bbt(1440 * 4 + 500, 3, 4), (5, 2, 20));
}

#[test]
fn six_eight_counts_eighths() {
    // One beat is an eighth note: 240 ticks, shown as 480.
    assert_eq!(bbt(240, 6, 8), (1, 2, 0));
    assert_eq!(bbt(250, 6, 8), (1, 2, 20));
    assert_eq!(bbt(1439, 6, 8), (1, 6, 478));
    assert_eq!(bbt(1440, 6, 8), (2, 1, 0));
}

#[test]
fn two_two_counts_halves() {
    // One beat is a half note: 960 ticks, shown as 480.
    assert_eq!(bbt(959, 2, 2), (1, 1, 479));
    assert_eq!(bbt(960, 2, 2), (1, 2, 0));
    assert_eq!(bbt(961, 2, 2), (1, 2, 0));
    assert_eq!(bbt(1920, 2, 2), (2, 1, 0));
}

#[test]
fn seven_sixteen() {
    // One beat is a sixteenth: 120 ticks, shown as 480.
    assert_eq!(bbt(125, 7, 16), (1, 2, 20));
    assert_eq!(bbt(839, 7, 16), (1, 7, 476));
    assert_eq!(bbt(840, 7, 16), (2, 1, 0));
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
    assert_eq!(Meter::new(4, 4).unwrap().ticks_per_bar(), 1920);
    assert_eq!(Meter::new(6, 8).unwrap().ticks_per_beat(), 240);
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
    assert!((seconds(480, t96, four) - 0.625).abs() < 1e-12);
    assert!((seconds(1920, t96, four) - 2.5).abs() < 1e-12);
    let t120 = Tempo::from_centi(12_000).unwrap();
    let six_eight = Meter::new(6, 8).unwrap();
    assert!((seconds(240, t120, six_eight) - 0.5).abs() < 1e-12);
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
