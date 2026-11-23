use super::*;

const RATES: [u32; 3] = [44_100, 48_000, 96_000];

/// A sine on both sides, `hops` × `HOP_FRAMES` frames long (so the last frame ends a hop).
fn sine(rate: u32, hz: f64, amplitude: f64, hops: usize) -> Vec<[f32; 2]> {
    (0..hops * HOP_FRAMES)
        .map(|n| {
            let v = (amplitude * (TAU * hz * n as f64 / f64::from(rate)).sin()) as f32;
            [v, v]
        })
        .collect()
}

fn silence(frames: usize) -> Vec<[f32; 2]> {
    vec![[0.0; 2]; frames]
}

fn middle(band: usize) -> f64 {
    (edge(band) * edge(band + 1)).sqrt()
}

fn heard(rate: u32, frames: &[[f32; 2]]) -> Spectrum {
    let mut spectrum = Spectrum::new(rate);
    spectrum.push(frames);
    spectrum
}

#[test]
fn every_band_is_read_and_the_bins_go_up_without_overlapping() {
    for rate in RATES {
        let mut next = 1;
        for (i, band) in plan(rate).into_iter().enumerate() {
            match band {
                Band::Bins { from, to } => {
                    assert!(
                        from >= next && to >= from + 2,
                        "{rate} Hz, band {i}: {band:?}"
                    );
                    next = to;
                }
                Band::Middle { .. } => {}
                Band::Out => panic!("{rate} Hz, band {i} is out"),
            }
        }
        assert!(next <= FFT_FRAMES / 2);
    }
    // The lowest bands are narrower than two bins (23.4 Hz at 48 kHz, 46.9 at 96).
    let middles = |rate| {
        plan(rate)
            .iter()
            .take_while(|b| matches!(b, Band::Middle { .. }))
            .count()
    };
    assert_eq!((middles(48_000), middles(96_000)), (5, 7));
}

/// At 96 kHz the lowest bands are narrower than the FFT tells apart (a bin is 47 Hz): the sine
/// still reads highest in its own band, but by a step or two only.
#[test]
fn a_sine_lands_in_its_band_at_44_1_48_and_96_khz() {
    for rate in RATES {
        for band in 0..SPECTRUM_BANDS {
            let bars = heard(rate, &sine(rate, middle(band), 1.0, 8)).bars();
            let top = bars[band];
            assert!(top >= 245, "{rate} Hz, band {band} reads {top}: {bars:?}");
            for (other, &b) in bars.iter().enumerate() {
                if other != band {
                    assert!(
                        b < top,
                        "{rate} Hz, band {band}: band {other} reads {b}: {bars:?}"
                    );
                }
                if other.abs_diff(band) >= 4 {
                    assert!(
                        b + 30 <= top,
                        "{rate} Hz, band {band}: band {other} reads {b}: {bars:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_full_scale_sine_reads_at_the_top_and_a_quieter_one_lower_by_as_much() {
    let rate = 48_000;
    let band = 16;
    assert!(matches!(plan(rate)[band], Band::Bins { .. }));
    assert_eq!(
        heard(rate, &sine(rate, middle(band), 1.0, 8)).bars()[band],
        255
    );
    let quieter = heard(rate, &sine(rate, middle(band), 0.1, 8)).bars()[band];
    assert!(quieter.abs_diff(bar(-20.0)) <= 1, "{quieter}");
    // On one side only: half the power.
    let one: Vec<[f32; 2]> = sine(rate, middle(band), 1.0, 8)
        .into_iter()
        .map(|[l, _]| [l, 0.0])
        .collect();
    assert!(heard(rate, &one).bars()[band].abs_diff(bar(-3.01)) <= 1);
}

#[test]
fn silence_reads_nothing_and_takes_no_fft() {
    for rate in RATES {
        let quiet = heard(rate, &silence(rate as usize));
        assert_eq!((quiet.bars(), quiet.ffts()), ([0; SPECTRUM_BANDS], 0));
        // Under −80 dBFS a window is left out; a little louder, it is worked and still reads 0.
        let hush = heard(rate, &sine(rate, 1000.0, 0.9e-4, 40));
        assert_eq!((hush.bars(), hush.ffts()), ([0; SPECTRUM_BANDS], 0));
        let low = heard(rate, &sine(rate, 1000.0, 1.2e-4, 40));
        assert_eq!(low.bars(), [0; SPECTRUM_BANDS]);
        assert!(low.ffts() > 0);
    }
}

#[test]
fn the_bars_fall_30_db_a_second_and_rise_at_once() {
    let rate = 48_000;
    let band = 16;
    let mut spectrum = heard(rate, &sine(rate, middle(band), 1.0, 8));
    let start = spectrum.bars()[band];
    // About a second, ending on a hop.
    let frames = 47 * HOP_FRAMES;
    spectrum.push(&silence(frames));
    let fallen = 30.0 * frames as f64 / f64::from(rate) * 255.0 / 72.0;
    let after = spectrum.bars()[band];
    assert!(
        (f64::from(start) - fallen - f64::from(after)).abs() <= 1.0,
        "{start} → {after}"
    );
    spectrum.push(&sine(rate, middle(band), 1.0, 2));
    assert_eq!(spectrum.bars()[band], start, "back at once");
    spectrum.push(&silence(rate as usize * 3));
    assert_eq!(spectrum.bars(), [0; SPECTRUM_BANDS]);
}

#[test]
fn with_no_frames_the_bars_fall_by_the_time() {
    let rate = 48_000;
    let band = 10;
    let mut spectrum = heard(rate, &sine(rate, middle(band), 1.0, 8));
    let start = spectrum.bars()[band];
    for _ in 0..50 {
        spectrum.fall_for(Duration::from_millis(10));
    }
    let after = spectrum.bars()[band];
    assert!(
        (f64::from(start) - 15.0 * 255.0 / 72.0 - f64::from(after)).abs() <= 1.0,
        "{start} → {after}"
    );
}

#[test]
fn a_restart_starts_from_silence_at_the_new_rate() {
    let mut spectrum = heard(48_000, &sine(48_000, 1000.0, 1.0, 8));
    assert!(spectrum.bars().iter().any(|&b| b > 0));
    spectrum.restart(96_000);
    assert_eq!(spectrum.bars(), [0; SPECTRUM_BANDS]);
    assert_eq!(spectrum.bands, plan(96_000));
    // What came before is not in the next window: one hop of silence reads nothing.
    spectrum.push(&silence(HOP_FRAMES));
    assert_eq!(spectrum.bars(), [0; SPECTRUM_BANDS]);
    spectrum.push(&sine(96_000, middle(12), 1.0, 8));
    assert!(spectrum.bars()[12] >= 245);
}

#[test]
fn after_the_stream_stopped_the_frames_before_do_not_come_back() {
    let rate = 48_000;
    let mut spectrum = heard(rate, &sine(rate, middle(12), 1.0, 8));
    // Half a hop more: the next window would still hold the sine.
    spectrum.push(&sine(rate, middle(12), 1.0, 1)[..HOP_FRAMES / 2]);
    let ffts = spectrum.ffts();
    spectrum.fall_for(Duration::from_millis(10));
    let fallen = spectrum.bars();
    spectrum.push(&silence(2 * HOP_FRAMES));
    assert_eq!(spectrum.ffts(), ffts, "the window is silence");
    assert!(
        spectrum
            .bars()
            .iter()
            .zip(fallen)
            .all(|(&now, before)| now <= before)
    );
}

#[test]
fn a_bar_never_holds_over_its_top() {
    let rate = 48_000;
    let band = 16;
    let mut loud = heard(rate, &sine(rate, middle(band), 100.0, 8));
    let mut full = heard(rate, &sine(rate, middle(band), 1.0, 8));
    assert_eq!((loud.bars()[band], full.bars()[band]), (255, 255));
    loud.push(&silence(47 * HOP_FRAMES));
    full.push(&silence(47 * HOP_FRAMES));
    // The loud one starts falling a hop later (its last window still holds the sine at +37 dB),
    // not 40 dB later.
    let (loud, full) = (loud.bars()[band], full.bars()[band]);
    assert!(full < loud && loud <= full + 3, "{loud}, {full}");
}
