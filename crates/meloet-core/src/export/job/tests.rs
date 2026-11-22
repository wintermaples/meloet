//! Jobs of the export against a real store in temporary folders: the song of
//! [`crate::export::fixture`] is rendered, and the files are read back.

use std::sync::atomic::AtomicUsize;

use meloet_model::{InstrumentRef, PluginRef, StripId, TempoMap, edit, mixer};
use meloet_protocol::{ExportFormat, PluginSlotRef, PluginWindowKind, RecordFormat};

use super::*;
use crate::audiofiles::decode::Decoder;
use crate::export::fixture::{BAR, BLOCK, Place, RATE, Run, read_wav, song};
use crate::plugins::hosts::DEFAULT_GEOMETRY;

/// How many frames the engine plays from `start` to `end`.
fn frames_of(doc: &Doc, start: u64, end: u64) -> usize {
    let map = TempoMap::new(&doc.timeline);
    map.frame_of(end as f64, map.seconds_at(start as f64), f64::from(RATE)) as usize
}

fn target(
    path: PathBuf,
    format: ExportFormat,
    sample_rate: u32,
    bits: RecordFormat,
) -> ExportTarget {
    ExportTarget {
        path,
        format,
        sample_rate,
        bits,
    }
}

fn finished(run: &Run) -> Vec<PathBuf> {
    let seen = run.until("finished", |n| {
        matches!(n, ExportNews::Finished { .. } | ExportNews::Failed { .. })
    });
    match seen.into_iter().last() {
        Some(ExportNews::Finished { files, .. }) => files,
        other => panic!("the job did not finish: {other:?}"),
    }
}

fn failed(run: &Run) -> ExportFailure {
    let seen = run.until("failed", |n| {
        matches!(
            n,
            ExportNews::Finished { .. }
                | ExportNews::Failed { .. }
                | ExportNews::Counted { done: true, .. }
        )
    });
    match seen.into_iter().last() {
        Some(ExportNews::Failed { reason, .. }) => reason,
        other => panic!("the job did not fail: {other:?}"),
    }
}

fn read_flac(path: &Path) -> (u32, Vec<f32>) {
    let mut flac = Decoder::open(path).unwrap();
    let mut out = Vec::new();
    while let Some((_, block)) = flac.next_block().unwrap() {
        out.extend_from_slice(block);
    }
    (flac.sample_rate(), out)
}

fn taker(path: &Path, head: u64, range: u64, tail: ExportTail) -> Taker {
    Taker {
        out: CacheWav::create(path, RATE, 2).unwrap(),
        head_left: head,
        range_left: range,
        tail,
        tail_frames: 0,
        tail_max: u64::from(EXPORT_TAIL_MAX_S) * u64::from(RATE),
        held: Vec::new(),
        quiet_for: 0,
        rate: RATE,
        peak: 0.0,
        places: OverPlaces::new(RATE),
        written: 0,
        over: false,
    }
}

/// Blocks of `BLOCK` frames whose every sample says which frame it is, as loud as `level`
/// says for the frame.
fn feed(taker: &mut Taker, level: impl Fn(u64) -> f32) -> u64 {
    let mut frame = 0u64;
    while !taker.over {
        let block: Vec<f32> = (frame..frame + BLOCK as u64)
            .flat_map(|f| [level(f), -level(f)])
            .collect();
        taker.take(&block).unwrap();
        frame += BLOCK as u64;
        assert!(frame < 100 * u64::from(RATE), "the tail never ends");
    }
    frame
}

#[test]
fn the_head_is_cut_and_the_tail_ends_when_it_is_quiet_or_after_ten_seconds() {
    let place = Place::new("export-taker");
    let path = place.root.join("t.wav");
    let range = 10_000u64;
    let loud = |frame: u64| 0.25 + (frame % 1_000) as f32 * 1e-4;

    // No tail: the range, and not the 300 frames the master delays everything by.
    let mut t = taker(&path, 300, range, ExportTail::Off);
    feed(&mut t, loud);
    assert_eq!((t.written, t.out.finish().unwrap()), (range, range));
    let back = read_wav(&path);
    assert_eq!(back.len(), 2 * range as usize);
    assert_eq!((back[0], back[1]), (loud(300), -loud(300)));
    assert_eq!(back[2 * 9_999], loud(10_299));

    // The sound is gone 5000 frames after the range: the file ends with the last block
    // that was not quiet, and the half second that was waited is not in it.
    let quiet_from = 300 + range + 5_000;
    let fading = |frame: u64| {
        if frame < quiet_from {
            loud(frame)
        } else {
            1e-5
        }
    };
    let mut t = taker(&path, 300, range, ExportTail::UntilSilent);
    let fed = feed(&mut t, fading);
    let last_loud_block = (quiet_from - 1) / BLOCK as u64;
    let end = (last_loud_block + 1) * BLOCK as u64 - 300;
    assert_eq!(t.out.finish().unwrap(), end);
    assert!(fed >= quiet_from + u64::from(RATE) / 2);
    assert!(fed < quiet_from + u64::from(RATE) / 2 + 2 * BLOCK as u64);

    // A quiet stretch shorter than half a second in the tail stays in the file.
    let gap = |frame: u64| match frame {
        f if f < 300 + range + 2_000 => loud(f),
        f if f < 300 + range + 20_000 => 0.0,
        f if f < 300 + range + 30_000 => loud(f),
        _ => 0.0,
    };
    let mut t = taker(&path, 300, range, ExportTail::UntilSilent);
    feed(&mut t, gap);
    let frames = t.out.finish().unwrap();
    assert!((range + 30_000..range + 30_000 + BLOCK as u64).contains(&frames));
    let back = read_wav(&path);
    assert_eq!(back[2 * (range as usize + 10_000)], 0.0);
    assert_eq!(
        back[2 * (range as usize + 25_000)],
        loud(300 + range + 25_000)
    );

    // A sound that never ends: ten seconds of it.
    let mut t = taker(&path, 0, range, ExportTail::UntilSilent);
    feed(&mut t, loud);
    assert_eq!(t.out.finish().unwrap(), range + 10 * u64::from(RATE));

    // Nothing after the range: the range.
    let only = |frame: u64| if frame < range { loud(frame) } else { 0.0 };
    let mut t = taker(&path, 0, range, ExportTail::UntilSilent);
    feed(&mut t, only);
    assert_eq!((t.peak, t.out.finish().unwrap()), (loud(999), range));
}

#[test]
fn the_places_over_0_db_are_runs_apart_by_100_ms() {
    let gap = u64::from(RATE) / 10;
    let count = |frames: &[(u64, f32)]| {
        let mut places = OverPlaces::new(RATE);
        let last = frames.iter().map(|(at, _)| *at).max().unwrap_or(0);
        let mut block = Vec::new();
        for frame in 0..=last + 10 {
            let level = (frames.iter().find(|(at, _)| *at == frame)).map_or(0.5, |(_, l)| *l);
            block.extend_from_slice(&[0.25, level]);
            if block.len() == 2 * BLOCK {
                places.feed(&block);
                block.clear();
            }
        }
        places.feed(&block);
        places.count
    };
    assert_eq!(count(&[]), 0);
    // Exactly 0 dB is not over.
    assert_eq!(count(&[(10, 1.0), (20, -1.0)]), 0);
    // A run of frames over is one place, and so are runs closer than 100 ms.
    let run: Vec<(u64, f32)> = (100..400).map(|f| (f, 1.2)).collect();
    assert_eq!(count(&run), 1);
    assert_eq!(count(&[(100, 1.1), (100 + gap - 1, -1.3)]), 1);
    // 100 ms not over between them: two places.
    assert_eq!(count(&[(100, 1.1), (100 + gap + 1, -1.3)]), 2);
    assert_eq!(
        count(&[(100, 1.1), (100 + gap + 1, 1.3), (100 + 3 * gap, 2.0)]),
        3
    );
    // Either channel.
    let mut places = OverPlaces::new(RATE);
    places.feed(&[1.5, 0.0, 0.0, 0.0]);
    assert_eq!(places.count, 1);

    // The taker counts only what goes into the file: not the head the master delays by.
    let place = Place::new("export-over");
    let path = place.root.join("o.wav");
    let mut t = taker(&path, 300, 10_000, ExportTail::Off);
    feed(&mut t, |frame| match frame {
        0..300 => 1.5,
        1_000 | 1_001 | 7_000 => 1.25,
        _ => 0.5,
    });
    assert_eq!(t.places.count, 2);
    t.out.abandon();
}

#[test]
fn the_tail_of_the_song_rings_on_and_without_one_the_file_ends_with_the_range() {
    let place = Place::new("export-tail");
    let (doc, _) = song();
    place.files_of(&doc);
    let span = (0, 2 * BAR);
    let range = frames_of(&doc, span.0, span.1);

    let off = place.start(place.setup(place.job(1, &doc, span, ExportTail::Off)));
    let (peak, seconds) = off.rendered();
    let cut = off.pass(0);
    assert_eq!(cut.len(), 2 * range);
    assert_eq!(seconds, 5, "two bars at 96 are five seconds");
    let largest = cut.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(largest > 0.05, "the song sounds");
    assert_eq!(peak, Some(20.0 * largest.log10()));

    let ringing = place.start(place.setup(place.job(2, &doc, span, ExportTail::UntilSilent)));
    ringing.rendered();
    let whole = ringing.pass(0);
    // The delay and the reverb ring on: more than half a second, and they do end.
    let more = whole.len() / 2 - range;
    assert!(more > RATE as usize / 2, "{more} frames of tail");
    assert!(more < 10 * RATE as usize, "{more} frames of tail");
    // Up to the range's end the two are the same file.
    assert!(
        whole[..2 * range] == cut[..],
        "the range differs with a tail"
    );
    // The song has stopped there: only what rings is left, and it dies away.
    let last = &whole[whole.len() - 2 * BLOCK..];
    assert!(last.iter().all(|s| s.abs() < 1e-3));
}

#[test]
fn each_track_gets_a_file_and_together_they_are_the_song() {
    let place = Place::new("export-tracks");
    let (doc, parts) = song();
    // Nothing on the master that makes the sum of the tracks another sound than the song.
    let doc = mixer::remove_effect(&doc, StripId::Master, 0).unwrap().doc;
    let doc = (edit::rename_track(&doc, parts.drums, "ドラム/キック?").unwrap()).doc;
    let doc = edit::rename_track(&doc, parts.vocals, "リード")
        .unwrap()
        .doc;
    let doc = edit::rename_track(&doc, parts.lead, "リード").unwrap().doc;
    place.files_of(&doc);
    let span = (0, 2 * BAR);
    let range = frames_of(&doc, span.0, span.1);

    let mix = place.start(place.setup(place.job(1, &doc, span, ExportTail::Off)));
    mix.rendered();
    let mix = mix.pass(0);

    let mut job = place.job(2, &doc, span, ExportTail::Off);
    job.per_track = true;
    let run = place.start(place.setup(job));
    run.rendered();
    let tracks: Vec<Vec<f32>> = (0..3).map(|n| run.pass(n)).collect();
    for track in &tracks {
        assert_eq!(track.len(), 2 * range);
        assert!(track.iter().any(|s| s.abs() > 0.01), "a track is silent");
    }
    let mut off = 0.0f32;
    for (i, &m) in mix.iter().enumerate() {
        let sum: f32 = tracks.iter().map(|t| t[i]).sum();
        off = off.max((sum - m).abs());
    }
    assert!(off < 1e-4, "the tracks add up to the song but for {off}");

    // The files are named as the tracks, in the folder chosen.
    let folder = place.root.join("out");
    run.handle.finish(target(
        folder.clone(),
        ExportFormat::Wav,
        RATE,
        RecordFormat::Bits32,
    ));
    let files = finished(&run);
    let names: Vec<String> = (files.iter())
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["リード.wav", "ドラムキック?.wav", "リード 2.wav"]);
    for (file, track) in files.iter().zip(&tracks) {
        assert_eq!(file.parent(), Some(folder.as_path()));
        assert!(read_wav(file) == *track);
    }
}

#[test]
fn the_file_has_the_form_asked_for() {
    let place = Place::new("export-forms");
    let (doc, _) = song();
    place.files_of(&doc);
    let span = (BAR, 2 * BAR);
    let range = frames_of(&doc, span.0, span.1);
    let out = place.root.join("out");
    let mut id = 0;
    // Renders the bar once more and brings it into a form.
    let mut export = |name: &str, format, rate, bits| -> (Vec<f32>, PathBuf) {
        id += 1;
        let run = place.start(place.setup(place.job(id, &doc, span, ExportTail::Off)));
        run.rendered();
        let floats = run.pass(0);
        let path = out.join(name);
        run.handle.finish(target(path.clone(), format, rate, bits));
        assert_eq!(finished(&run), std::slice::from_ref(&path));
        // The work place is empty again.
        let work = run.work.clone();
        run.handle.join();
        assert!(!work.exists());
        (floats, path)
    };

    let (floats, path) = export("f.wav", ExportFormat::Wav, RATE, RecordFormat::Bits32);
    assert_eq!(floats.len(), 2 * range);
    assert!(
        read_wav(&path) == floats,
        "32 bits are the floats as they are"
    );

    // Every run of the same song is the same audio.
    let (again, path) = export("w24.wav", ExportFormat::Wav, RATE, RecordFormat::Bits24);
    assert!(again == floats, "two runs differ");
    let whole = |v: &f32| (f64::from(v.clamp(-1.0, 1.0)) * 8_388_607.0).round() as i32;
    let wav24: Vec<i32> = (read_wav(&path).iter())
        .map(|v| (v * 8_388_608.0) as i32)
        .collect();
    assert!(wav24 == floats.iter().map(whole).collect::<Vec<i32>>());

    let (_, path) = export("f24.flac", ExportFormat::Flac, RATE, RecordFormat::Bits24);
    let (rate, flac) = read_flac(&path);
    assert_eq!(rate, RATE);
    let flac24: Vec<i32> = flac.iter().map(|v| (v * 8_388_608.0) as i32).collect();
    assert!(flac24 == wav24, "flac and wav hold other numbers");

    // 16 bits: the dither starts from the same seed, so two exports are the same file.
    let (_, a) = export("a16.wav", ExportFormat::Wav, RATE, RecordFormat::Bits16);
    let (_, b) = export("b16.wav", ExportFormat::Wav, RATE, RecordFormat::Bits16);
    assert!(std::fs::read(&a).unwrap() == std::fs::read(&b).unwrap());
    let (_, path) = export("f16.flac", ExportFormat::Flac, RATE, RecordFormat::Bits16);
    let wav16: Vec<i32> = (read_wav(&a).iter())
        .map(|v| (v * 32_768.0) as i32)
        .collect();
    let flac16: Vec<i32> = (read_flac(&path).1.iter())
        .map(|v| (v * 32_768.0) as i32)
        .collect();
    assert!(flac16 == wav16);

    // Another rate: as many frames as the time is long, rounded up.
    for (rate, name) in [(44_100u32, "44.flac"), (96_000, "96.flac")] {
        let (_, path) = export(name, ExportFormat::Flac, rate, RecordFormat::Bits24);
        let (said, audio) = read_flac(&path);
        assert_eq!(said, rate);
        let frames = (range as u64 * u64::from(rate)).div_ceil(u64::from(RATE));
        assert_eq!(audio.len() as u64, 2 * frames);
        // The same sound: as loud, give or take what the filter does to the peaks.
        let peak = |a: &[f32]| a.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((peak(&audio) - peak(&floats)).abs() < 0.1);
    }
}

#[test]
fn a_job_waits_for_audio_that_is_being_made_and_makes_what_nobody_makes() {
    let place = Place::new("export-made");
    let (doc, _) = song();
    place.files_of(&doc);
    let span = (4 * BAR, 6 * BAR);
    let stretch = place.root.join("cache").join(crate::audiofiles::CACHE_MADE);
    let made = || std::fs::read_dir(&stretch).map_or(0, |d| d.count());

    // The maker's thread is busy: nothing is counted, and nothing is made here.
    let busy = Arc::new(AtomicUsize::new(1));
    let mut setup = place.setup(place.job(1, &doc, span, ExportTail::Off));
    setup.maker_waiting = Some(busy.clone());
    let run = place.start(setup);
    assert!(run.news.recv_timeout(Duration::from_millis(500)).is_err());
    assert_eq!(made(), 0);
    // It is done, and did not make these: the job makes them itself.
    busy.store(0, Relaxed);
    run.rendered();
    assert_eq!(made(), 2, "the pitched clip and the stretched one");
    let first = run.pass(0);
    assert!(first.iter().any(|s| s.abs() > 0.05));

    // With the audio in the cache the job starts at once, and is the same audio.
    let run = place.start(place.setup(place.job(2, &doc, span, ExportTail::Off)));
    run.rendered();
    assert!(run.pass(0) == first);

    // The cache was emptied: made again, it is the same audio.
    std::fs::remove_dir_all(&stretch).unwrap();
    let job = place.job(3, &doc, span, ExportTail::Off);
    let run = place.start(place.setup(job));
    run.rendered();
    assert!(run.pass(0) == first, "made again, it is the same audio");
    assert_eq!(made(), 2);
}

#[test]
fn a_job_given_up_leaves_nothing_and_one_that_cannot_write_says_so() {
    let place = Place::new("export-ends");
    let (doc, _) = song();
    place.files_of(&doc);
    let span = (0, BAR);

    // Given up in the middle (at the speed it plays, so that there is a middle).
    let mut job = place.job(1, &doc, span, ExportTail::Off);
    job.realtime = true;
    let run = place.start(place.setup(job));
    run.until("counted", |n| matches!(n, ExportNews::Counted { .. }));
    assert!(run.work.join("pass-0.wav.part").exists());
    run.handle.give_up();
    let work = run.work.clone();
    run.handle.join();
    assert!(!work.exists());
    assert!(
        run.news
            .try_iter()
            .all(|n| matches!(n, ExportNews::Counted { done: false, .. }))
    );

    // Given up when it is rendered and waits for 「書き出す」.
    let run = place.start(place.setup(place.job(2, &doc, span, ExportTail::Off)));
    run.rendered();
    assert!(run.work.join("pass-0.wav").exists());
    let work = run.work.clone();
    drop(run.handle);
    let until = Instant::now() + Duration::from_secs(10);
    while work.exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!work.exists());

    // The place does not take the file: a file is where its folder should be.
    let blocked = place.root.join("blocked");
    std::fs::write(&blocked, b"x").unwrap();
    let run = place.start(place.setup(place.job(3, &doc, span, ExportTail::Off)));
    run.rendered();
    run.handle.finish(target(
        blocked.join("a.wav"),
        ExportFormat::Wav,
        RATE,
        RecordFormat::Bits24,
    ));
    assert_eq!(failed(&run), ExportFailure::CannotWrite);
    let work = run.work.clone();
    run.handle.join();
    assert!(!work.exists());
    assert_eq!(std::fs::read(&blocked).unwrap(), b"x");

    // The work place cannot be made.
    let mut setup = place.setup(place.job(4, &doc, span, ExportTail::Off));
    setup.work = blocked.join("work");
    let run = place.start(setup);
    assert_eq!(failed(&run), ExportFailure::CannotWrite);

    // What is there is written over, and nothing else is left beside it.
    let out = place.root.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("a.wav"), b"old").unwrap();
    let run = place.start(place.setup(place.job(5, &doc, span, ExportTail::Off)));
    run.rendered();
    run.handle.finish(target(
        out.join("a.wav"),
        ExportFormat::Wav,
        RATE,
        RecordFormat::Bits24,
    ));
    finished(&run);
    let names: Vec<_> = (std::fs::read_dir(&out).unwrap())
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["a.wav"]);
    assert_eq!(
        read_wav(&out.join("a.wav")).len(),
        2 * frames_of(&doc, 0, BAR)
    );
}

#[test]
fn a_plugin_that_does_not_load_fails_the_export_by_its_name() {
    let place = Place::new("export-plugin");
    let (doc, parts) = song();
    let key = "lv2:https://surge-synthesizer.github.io/lv2/surge-xt";
    let surge = InstrumentRef::Plugin(Arc::new(PluginRef::new(key, "Surge XT", "Surge", "")));
    let doc = edit::set_instrument(&doc, parts.drums, surge).unwrap().doc;
    place.files_of(&doc);
    let span = (0, BAR);
    let known = vec![(
        Arc::from(key),
        PathBuf::from("/usr/lib/lv2/Surge XT.lv2"),
        PluginWindowKind::Opens,
    )];
    let hosts = HostOptions {
        exe: PathBuf::from("/nowhere/meloet-plughost"),
        env: Vec::new(),
        geometry: DEFAULT_GEOMETRY,
    };

    let mut setup = place.setup(place.job(1, &doc, span, ExportTail::Off));
    setup.known = known.clone();
    setup.hosts = Some(hosts.clone());
    let run = place.start(setup);
    assert_eq!(
        failed(&run),
        ExportFailure::Plugin {
            name: "Surge XT".to_owned()
        }
    );
    let work = run.work.clone();
    run.handle.join();
    assert!(!work.exists());

    // One that has stopped is passed by: its track is silent, and the export goes on.
    let mut job = place.job(2, &doc, span, ExportTail::Off);
    job.skip = vec![PluginSlotRef::Instrument {
        track: parts.drums.0,
    }];
    let mut setup = place.setup(job);
    setup.known = known;
    setup.hosts = Some(hosts);
    let run = place.start(setup);
    run.rendered();
    let stopped = run.pass(0);
    assert!(stopped.iter().any(|s| s.abs() > 0.05));

    // One this PC does not have sounds as the sine wave, as it does while playing.
    let run = place.start(place.setup(place.job(3, &doc, span, ExportTail::Off)));
    run.rendered();
    assert!(run.pass(0) != stopped);
}
