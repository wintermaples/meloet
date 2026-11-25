#![allow(
    clippy::disallowed_methods,
    reason = "the test thread is the control side; it may sleep"
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use meloet_engine::{BlockTiming, Engine, EngineConfig, EngineParts, InputEnds, input_ends};
use meloet_model::{Meter, Tempo, Timeline};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::TrackType;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;

use super::*;

const RATE: u32 = 48_000;

fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "meloet-vocalrec-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The file as symphonia 0.6.1 reads it: (rate, channels, interleaved samples).
fn decode(path: &Path) -> (u32, usize, Vec<f32>) {
    let stream = MediaSourceStream::new(Box::new(File::open(path).unwrap()), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("wav");
    let mut format = symphonia::default::get_probe()
        .probe(&hint, stream, Default::default(), Default::default())
        .expect("a wav file");
    let track = format.default_track(TrackType::Audio).expect("a track");
    let id = track.id;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).unwrap();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .unwrap();
    let (mut rate, mut channels, mut data) = (0, 0, Vec::<f32>::new());
    while let Ok(Some(packet)) = format.next_packet() {
        if packet.track_id != id {
            continue;
        }
        let buffer = decoder.decode(&packet).unwrap();
        rate = buffer.spec().rate();
        channels = buffer.spec().channels().count();
        let at = data.len();
        data.resize(at + buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut data[at..]);
    }
    (rate, channels, data)
}

fn wave(frames: usize, channels: usize) -> Vec<f32> {
    (0..frames * channels)
        .map(|i| ((i / channels) as f32 * 0.01 + i as f32 * 0.5).sin() * 0.8)
        .collect()
}

#[test]
fn every_format_reads_back_before_and_after_the_lengths_are_put_in() {
    let dir = scratch("formats");
    let samples = wave(4_800, 2);
    for (format, step) in [
        (RecordFormat::Bits16, 2.5 / 32_767.0),
        (RecordFormat::Bits24, 1.0 / 8_388_607.0),
        (RecordFormat::Bits32, 0.0),
    ] {
        let path = dir.join(format!("{}.wav", format.bits()));
        let mut wav = WavFile::create(&path, RATE, 2, format).unwrap();
        wav.write(&samples[..4_000]).unwrap();
        wav.write_silence(100).unwrap();
        wav.write(&samples[4_000..]).unwrap();
        wav.sync().unwrap();
        assert_eq!(wav.frames(), 4_900);

        let check = |what: &str| {
            let (rate, channels, read) = decode(&path);
            assert_eq!((rate, channels, read.len()), (RATE, 2, 9_800), "{what}");
            let expected = [&samples[..4_000], &[0.0; 200], &samples[4_000..]].concat();
            let worst = (read.iter().zip(&expected))
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            assert!(worst <= step, "{what} {format:?}: off by {worst}");
            worst
        };
        // While it is written: the head says "to the end of the file".
        let head = fs::read(&path).unwrap();
        assert_eq!(head[4..8], [0xFF; 4]);
        assert_eq!(head[40..44], [0xFF; 4]);
        let worst = check("unfinished");
        if format == RecordFormat::Bits16 {
            assert!(worst > 0.2 / 32_767.0, "16 bits are dithered");
        }
        assert_eq!(wav.finish().unwrap(), 4_900);
        let head = fs::read(&path).unwrap();
        let data = 4_900 * 2 * u32::from(format.bits() / 8);
        assert_eq!(head[40..44], data.to_le_bytes());
        assert_eq!(head[4..8], (data + 36).to_le_bytes());
        check("finished");
    }
    assert!(WavFile::create(&dir.join("16.wav"), RATE, 2, RecordFormat::Bits16).is_err());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_file_left_half_written_is_mended() {
    let dir = scratch("mend");
    let path = dir.join("歌 7.wav");
    let samples = wave(1_000, 1);
    let mut wav = WavFile::create(&path, 44_100, 1, RecordFormat::Bits24).unwrap();
    wav.write(&samples).unwrap();
    drop(wav);
    // meloet ended in the middle of a frame.
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(&[1, 2]);
    fs::write(&path, &bytes).unwrap();

    let info = mend_wav(&path).unwrap();
    assert_eq!(
        info,
        WavInfo {
            frames: 1_000,
            sample_rate: 44_100,
            channels: 1
        }
    );
    let mended = fs::read(&path).unwrap();
    assert_eq!(mended.len(), 44 + 3_000);
    assert_eq!(mended[40..44], 3_000u32.to_le_bytes());
    assert_eq!(decode(&path).2.len(), 1_000);

    fs::write(dir.join("other.wav"), b"RIFFxxxxWAVEnope").unwrap();
    assert!(mend_wav(&dir.join("other.wav")).is_err());
    let _ = fs::remove_dir_all(dir);
}

#[cfg(target_os = "linux")]
#[test]
fn after_a_write_fails_nothing_more_goes_into_the_file() {
    let dir = scratch("failed");
    let path = dir.join("歌 8.wav");
    let samples = wave(1_000, 1);
    let mut wav = WavFile::create(&path, RATE, 1, RecordFormat::Bits24).unwrap();
    wav.write(&samples[..600]).unwrap();
    // The disk is full: a write stops part way, two bytes of a frame in.
    let full = File::options().write(true).open("/dev/full").unwrap();
    let mut disk = std::mem::replace(&mut wav.file, full);
    assert!(wav.write(&samples[600..700]).is_err());
    disk.write_all(&[1, 2]).unwrap();
    // Room again: the blocks after are not written out of step.
    wav.file = disk;
    assert!(wav.write(&samples[700..]).is_err());
    assert!(wav.write_silence(10).is_err());
    assert_eq!(wav.frames(), 600);
    assert_eq!(wav.finish().unwrap(), 600);

    assert_eq!(fs::metadata(&path).unwrap().len(), 44 + 600 * 3);
    let (_, channels, read) = decode(&path);
    assert_eq!((channels, read.len()), (1, 600));
    let worst = (read.iter().zip(&samples))
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(worst <= 1.0 / 8_388_607.0, "off by {worst}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_file_name_is_the_track_and_the_id() {
    assert_eq!(file_name("歌", 12), "歌 12.wav");
    assert_eq!(file_name(" コーラス/上 ", 3), "コーラス上 3.wav");
    assert_eq!(file_name("..", 4), "歌 4.wav");
}

#[test]
fn a_long_track_name_is_cut_to_fit_a_file_name() {
    let long = "歌".repeat(100);
    let name = file_name(&long, 12);
    // Room is left for the « (2)» of a taken name.
    assert!(name.len() + " (99)".len() <= 255, "{} bytes", name.len());
    assert!(name.starts_with("歌歌歌"), "{name}");
    assert!(name.ends_with("歌 12.wav"), "{name}");
    // Cut between two characters, with no space or dot left before the id.
    let name = file_name(&format!("{} .x", "a".repeat(199)), 3);
    assert_eq!(name, format!("{} 3.wav", "a".repeat(199)));
}

/// An engine with no stream, to fill a clock: block `n` reaches the DAC at `dac_us(n)`.
struct Out {
    parts: EngineParts,
    frames: u64,
}

const BLOCK: usize = 256;
const OUT_DELAY_US: u64 = 4_000;

impl Out {
    fn new() -> Out {
        Out {
            parts: Engine::new(EngineConfig::default()),
            frames: 0,
        }
    }

    fn block(&mut self) {
        let callback_us = 1_000_000 + self.frames * 1_000_000 / u64::from(RATE);
        let timing = BlockTiming {
            callback_us,
            playback_us: callback_us + OUT_DELAY_US,
        };
        self.parts.engine.process(&mut [0.0; BLOCK * 2], timing);
        self.frames += BLOCK as u64;
    }
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn the_writer_splits_the_input_into_the_tracks_files_and_places_their_head() {
    let dir = scratch("writer");
    let mut out = Out::new();
    let InputEnds {
        mut tap,
        record,
        meters,
        ..
    } = input_ends(2, RATE);
    meters.set_sink_monitor(true);
    let writer = DiskWriter::spawn();
    assert!(writer.stop().is_none());
    let job = |names: [&str; 2]| RecordJob {
        files: vec![
            RecordFile {
                path: dir.join(names[0]),
                channels: vec![1],
            },
            RecordFile {
                path: dir.join(names[1]),
                channels: vec![0, 1],
            },
        ],
        format: RecordFormat::Bits32,
        heard: None,
    };
    assert!(writer.start(job(["a.wav", "b.wav"])).is_err(), "no input");
    writer.set_input(Some((record, meters.clone())));

    // Input frame n is captured at 1 s + n / RATE, 10 ms after it was sung... as the PC
    // says: the time a block comes with is one block after its first frame.
    let samples = wave(BLOCK * 400, 2);
    let mut given = 0usize;
    let mut cycle = |out: &mut Out, tap: &mut meloet_engine::InputTap| {
        out.block();
        let capture_us = 1_000_000 + ((given + BLOCK) as u64) * 1_000_000 / u64::from(RATE);
        tap.block(
            &samples[2 * given..2 * (given + BLOCK)],
            capture_us,
            capture_us + 500,
        );
        given += BLOCK;
    };
    for _ in 0..10 {
        cycle(&mut out, &mut tap);
    }
    assert!(!meters.recording());
    let peaks = writer.start(job(["a.wav", "b.wav"])).unwrap();
    assert_eq!(peaks.len(), 2);
    assert!(meters.recording());
    assert!(dir.join("a.wav").exists() && dir.join("b.wav").exists());
    for _ in 0..20 {
        cycle(&mut out, &mut tap);
    }
    // The files begin with the first block that ends after this moment: the 16th block.
    let from_frame = 15 * BLOCK + 100;
    writer.from(1_000_000.0 + from_frame as f64 * 1e6 / f64::from(RATE));
    for _ in 0..70 {
        cycle(&mut out, &mut tap);
    }
    // Input frame n was captured at 1 s + n / RATE, and the files begin with frame
    // 15 * BLOCK: a moment is where counting from there puts it, also before the files.
    let moment = |frame: f64| 1_000_000.0 + frame * 1e6 / f64::from(RATE);
    for frame in [15.0 * BLOCK as f64, 20_000.25, 3_000.5] {
        let got = writer.mark(moment(frame)).expect("the files have blocks");
        let expected = frame - (15 * BLOCK) as f64;
        assert!((got - expected).abs() < 0.05, "{got} for {expected}");
    }
    // The wave is there while the recording goes on.
    assert!(peaks[0].lock().unwrap().frames() > 0);
    let written = writer.stop().expect("a recording");
    assert_eq!(writer.mark(moment(0.0)), None, "nothing is recorded");
    assert!(!meters.recording());
    assert_eq!(written.error, None);
    assert_eq!((written.dropped, written.gaps, written.full), (0, 0, false));
    assert_eq!(written.frames, (85 * BLOCK) as u64);
    assert_eq!(written.sample_rate, RATE);

    let (rate, channels, a) = decode(&dir.join("a.wav"));
    assert_eq!((rate, channels, a.len()), (RATE, 1, 85 * BLOCK));
    let from = 15 * BLOCK;
    assert!(
        a.iter()
            .enumerate()
            .all(|(i, s)| *s == samples[2 * (from + i) + 1])
    );
    let (_, channels, b) = decode(&dir.join("b.wav"));
    assert_eq!((channels, b.len()), (2, 2 * 85 * BLOCK));
    assert_eq!(b[..], samples[2 * from..2 * (from + 85 * BLOCK)]);

    // The peaks made while writing are the peaks of what the files hold, and they become
    // a peaks file on the writer's thread.
    for (peaks, (channels, file)) in peaks.iter().zip([(1, &a), (2, &b)]) {
        let mut whole = PeakBuilder::new(channels);
        whole.push(file);
        assert_eq!(*peaks.lock().unwrap(), whole);
    }
    let kept = dir.join("peaks/b.peaks");
    let answer = writer.write_peaks(peaks[1].clone(), kept.clone());
    assert!(answer.recv().unwrap().is_ok());
    let head = crate::peaks::read_peaks_head(&kept).unwrap();
    assert_eq!((head.channels, head.frames), (2, (85 * BLOCK) as u64));

    // The next recording starts from nothing; the files of the last one stay.
    assert!(
        writer.start(job(["a.wav", "c.wav"])).is_err(),
        "a.wav is there"
    );
    drop(writer);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_ring_that_ran_full_leaves_silence_of_the_same_length() {
    let dir = scratch("full");
    let InputEnds {
        mut tap,
        record,
        meters,
        ..
    } = input_ends(1, RATE);
    meters.set_sink_monitor(true);
    let second = wave(RATE as usize, 1);
    let mut t_us = 5_000_000u64;
    let mut give = |tap: &mut meloet_engine::InputTap, seconds: usize| {
        for _ in 0..seconds {
            for chunk in second.chunks(4_800) {
                t_us += 100_000;
                tap.block(chunk, t_us, t_us);
            }
        }
    };
    // The writer's side, driven by hand here so that the disk "stalls" when the test says.
    let mut side = Side::new(Arc::new(Mutex::new(Written::default())));
    side.input = Some((record, meters.clone()));
    side.into = vec![0.0; RATE as usize];
    let job = RecordJob {
        files: vec![RecordFile {
            path: dir.join("歌 1.wav"),
            channels: vec![0],
        }],
        format: RecordFormat::Bits16,
        heard: None,
    };
    side.start(job).unwrap();
    side.job.as_mut().unwrap().from_us = Some(0.0);
    // Seven seconds come and nobody reads: the ring holds four.
    give(&mut tap, 7);
    assert_eq!(meters.record_dropped(), 3 * u64::from(RATE));
    side.drain();
    give(&mut tap, 1);
    side.drain();
    side.end_job();
    let written = side.job.take().unwrap().said;
    assert_eq!(
        written.frames,
        8 * u64::from(RATE),
        "every frame has its place"
    );
    assert_eq!(written.dropped, 3 * u64::from(RATE));
    assert!(written.dropped_blocks > 0 && written.gap_count == 0);
    assert_eq!(
        (written.gaps, written.full, &written.error),
        (0, false, &None)
    );

    let (_, _, read) = decode(&dir.join("歌 1.wav"));
    assert_eq!(read.len(), 8 * RATE as usize);
    let near = |got: &[f32], want: &[f32]| {
        (got.iter().zip(want)).all(|(a, b)| (a - b).abs() < 3.0 / 32_767.0)
    };
    let rate = RATE as usize;
    for n in [0, 1, 2, 3, 7] {
        assert!(near(&read[n * rate..(n + 1) * rate], &second), "second {n}");
    }
    assert!(
        read[4 * rate..7 * rate]
            .iter()
            .all(|s| s.abs() < 3.0 / 32_767.0)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn what_the_latency_measurement_hears_is_kept_in_memory() {
    let InputEnds {
        mut tap,
        record,
        meters,
        ..
    } = input_ends(2, RATE);
    meters.set_sink_monitor(true);
    let writer = DiskWriter::spawn();
    writer.set_input(Some((record, meters.clone())));
    assert!(writer.captured().blocks.is_empty());
    writer.capture(true);
    wait_for("the switch", || meters.recording());
    let samples = wave(512, 2);
    tap.block(&samples[..512], 1_000, 1_100);
    tap.block(&samples[512..], 2_000, 2_100);
    let kept = writer.captured();
    assert_eq!(
        (kept.blocks.len(), kept.channels, kept.sample_rate),
        (2, 2, RATE)
    );
    assert_eq!(kept.first_frame(), 0);
    assert_eq!(kept.samples, samples);
    assert_eq!(
        kept.channel(1),
        samples
            .iter()
            .skip(1)
            .step_by(2)
            .copied()
            .collect::<Vec<_>>()
    );
    writer.capture(false);
    wait_for("the switch", || !meters.recording());
    assert!(writer.captured().blocks.is_empty());
}

fn flat(bpm_centi: u32) -> TempoMap {
    TempoMap::new(&Timeline::new(
        Tempo::from_centi(bpm_centi).unwrap(),
        Meter::FOUR_FOUR,
    ))
}

#[test]
fn takes_are_counted_from_the_frame_the_song_rolled_at() {
    // 120: a quarter (1920 ticks) is 24000 frames; a tick is 12.5 frames.
    let map = flat(12_000);
    let marks = Marks {
        from: 7_680,
        loop_range: None,
        // The file began half a beat (12000 frames) before the song rolled at 7680.
        roll: Some((12_000.4, 7_680.0)),
        laps: vec![],
    };
    // Stopped two beats and a bit in.
    let takes = takes_from(&marks, &map, RATE, 80_000, Some((11_520.3, 0)));
    assert_eq!(
        takes,
        [NewAudioTake {
            origin: 7_680,
            length: 3_841,
            first_frame: 12_000,
            frames: 48_013,
        }]
    );
    // The file is shorter than the stop says (the last block never came): the take ends
    // where the file does.
    let short = takes_from(&marks, &map, RATE, 36_000, Some((11_520.3, 0)));
    assert_eq!((short[0].length, short[0].frames), (1_919, 23_988));
    // meloet ended while recording: the file to its end.
    let ended = takes_from(&marks, &map, RATE, 80_000, None);
    assert_eq!((ended[0].first_frame, ended[0].frames), (12_000, 67_988));
    assert_eq!(ended[0].length, 5_439);
    // The head was never placed: the file's first frame counts as sung at `from`.
    let unplaced = Marks {
        roll: None,
        ..marks.clone()
    };
    let takes = takes_from(&unplaced, &map, RATE, 48_000, None);
    assert_eq!(
        takes[0],
        NewAudioTake {
            origin: 7_680,
            length: 3_840,
            first_frame: 0,
            frames: 48_000
        }
    );
    // A file that begins after the recording's start (it was started while playing).
    let late = Marks {
        roll: Some((-250.0, 7_680.0)),
        ..marks.clone()
    };
    let takes = takes_from(&late, &map, RATE, 48_000, None);
    assert_eq!((takes[0].origin, takes[0].first_frame), (7_700, 0));
    // Nothing recorded at all.
    assert!(takes_from(&marks, &map, RATE, 12_000, None).is_empty());
    assert!(takes_from(&marks, &map, RATE, 80_000, Some((7_680.0, 0))).is_empty());
}

#[test]
fn every_lap_of_a_loop_is_a_take_of_the_same_file() {
    let map = flat(12_000);
    // The loop is bar 2 (7680 to 15360: 96000 frames); the file began 12000 frames early.
    let marks = Marks {
        from: 7_680,
        loop_range: Some((7_680, 15_360)),
        roll: Some((12_000.0, 7_680.0)),
        laps: vec![108_000.0, 204_000.0, 300_000.0],
    };
    // Stopped a beat into the third lap; a fourth lap's mark came after the stop was asked.
    let takes = takes_from(&marks, &map, RATE, 400_000, Some((9_600.0, 2)));
    let lap = |first_frame, length, frames| NewAudioTake {
        origin: 7_680,
        length,
        first_frame,
        frames,
    };
    assert_eq!(
        takes,
        [
            lap(12_000, 7_680, 96_000),
            lap(108_000, 7_680, 96_000),
            lap(204_000, 1_920, 24_000),
        ]
    );
    // meloet ended in the fourth lap.
    let ended = takes_from(&marks, &map, RATE, 310_000, None);
    assert_eq!(ended.len(), 4);
    assert_eq!(ended[3], lap(300_000, 800, 10_000));
}

#[test]
fn the_note_is_written_whole_read_back_and_removed() {
    let dir = scratch("note");
    assert_eq!(read_note(&dir), None);
    let note = RecNote::new(
        Some(Path::new("/home/x/曲/曲.meloet")),
        RATE,
        RecordFormat::Bits24,
        vec![NoteFile {
            file: "歌 12.wav".into(),
            track: 6,
            track_name: "歌".into(),
            channels: 1,
        }],
        Marks {
            from: 7_680,
            loop_range: Some((7_680, 15_360)),
            roll: Some((12_000.25, 7_680.0)),
            laps: vec![108_000.25],
        },
    );
    write_note(&dir, &note).unwrap();
    assert!(dir.join("work/rec.json").exists());
    assert!(!dir.join("work/.rec.json.new").exists());
    assert_eq!(read_note(&dir), Some(note));
    remove_note(&dir);
    assert_eq!(read_note(&dir), None);

    fs::write(dir.join("work/rec.json"), b"{\"version\": 99}").unwrap();
    assert_eq!(read_note(&dir), None);
    assert!(!dir.join("work/rec.json").exists(), "put away");
    assert_eq!(work_audio_dir(&dir), dir.join("work/音"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cycles_the_input_missed_are_written_as_silence_of_their_length() {
    let dir = scratch("gaps");
    let InputEnds {
        mut tap,
        record,
        meters,
        ..
    } = input_ends(1, RATE);
    meters.set_sink_monitor(true);
    let mut side = Side::new(Arc::new(Mutex::new(Written::default())));
    side.input = Some((record, meters.clone()));
    side.into = vec![0.0; RATE as usize];
    side.start(RecordJob {
        files: vec![RecordFile {
            path: dir.join("歌 1.wav"),
            channels: vec![0],
        }],
        format: RecordFormat::Bits32,
        heard: None,
    })
    .unwrap();
    side.job.as_mut().unwrap().from_us = Some(0.0);

    // Blocks of 256 frames (5333 µs). The time of a block is its cycle's, a block after its
    // first frame; some wake a little late, which is no gap.
    let samples = wave(256 * 40, 1);
    let block_us = 256.0 * 1e6 / f64::from(RATE);
    let give = |tap: &mut meloet_engine::InputTap, n: usize, cycle: f64, late_us: f64| {
        let t_us = (1_000_000.0 + cycle * block_us + late_us) as u64;
        tap.block(&samples[256 * n..256 * (n + 1)], t_us, t_us);
    };
    for n in 0..10 {
        give(
            &mut tap,
            n,
            (n + 1) as f64,
            if n == 4 { 900.0 } else { 0.0 },
        );
    }
    // Three cycles never come.
    for n in 10..20 {
        give(&mut tap, n, (n + 4) as f64, 0.0);
    }
    side.drain();
    // A moment after the gap is where its frames are now.
    let job = side.job.as_mut().unwrap();
    let at = job.times.frame_at(1_000_000.0 + 15.0 * block_us).unwrap();
    // That moment is the first frame of the third block after the gap.
    assert!((at - (12 * 256 + 3 * 256) as f64).abs() < 0.05, "{at}");
    side.end_job();
    let written = side.job.take().unwrap().said;
    assert_eq!((written.gaps, written.dropped), (3 * 256, 0));
    assert_eq!((written.gap_count, written.dropped_blocks), (1, 0));
    assert_eq!(written.frames, 23 * 256);
    let (_, _, read) = decode(&dir.join("歌 1.wav"));
    assert_eq!(read[..2_560], samples[..2_560]);
    assert!(read[2_560..2_560 + 768].iter().all(|s| *s == 0.0));
    assert_eq!(read[2_560 + 768..], samples[2_560..5_120]);
    let _ = fs::remove_dir_all(dir);
}

/// The pieces of `tests/fixtures/input-blocks.txt`: the blocks as cpal gave them.
fn measured_blocks() -> Vec<(&'static str, Vec<InputBlock>)> {
    let mut pieces: Vec<(&str, Vec<InputBlock>)> = Vec::new();
    for line in include_str!("../../tests/fixtures/input-blocks.txt").lines() {
        if line.starts_with("##") {
            continue;
        }
        if let Some(name) = line.strip_prefix("# ") {
            pieces.push((name, Vec::new()));
            continue;
        }
        let (frames, capture_us) = line.split_once(' ').expect("two numbers");
        let blocks = &mut pieces.last_mut().expect("a piece").1;
        let frames: u32 = frames.parse().unwrap();
        blocks.push(InputBlock {
            first_frame: (blocks.last()).map_or(0, |b| b.first_frame + u64::from(b.frames)),
            frames,
            capture_us: capture_us.parse().unwrap(),
            callback_us: 0,
            dropped: false,
            sink_monitor: true,
        });
    }
    pieces
}

/// (frames filled, places) by the rule, and by the clock: how many frames the times of the
/// first and the last block say there are more than came.
fn filled(blocks: &[InputBlock]) -> ((u64, u32), f64) {
    let mut gaps = Gaps::default();
    let (mut frames, mut places, mut next) = (0u64, 0u32, 0u64);
    let mut write = |missed: u64, cut: u32, block: InputBlock, _: Option<f64>, samples: &[f32]| {
        assert_eq!(cut, 0, "with no clock of the engine, nothing is cut");
        assert_eq!(block.first_frame, next, "in the order they came");
        assert_eq!(samples.len(), block.frames as usize);
        next += u64::from(block.frames);
        frames += missed;
        places += u32::from(missed > 0);
    };
    let samples = [0.0f32; 1_024];
    for block in blocks {
        gaps.take(
            *block,
            None,
            &samples[..block.frames as usize],
            RATE,
            &mut write,
        );
    }
    gaps.settle(RATE, &mut write);
    let (first, last) = (blocks[0], blocks[blocks.len() - 1]);
    assert_eq!(
        next,
        last.first_frame + u64::from(last.frames),
        "every block"
    );
    let by_clock = (last.first_frame_us(RATE) - first.first_frame_us(RATE)) * f64::from(RATE) / 1e6
        - last.first_frame as f64;
    ((frames, places), by_clock)
}

#[test]
fn measured_holes_are_filled_and_a_block_whose_time_alone_is_late_is_not() {
    let pieces = measured_blocks();
    let piece = |name: &str| {
        let blocks = &pieces
            .iter()
            .find(|(n, _)| *n == name)
            .expect("the piece")
            .1;
        let (by_rule, by_clock) = filled(blocks);
        // What is filled is what the clock says is missing.
        assert!(
            (by_rule.0 as f64 - by_clock).abs() < 1.0,
            "{name}: {by_rule:?} {by_clock}"
        );
        by_rule
    };
    // A quiet PC, and a busy one at 256 frames: nothing.
    assert_eq!(piece("quiet-64"), (0, 0));
    assert_eq!(piece("busy-256"), (0, 0));
    // A busy PC at 64 frames: holes of one to three cycles, some one after the other.
    assert_eq!(piece("busy-64-holes-a"), (448, 5));
    assert_eq!(piece("busy-64-holes-b"), (512, 4));
    assert_eq!(piece("busy-64-holes-c"), (640, 9));
    // One block's time is a cycle late, the next is back on the line.
    assert_eq!(piece("busy-64-only-late"), (0, 0));
    // A cycle is missed and the first block after it is a cycle late by its time as well:
    // one cycle is filled there, not two. (b has three holes more.)
    assert_eq!(piece("load-64-hole-and-late-a"), (64, 1));
    assert_eq!(piece("load-64-hole-and-late-b"), (192 + 64 + 128 + 256, 4));
    assert_eq!(piece("load-64-hole-and-late-c"), (128 + 64, 2));
}

#[test]
fn late_times_of_one_or_two_blocks_fill_nothing_and_a_hole_at_the_end_is_filled() {
    let block_us = 64.0 * 1e6 / f64::from(RATE);
    let blocks = |cycles: &[f64]| -> Vec<InputBlock> {
        (cycles.iter().enumerate())
            .map(|(n, cycle)| InputBlock {
                first_frame: n as u64 * 64,
                frames: 64,
                capture_us: (1e6 + (cycle + 1.0) * block_us).round() as u64,
                callback_us: 0,
                dropped: false,
                sink_monitor: true,
            })
            .collect()
    };
    let rule = |cycles: &[f64]| filled(&blocks(cycles)).0;
    assert_eq!(rule(&[0.0, 1.0, 3.0, 3.0, 4.0, 5.0]), (0, 0));
    assert_eq!(rule(&[0.0, 1.0, 3.0, 4.0, 4.0, 5.0]), (0, 0));
    // Late for longer than is waited: a hole.
    assert_eq!(rule(&[0.0, 1.0, 3.0, 4.0, 5.0, 6.0]), (64, 1));
    // Two holes, one block between them.
    assert_eq!(rule(&[0.0, 1.0, 3.0, 4.0, 7.0, 8.0, 9.0]), (64 + 128, 2));
    // A hole of one cycle, and the block after it says two.
    assert_eq!(rule(&[0.0, 1.0, 4.0, 4.0, 5.0, 6.0]), (64, 1));
    assert_eq!(rule(&[0.0, 1.0, 4.0, 4.0]), (64, 1));
    // A hole, and another one right after its first block.
    assert_eq!(rule(&[0.0, 1.0, 3.0, 5.0, 6.0, 7.0]), (64 + 64, 2));
    // The recording ends on a late block: taken as it says.
    assert_eq!(rule(&[0.0, 1.0, 4.0]), (128, 1));
    assert_eq!(rule(&[0.0, 1.0, 4.0, 5.0]), (128, 1));
}

#[test]
fn a_block_whose_time_alone_is_late_is_written_where_the_count_says() {
    let dir = scratch("only-late");
    let InputEnds {
        mut tap,
        record,
        meters,
        ..
    } = input_ends(1, RATE);
    meters.set_sink_monitor(true);
    let mut side = Side::new(Arc::new(Mutex::new(Written::default())));
    side.input = Some((record, meters.clone()));
    side.into = vec![0.0; RATE as usize];
    side.start(RecordJob {
        files: vec![RecordFile {
            path: dir.join("歌 1.wav"),
            channels: vec![0],
        }],
        format: RecordFormat::Bits32,
        heard: None,
    })
    .unwrap();
    side.job.as_mut().unwrap().from_us = Some(0.0);

    let samples = wave(64 * 12, 1);
    let block_us = 64.0 * 1e6 / f64::from(RATE);
    // Block 5 says it came a cycle late; block 6 is where the count says.
    for n in 0..12 {
        let cycle = if n == 5 { n + 2 } else { n + 1 };
        let t_us = (1e6 + cycle as f64 * block_us) as u64;
        tap.block(&samples[64 * n..64 * (n + 1)], t_us, t_us);
        side.drain();
    }
    side.end_job();
    let written = side.job.take().unwrap().said;
    assert_eq!((written.gaps, written.gap_count), (0, 0));
    assert_eq!(written.frames, 64 * 12);
    let (_, _, read) = decode(&dir.join("歌 1.wav"));
    assert_eq!(read, samples);
    let _ = fs::remove_dir_all(dir);
}

/// An engine and an input of `BLOCK` frames on one clock, cycle after cycle, where either
/// may miss cycles (`out_misses`, `in_misses`) and an input block's time may be a cycle
/// late with nothing missing (`in_late`). Every sample sung says when it was sung. What was
/// written, and of every frame of the file that is not silence: the engine's frame heard
/// when it was sung, less the frame of the file.
fn heard_less_file(out_misses: &[u64], in_misses: &[u64], in_late: &[u64]) -> (Written, Vec<f64>) {
    const CYCLES: u64 = 160;
    let dir = scratch("heard");
    let block_us = BLOCK as f64 * 1e6 / f64::from(RATE);
    let cycle_us = |n: u64| 1e6 + n as f64 * block_us;
    let mut parts = Engine::new(EngineConfig::default());
    let clock = parts.handle.clock().clone();
    let InputEnds {
        mut tap,
        record,
        meters,
        ..
    } = input_ends(1, RATE);
    let mut side = Side::new(Arc::new(Mutex::new(Written::default())));
    side.input = Some((record, meters.clone()));
    side.into = vec![0.0; RATE as usize];
    side.start(RecordJob {
        files: vec![RecordFile {
            path: dir.join("歌 1.wav"),
            channels: vec![0],
        }],
        format: RecordFormat::Bits32,
        heard: Some(HeardLine {
            clock: clock.clone(),
            x_us: 0.0,
        }),
    })
    .unwrap();
    side.job.as_mut().unwrap().from_us = Some(0.0);
    // Sample i of cycle n was sung at the moment `sung(n * BLOCK + i)`.
    let sung = |frame: f64| cycle_us(0) - block_us + frame * 1e6 / f64::from(RATE);
    for n in 0..CYCLES {
        if !out_misses.contains(&n) {
            let callback_us = cycle_us(n) as u64;
            let timing = BlockTiming {
                callback_us,
                playback_us: callback_us + 2 * block_us as u64,
            };
            parts.engine.process(&mut [0.0; BLOCK * 2], timing);
        }
        // The engine's clock knows the moments a few cycles back.
        if n >= 4 && !in_misses.contains(&n) {
            let samples: Vec<f32> = (0..BLOCK as u64)
                .map(|i| (n * BLOCK as u64 + i) as f32)
                .collect();
            let late = if in_late.contains(&n) { block_us } else { 0.0 };
            // A source's time is its block's first frame, a block before the cycle.
            let capture_us = (cycle_us(n) - block_us + late) as u64;
            tap.block(&samples, capture_us, cycle_us(n) as u64);
        }
        side.drain();
    }
    side.end_job();
    let written = side.job.take().unwrap().said;
    let (_, _, read) = decode(&dir.join("歌 1.wav"));
    let apart = (read.iter().enumerate())
        .step_by(17)
        .filter(|(_, s)| **s != 0.0)
        .map(|(k, s)| clock.frame_at(sung(f64::from(*s))).unwrap() - k as f64)
        .collect();
    let _ = fs::remove_dir_all(dir);
    (written, apart)
}

#[test]
fn the_frames_of_the_file_stay_where_the_song_was_heard_whichever_side_misses_cycles() {
    let one_line = |apart: &[f64]| {
        let first = apart[0];
        assert!(
            apart.iter().all(|a| (a - first).abs() < 0.01),
            "{:?}",
            apart
                .iter()
                .map(|a| a - first)
                .filter(|a| a.abs() >= 0.01)
                .take(4)
                .collect::<Vec<_>>()
        );
    };
    let block = BLOCK as u64;
    // Nothing missed.
    let (written, apart) = heard_less_file(&[], &[], &[]);
    one_line(&apart);
    assert_eq!((written.gaps, written.cut), (0, 0));
    // The output alone misses a cycle, then two: as much of the input is left out.
    let (written, apart) = heard_less_file(&[40, 90, 91], &[], &[]);
    one_line(&apart);
    assert_eq!((written.gaps, written.gap_count), (0, 0));
    assert_eq!((written.cut, written.cut_count), (3 * block, 2));
    assert_eq!(written.frames, (156 - 3) * block);
    // The input alone misses a cycle: silence in its place, as before.
    let (written, apart) = heard_less_file(&[], &[60], &[]);
    one_line(&apart);
    assert_eq!(
        (written.gaps, written.gap_count, written.cut),
        (block, 1, 0)
    );
    // Both miss the same moment (what the output's cycle 70 would have played, two blocks
    // on, is what the input's cycle 73 would have taken, a block back): nothing to do.
    let (written, apart) = heard_less_file(&[70], &[73], &[]);
    one_line(&apart);
    assert_eq!((written.gaps, written.cut), (0, 0));
    assert_eq!(written.frames, 155 * block);
    // Both miss the same cycle: those are two moments, one filled and one left out.
    let (written, apart) = heard_less_file(&[70], &[70], &[]);
    one_line(&apart);
    assert_eq!((written.gaps, written.cut), (block, block));
    assert_eq!(written.frames, 155 * block);
    // A block whose time alone is late, next to the output missing a cycle.
    let (written, apart) = heard_less_file(&[101], &[], &[50, 100]);
    one_line(&apart);
    assert_eq!(
        (written.gaps, written.cut, written.cut_count),
        (0, block, 1)
    );
}

/// A measuring tool: the rule on every block of a run of
/// `meloet-engine/tests/input_gaps_private.rs` (`MELOET_GAPS_OUT`), named by
/// `MELOET_GAPS_BLOCKS`. Run with `-- --ignored --nocapture`.
#[test]
#[ignore = "reads a file of measured blocks"]
fn the_rule_fills_what_the_clock_says_is_missing_in_a_measured_run() {
    let path = std::env::var("MELOET_GAPS_BLOCKS").expect("MELOET_GAPS_BLOCKS names the file");
    let rows: Vec<[u64; 4]> = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let blocks: Vec<InputBlock> = (rows.iter())
        .map(
            |&[first_frame, frames, capture_us, callback_us]| InputBlock {
                first_frame: first_frame - rows[0][0],
                frames: frames as u32,
                capture_us,
                callback_us,
                dropped: false,
                sink_monitor: true,
            },
        )
        .collect();
    let ((frames, places), by_clock) = filled(&blocks);
    eprintln!(
        "{path}: {} blocks, the rule fills {frames} frames in {places} places, \
         the clock says {by_clock:.1} are missing",
        blocks.len()
    );
    let block = f64::from(blocks.iter().map(|b| b.frames).max().unwrap_or(0));
    assert!(
        (frames as f64 - by_clock).abs() < block,
        "more or less than is missing"
    );
}
