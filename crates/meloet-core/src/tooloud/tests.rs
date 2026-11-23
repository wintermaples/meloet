use super::*;

const TICK: Duration = Duration::from_micros(16_667);

/// Readings 60 times a second, from `from` for `seconds`; the first finding and when.
fn run(
    w: &mut Watch,
    from: Duration,
    seconds: f64,
    mut reading: impl FnMut(usize, Duration) -> Reading,
) -> Option<(Found, Duration)> {
    let ticks = (seconds / TICK.as_secs_f64()).round() as usize;
    let mut first = None;
    for i in 0..ticks {
        let at = from + TICK * i as u32;
        if let Some(found) = w.read(reading(i, at)) {
            first.get_or_insert((found, at));
        }
    }
    first
}

fn limited(at: Duration, reduction_db: f32) -> Reading {
    Reading {
        at,
        playing: true,
        fresh: true,
        aside: false,
        limiter: true,
        reduction_db,
        peak: 0.9,
    }
}

fn bare(at: Duration, peak: f32) -> Reading {
    Reading {
        limiter: false,
        reduction_db: 0.0,
        peak,
        ..limited(at, 0.0)
    }
}

#[test]
fn a_limiter_holding_3_db_or_more_for_half_the_time_is_too_loud() {
    let mut w = Watch::default();
    // 6 dB and 1 dB by turns, half a second each: half the time at 6 dB.
    let (found, at) = run(&mut w, Duration::ZERO, 10.0, |i, at| {
        limited(at, if i / 30 % 2 == 0 { 6.0 } else { 1.0 })
    })
    .expect("found");
    assert!(found.limiter);
    assert!((found.by_db - 6.0).abs() < 1e-4, "{found:?}");
    // The window holds 2 s of 6 dB once about 4 s have gone by.
    assert!(
        at >= Duration::from_millis(3_400) && at <= Duration::from_millis(4_100),
        "{at:?}"
    );

    // Less than half the time: never.
    let mut w = Watch::default();
    let less = run(&mut w, Duration::ZERO, 20.0, |i, at| {
        limited(at, if i % 10 < 4 { 9.0 } else { 2.9 })
    });
    assert_eq!(less, None);
}

#[test]
fn the_mean_is_taken_over_the_time_the_limiter_held_3_db_or_more() {
    let mut w = Watch::default();
    let (found, at) = run(&mut w, Duration::ZERO, 10.0, |i, at| {
        limited(at, [4.0, 6.0, 0.0][i % 3])
    })
    .expect("found");
    assert!((found.by_db - 5.0).abs() < 0.01, "{found:?}");
    assert!(
        at < Duration::from_secs(4),
        "two thirds of the time: {at:?}"
    );
}

#[test]
fn without_a_limiter_three_readings_over_0_dbfs_are_too_loud() {
    let mut w = Watch::default();
    let peaks = |i: usize| match i {
        100 => 1.1,
        200 => 1.28,
        250 => 1.05,
        _ => 0.8,
    };
    let (found, at) = run(&mut w, Duration::ZERO, 6.0, |i, at| bare(at, peaks(i))).expect("found");
    assert!(!found.limiter);
    assert!(
        (found.by_db - 20.0 * 1.28f32.log10()).abs() < 1e-4,
        "{found:?}"
    );
    assert_eq!(at, TICK * 250);

    // Two, or three more than 4 s apart: never.
    let mut w = Watch::default();
    let spread = |i: usize| {
        if [10, 300, 600].contains(&i) {
            1.5
        } else {
            0.5
        }
    };
    assert_eq!(
        run(&mut w, Duration::ZERO, 12.0, |i, at| bare(at, spread(i))),
        None
    );
    // Exactly 0 dBFS is not over.
    let mut w = Watch::default();
    assert_eq!(
        run(&mut w, Duration::ZERO, 6.0, |_, at| bare(at, 1.0)),
        None
    );
}

#[test]
fn once_a_play_and_again_in_the_next() {
    let mut w = Watch::default();
    let loud = |_, at| limited(at, 8.0);
    let found = run(&mut w, Duration::ZERO, 20.0, loud);
    assert!(found.is_some());
    let mut again = 0;
    for i in 0..600 {
        again += usize::from(
            w.read(limited(Duration::from_secs(20) + TICK * i, 8.0))
                .is_some(),
        );
    }
    assert_eq!(again, 0, "not twice in a play");

    let stopped = Reading {
        playing: false,
        ..limited(Duration::from_secs(40), 8.0)
    };
    assert_eq!(w.read(stopped), None);
    let next = run(&mut w, Duration::from_secs(41), 5.0, loud);
    assert!(next.is_some(), "a new play");
}

#[test]
fn stopped_counting_in_the_test_tone_and_readings_with_no_block_are_not_looked_at() {
    // Stopped (or counting in: the same for the watch).
    let mut w = Watch::default();
    let stopped = |_, at| Reading {
        playing: false,
        ..bare(at, 2.0)
    };
    assert_eq!(run(&mut w, Duration::ZERO, 10.0, stopped), None);

    // The test tone over the song: its readings and the one after are left out.
    let mut w = Watch::default();
    let tone = |i: usize, at| Reading {
        aside: i.is_multiple_of(2),
        ..bare(at, if i.is_multiple_of(2) { 1.5 } else { 0.5 })
    };
    assert_eq!(run(&mut w, Duration::ZERO, 10.0, tone), None);
    let mut w = Watch::default();
    let after = |i: usize, at| Reading {
        aside: i.is_multiple_of(3),
        ..bare(at, if i % 3 == 1 { 1.5 } else { 0.5 })
    };
    assert_eq!(run(&mut w, Duration::ZERO, 10.0, after), None);

    // A reading with no block in between says nothing; the next stands for both.
    let mut w = Watch::default();
    let sparse = |i: usize, at| Reading {
        fresh: i.is_multiple_of(2),
        ..limited(at, if i.is_multiple_of(2) { 6.0 } else { 0.0 })
    };
    let (found, at) = run(&mut w, Duration::ZERO, 10.0, sparse).expect("found");
    assert!((found.by_db - 6.0).abs() < 1e-4);
    assert!(
        at <= Duration::from_millis(2_100),
        "all the time held: {at:?}"
    );
}

#[test]
fn the_notice_rounds_up_into_1_to_12() {
    let said = |by_db: f32, limiter| match notice(by_db, limiter) {
        StatusMessage::TooLoud {
            by_db,
            lower_db,
            limiter: l,
        } => {
            assert_eq!(l, limiter);
            (by_db, lower_db)
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(said(5.62, true), (5.6, 6));
    assert_eq!(said(5.0, true), (5.0, 5));
    assert_eq!(said(5.04, true), (5.0, 5));
    assert_eq!(said(2.06, false), (2.1, 3));
    assert_eq!(said(0.02, false), (0.0, 1));
    assert_eq!(said(30.0, true), (30.0, 12));
    assert_eq!(said(f32::NAN, false), (0.0, 1));
}
