use super::*;

/// A ramp from −1 to 1 over `frames`, mono.
fn ramp(frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| -1.0 + 2.0 * i as f32 / (frames - 1) as f32)
        .collect()
}

/// The peaks of `level` straight from the samples, for one channel of `channels`.
fn plain(samples: &[f32], channels: usize, channel: usize, level: usize) -> Vec<(i16, i16)> {
    let per = PEAK_LEVELS[level] as usize * channels;
    samples
        .chunks(per)
        .map(|chunk| {
            let of = chunk
                .iter()
                .skip(channel)
                .step_by(channels)
                .map(|&s| to_i16(s));
            (of.clone().min().unwrap(), of.max().unwrap())
        })
        .collect()
}

fn pairs(values: &[i16], channels: usize, channel: usize) -> Vec<(i16, i16)> {
    values
        .chunks_exact(2 * channels)
        .map(|p| (p[2 * channel], p[2 * channel + 1]))
        .collect()
}

#[test]
fn every_level_is_the_smallest_and_largest_of_its_frames() {
    // Not a whole number of peaks at any level.
    let samples = ramp(70_000);
    let mut b = PeakBuilder::new(1);
    b.push(&samples);
    assert_eq!(b.frames(), 70_000);
    for level in 0..5 {
        let expected = plain(&samples, 1, 0, level);
        assert_eq!(b.count(level) as usize, expected.len(), "level {level}");
        assert_eq!(pairs(&b.values(level, 0, u32::MAX), 1, 0), expected);
    }
    assert_eq!(b.count(0), 274);
    assert_eq!(b.count(4), 2);
}

#[test]
fn blocks_of_any_length_make_the_same_peaks() {
    let samples: Vec<f32> = (0..2 * 50_000)
        .map(|i| ((i * 7919) % 2001) as f32 / 1000.0 - 1.0)
        .collect();
    let mut whole = PeakBuilder::new(2);
    whole.push(&samples);
    let mut pieces = PeakBuilder::new(2);
    let mut rest = &samples[..];
    for len in [2, 510, 512, 514, 2 * 4096, 6, 2 * 30_000]
        .into_iter()
        .cycle()
    {
        if rest.is_empty() {
            break;
        }
        let (now, later) = rest.split_at(len.min(rest.len()));
        pieces.push(now);
        rest = later;
    }
    assert_eq!(whole, pieces);
    for channel in 0..2 {
        for level in 0..5 {
            assert_eq!(
                pairs(&whole.values(level, 0, u32::MAX), 2, channel),
                plain(&samples, 2, channel, level)
            );
        }
    }
}

#[test]
fn a_recording_shows_the_peak_that_is_not_whole_yet() {
    let mut b = PeakBuilder::new(1);
    assert_eq!(b.count(0), 0);
    assert!(b.reply(31, 0, 0, 100, true).values.is_empty());
    b.push(&[0.5; 100]);
    // One peak at every level, still open.
    for level in 0..5 {
        assert_eq!(b.count(level), 1);
        assert_eq!(b.values(level, 0, 9), [16_383, 16_383]);
    }
    b.push(&[-0.25; 300]);
    let reply = b.reply(31, 0, 0, 100, true);
    assert_eq!((reply.total, reply.count(), reply.growing), (2, 2, true));
    assert_eq!(reply.values, [-8191, 16_383, -8191, -8191]);
    // A range past the end is cut; one that starts past it is empty.
    assert_eq!(b.reply(31, 0, 1, 100, true).values, [-8191, -8191]);
    assert!(b.reply(31, 0, 2, 100, true).values.is_empty());
    assert_eq!(b.reply(31, 9, 0, 1, false).level, 4);
}

#[test]
fn samples_out_of_range_and_not_numbers() {
    let mut b = PeakBuilder::new(1);
    b.push(&[3.0, -3.0, f32::NAN]);
    assert_eq!(b.values(0, 0, 1), [-32_767, 32_767]);
}

#[test]
fn the_file_reads_back_what_was_asked_for() {
    let dir = std::env::temp_dir().join(format!("meloet-peaks-{}", std::process::id()));
    let path = peaks_path(&dir, &[0xab; 16]);
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        format!("{}.peaks", "ab".repeat(16))
    );
    let samples: Vec<f32> = (0..2 * 300_000)
        .map(|i| ((i * 31) % 1999) as f32 / 1000.0 - 1.0)
        .collect();
    let mut b = PeakBuilder::new(2);
    b.push(&samples);
    b.write(&path).unwrap();

    let head = read_peaks_head(&path).unwrap();
    assert_eq!((head.channels, head.frames), (2, 300_000));
    assert_eq!(head.counts, std::array::from_fn(|l| b.count(l)));
    for (level, first, count) in [(0u8, 0u32, 10u32), (0, 1000, 500), (2, 3, 1000), (4, 0, 9)] {
        let got = read_peaks(&path, 31, level, first, count).unwrap();
        assert_eq!(got, b.reply(31, level, first, count, false));
        assert_eq!(Peaks::decode(&got.encode()), Ok(got));
    }
    let place = PeaksPlace::File(path.clone());
    assert_eq!(
        place.reply(31, 1, 0, 4),
        Some(b.reply(31, 1, 0, 4, false).encode())
    );
    let shared = PeaksPlace::Growing(Arc::new(Mutex::new(b.clone())));
    assert_eq!(
        shared.reply(31, 1, 0, 4),
        Some(b.reply(31, 1, 0, 4, true).encode())
    );

    // A file cut short, or of something else, is no peaks file.
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - 2]).unwrap();
    assert!(read_peaks_head(&path).is_err());
    assert_eq!(place.reply(31, 0, 0, 4), None);
    std::fs::write(&path, b"RIFF....WAVE").unwrap();
    assert!(read_peaks(&path, 31, 0, 0, 4).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(place.reply(31, 0, 0, 4), None);
}

#[test]
fn ten_minutes_of_stereo_is_about_a_megabyte() {
    // 22-9: 48 kHz stereo, 10 minutes: 0.9 MB at the finest level, 1.2 MB for the five.
    let frames = 48_000u64 * 600;
    let finest = frames.div_ceil(256) * 2 * 4;
    assert_eq!(finest, 900_000);
    let all: u64 = PEAK_LEVELS
        .iter()
        .map(|&per| frames.div_ceil(u64::from(per)) * 8)
        .sum();
    assert!(all < 1_210_000, "{all}");
}
