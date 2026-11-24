//! Jobs of the export against a real store in temporary folders: the song of
//! [`crate::export::fixture`] is rendered, and the files are read back.

use std::sync::atomic::AtomicUsize;

use meloet_model::{InstrumentRef, PluginRef, StripId, TempoMap, edit, mixer};
use meloet_protocol::{ExportFormat, PluginSlotRef, PluginWindowKind, RecordFormat};

use super::*;
use crate::ai::{AiAnalysis, AiBandLayout};
use crate::audiofiles::decode::Decoder;
use crate::export::Measuring;
use crate::export::fixture::{BAR, BLOCK, Place, RATE, Run, loud, read_wav, song};
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
        loudness: ExportLoudness::AsIs,
        mp3_kbps: meloet_protocol::EXPORT_MP3_DEFAULT_KBPS,
        info: Default::default(),
    }
}

fn finished(run: &Run) -> Vec<PathBuf> {
    finished_with_wave(run).0
}

fn finished_with_wave(run: &Run) -> (Vec<PathBuf>, Vec<u8>) {
    let seen = run.until("finished", |n| {
        matches!(n, ExportNews::Finished { .. } | ExportNews::Failed { .. })
    });
    match seen.into_iter().last() {
        Some(ExportNews::Finished { files, wave, .. }) => (files, wave),
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
        squares: 0.0,
        written: 0,
        over: false,
        listen: None,
        true_peak: None,
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

/// The last word of a job that is rendered: the peak and the places over 0 dB.
fn checked(run: &Run) -> (Option<f32>, u32) {
    let seen = run.until("rendered", |n| {
        matches!(
            n,
            ExportNews::Counted { done: true, .. } | ExportNews::Failed { .. }
        )
    });
    match seen.last() {
        Some(ExportNews::Counted { peak_db, over, .. }) => (*peak_db, *over),
        other => panic!("the job failed: {other:?}"),
    }
}

#[test]
fn the_check_hears_what_goes_over_0_db_and_only_the_whole_numbers_cut_it() {
    let place = Place::new("export-loud");
    let (doc, _) = song();
    let doc_loud = loud(&doc, false);
    place.files_of(&doc_loud);
    let span = (0, 2 * BAR);
    let largest = |a: &[f32]| a.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let out = place.root.join("out");
    let mut id = 0;
    let mut export = |doc: &Doc, name: &str, bits| {
        id += 1;
        let run = place.start(place.setup(place.job(id, doc, span, ExportTail::Off)));
        let (peak, over) = checked(&run);
        let floats = run.pass(0);
        let path = out.join(name);
        run.handle
            .finish(target(path.clone(), ExportFormat::Wav, RATE, bits));
        assert_eq!(finished(&run), std::slice::from_ref(&path));
        (peak, over, floats, read_wav(&path))
    };

    // Nothing on the master stops it: the check counts what the faders make, before any cut.
    let (peak, over, floats, wav32) = export(&doc_loud, "loud32.wav", RecordFormat::Bits32);
    let top = largest(&floats);
    assert!(top > 1.0, "the song does not go over 0 dB: {top}");
    assert!(over > 0);
    let peak = peak.unwrap();
    assert!(peak > 0.0, "{peak} dB");
    assert_eq!(peak, 20.0 * top.log10());
    // 32 bit floats keep it as it is.
    assert!(wav32 == floats);
    // 16 and 24 bits cut it where they are written.
    for (name, bits) in [
        ("loud24.wav", RecordFormat::Bits24),
        ("loud16.wav", RecordFormat::Bits16),
    ] {
        let (_, again, same, wav) = export(&doc_loud, name, bits);
        assert_eq!(again, over);
        assert!(same == floats, "two runs differ");
        assert!(largest(&wav) <= 1.0, "{name}: {}", largest(&wav));
        assert!(largest(&wav) > 0.99, "{name}: {}", largest(&wav));
    }

    // With the master's limiter, nothing goes over.
    let (peak, over, floats, _) = export(&loud(&doc, true), "limited.wav", RecordFormat::Bits32);
    assert_eq!(over, 0);
    assert!(largest(&floats) <= 1.0);
    assert!(peak.unwrap() < 0.0, "{peak:?} dB");
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

/// What a job that measures answered.
fn measured(run: &Run) -> (Vec<PassLevels>, Option<Box<AiAnalysis>>) {
    let seen = run.until("measured", |n| {
        matches!(n, ExportNews::Measured { .. } | ExportNews::Failed { .. })
    });
    match seen.into_iter().last() {
        Some(ExportNews::Measured {
            passes, analysis, ..
        }) => (passes, analysis),
        other => panic!("the job did not measure: {other:?}"),
    }
}

fn measuring(alone: Vec<TrackId>, ignore_mute: bool) -> Measuring {
    Measuring {
        alone,
        ignore_mute,
        bands: None,
        compare: Vec::new(),
        cuts: Vec::new(),
    }
}

#[test]
fn a_muted_track_is_measured_alone_as_if_it_sounded_and_the_mix_keeps_it_muted() {
    let place = Place::new("export-measure-mute");
    let (doc, parts) = song();
    place.files_of(&doc);
    let muted = edit::toggle_mute(&doc, parts.lead).unwrap().doc;
    let span = (0, 2 * BAR);
    let measure = |id: u64, doc: &Doc, ignore_mute: bool| {
        let mut job = place.job(id, doc, span, ExportTail::Off);
        job.per_track = true;
        job.measure = Some(measuring(vec![parts.lead, parts.drums], ignore_mute));
        measured(&place.start(place.setup(job))).0
    };
    let sounding = measure(1, &doc, false);
    let kept = measure(2, &muted, false);
    let ignored = measure(3, &muted, true);
    assert_eq!(sounding.len(), 3);
    assert!(sounding[1].peak > 0.01, "{sounding:?}");
    // Muted and not ignored: silent alone, and out of the mix.
    assert_eq!(kept[1].peak, 0.0, "{kept:?}");
    assert!(kept[0].rms < sounding[0].rms, "{kept:?} {sounding:?}");
    // Ignored: alone it sounds as it does unmuted, and the mix still leaves it out.
    assert_eq!(ignored[1], sounding[1]);
    assert_eq!(ignored[2], sounding[2]);
    assert_eq!(ignored[0], kept[0]);
    assert!(ignored[1].lufs.is_some() && kept[1].lufs.is_none());
}

#[test]
fn a_job_that_measures_bands_and_sections_answers_for_every_rendering() {
    let place = Place::new("export-measure-bands");
    let (doc, parts) = song();
    place.files_of(&doc);
    let mut job = place.job(1, &doc, (0, 4 * BAR), ExportTail::Off);
    job.per_track = true;
    job.measure = Some(Measuring {
        alone: vec![parts.lead, parts.drums, parts.vocals],
        ignore_mute: false,
        bands: Some(AiBandLayout::Octave),
        compare: vec![0, 2],
        cuts: vec![BAR, 3 * BAR],
    });
    let (passes, analysis) = measured(&place.start(place.setup(job)));
    let analysis = analysis.expect("bands were asked for");
    assert_eq!(passes.len(), 4);
    assert_eq!(analysis.bands.len(), 10);
    assert_eq!(analysis.levels.len(), 4);
    assert!(analysis.levels.iter().all(|l| l.len() == 10));
    let shares: f64 = analysis.levels[0].iter().map(|b| b.share).sum();
    assert!(shares > 0.9 && shares <= 1.0 + 1e-9, "{shares}");
    let pairs: Vec<(usize, usize)> = analysis.overlaps.iter().map(|o| (o.a, o.b)).collect();
    assert_eq!(pairs, [(0, 2)]);
    let edges: Vec<(u64, u64)> = (analysis.sections.iter())
        .map(|s| (s.start, s.end))
        .collect();
    assert_eq!(edges, [(0, BAR), (BAR, 3 * BAR), (3 * BAR, 4 * BAR)]);
    let frames: u64 = analysis.sections.iter().map(|s| s.passes[0].frames).sum();
    assert_eq!(frames, passes[0].frames);
    assert!(analysis.sections.iter().all(|s| s.passes.len() == 4));
}

/// The integrated loudness and the true peak (dBTP) of interleaved stereo, by ebur128 alone.
fn remeasured(samples: &[f32], rate: u32) -> (f32, f32) {
    use ebur128::{EbuR128, Mode};
    let mut meter = EbuR128::new(2, rate, Mode::I | Mode::TRUE_PEAK).unwrap();
    meter.add_frames_f32(samples).unwrap();
    let peak = (0..2)
        .map(|c| meter.true_peak(c).unwrap())
        .fold(0.0, f64::max);
    (
        meter.loudness_global().unwrap() as f32,
        (20.0 * peak.log10()) as f32,
    )
}

/// Ten seconds at [`RATE`]: a 1 kHz sine of `amplitude`, and, with `bursts`, a millisecond
/// at −6 dB every two seconds.
fn fixed_signal(amplitude: f32, bursts: bool) -> Vec<f32> {
    use std::f32::consts::TAU;
    (0..10 * RATE as usize)
        .flat_map(|i| {
            let t = (i % RATE as usize) as f32 / RATE as f32;
            let loud = bursts && i % (2 * RATE as usize) < 48;
            let level = if loud { 0.5 } else { amplitude };
            let s = level * (TAU * 1_000.0 * t).sin();
            [s, s * 0.9]
        })
        .collect()
}

/// Writes `samples` as a job renders them, measures them as the job does, and brings them
/// into `target` as 「書き出す」 does. Returns what was measured and what was written.
fn converted(
    place: &Place,
    name: &str,
    samples: &[f32],
    target: &ExportTarget,
) -> (Option<f32>, Option<f32>, Vec<f32>) {
    let from = place.root.join(format!("{name}.render.wav"));
    let mut wav = CacheWav::create(&from, RATE, 2).unwrap();
    let mut listen = Listener::new(RATE, None, &[]);
    let mut true_peak = TruePeak::new(RATE).unwrap();
    for block in samples.chunks(2 * BLOCK) {
        wav.write(block).unwrap();
        listen.feed(block);
        true_peak.feed(block);
    }
    wav.finish().unwrap();
    let (lufs, peak) = (listen.finish().lufs, true_peak.db());
    let gain = match target.loudness {
        ExportLoudness::Match => loudness::match_gain(lufs, peak).map(|m| m.gain_db),
        ExportLoudness::AsIs => None,
    };
    let stop = AtomicBool::new(false);
    std::fs::create_dir_all(target.path.parent().unwrap()).unwrap();
    if convert(&from, &target.path, RATE, target, gain, &stop).is_err() {
        panic!("{name} was not converted");
    }
    let out = match target.format {
        ExportFormat::Wav => read_wav(&target.path),
        ExportFormat::Flac => read_flac(&target.path).1,
        ExportFormat::Mp3 | ExportFormat::Midi => unreachable!("not a format of this test"),
    };
    (lufs, peak, out)
}

fn matched(path: PathBuf, format: ExportFormat, rate: u32, bits: RecordFormat) -> ExportTarget {
    ExportTarget {
        loudness: ExportLoudness::Match,
        ..target(path, format, rate, bits)
    }
}

#[test]
fn matched_fixed_signals_measure_minus_14_lufs_or_stop_at_minus_1_dbtp() {
    let place = Place::new("export-match");
    let out = |name: &str| place.root.join("out").join(name);

    // Quiet: brought up to −14 LUFS.
    let quiet = fixed_signal(0.05, false);
    let target = matched(
        out("quiet.wav"),
        ExportFormat::Wav,
        RATE,
        RecordFormat::Bits32,
    );
    let (lufs, peak, file) = converted(&place, "quiet", &quiet, &target);
    let (lufs, peak) = (lufs.unwrap(), peak.unwrap());
    // The check's loudness is BS.1770's, as ebur128 has it too.
    let (again, again_peak) = remeasured(&quiet, RATE);
    assert!((lufs - again).abs() < 0.05, "{lufs} / {again}");
    assert!((peak - again_peak).abs() < 0.01, "{peak} / {again_peak}");
    assert!(lufs < -20.0 && peak < -20.0, "{lufs} LUFS {peak} dBTP");
    let (lufs, peak) = remeasured(&file, RATE);
    assert!((lufs + 14.0).abs() <= 0.2, "{lufs} LUFS");
    assert!(peak < -1.0, "{peak} dBTP");

    // Loud: brought down to −14 LUFS.
    let loud = fixed_signal(0.9, false);
    let target = matched(
        out("loud.flac"),
        ExportFormat::Flac,
        RATE,
        RecordFormat::Bits24,
    );
    let (before, _, file) = converted(&place, "loud", &loud, &target);
    assert!(before.unwrap() > -5.0, "{before:?}");
    let (lufs, _) = remeasured(&file, RATE);
    assert!((lufs + 14.0).abs() <= 0.2, "{lufs} LUFS");

    // Quiet with short peaks: up only until the true peak is at −1 dBTP.
    let peaky = fixed_signal(0.05, true);
    let target = matched(
        out("peaky.wav"),
        ExportFormat::Wav,
        RATE,
        RecordFormat::Bits24,
    );
    let (before, _, file) = converted(&place, "peaky", &peaky, &target);
    let (lufs, peak) = remeasured(&file, RATE);
    assert!((peak + 1.0).abs() <= 0.2, "{peak} dBTP");
    assert!(lufs < -14.5 && lufs > before.unwrap(), "{lufs} LUFS");
    let file_peak = file.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(file_peak < 1.0, "24 bits do not clip: {file_peak}");

    // Brought to another rate: the same, give or take what the resampler does.
    for rate in [44_100, 96_000] {
        let name = format!("quiet-{rate}.flac");
        let target = matched(out(&name), ExportFormat::Flac, rate, RecordFormat::Bits24);
        let (_, _, file) = converted(&place, &name, &quiet, &target);
        let (lufs, _) = remeasured(&file, rate);
        assert!((lufs + 14.0).abs() <= 0.3, "{rate} Hz: {lufs} LUFS");
        let name = format!("peaky-{rate}.wav");
        let target = matched(out(&name), ExportFormat::Wav, rate, RecordFormat::Bits32);
        let (_, _, file) = converted(&place, &name, &peaky, &target);
        let (_, peak) = remeasured(&file, rate);
        assert!((peak + 1.0).abs() <= 0.3, "{rate} Hz: {peak} dBTP");
    }
}

#[test]
fn as_is_writes_the_samples_as_they_were_rendered() {
    let place = Place::new("export-as-is");
    let signal = fixed_signal(0.05, true);
    let path = place.root.join("out").join("as-is.wav");
    let plain = target(path, ExportFormat::Wav, RATE, RecordFormat::Bits32);
    let (_, _, file) = converted(&place, "as-is", &signal, &plain);
    assert!(file == signal);

    // Silence cannot be matched: written as it is.
    let silence = vec![0.0f32; 2 * RATE as usize];
    let path = place.root.join("out").join("silence.wav");
    let target = matched(path, ExportFormat::Wav, RATE, RecordFormat::Bits32);
    let (lufs, peak, file) = converted(&place, "silence", &silence, &target);
    assert_eq!((lufs, peak), (None, None));
    assert!(file == silence);
}

/// The loudness and the true peak in the last word of a rendered job.
fn loudness_said(run: &Run) -> (Option<f32>, Option<f32>) {
    let seen = run.until("rendered", |n| {
        matches!(
            n,
            ExportNews::Counted { done: true, .. } | ExportNews::Failed { .. }
        )
    });
    match seen.last() {
        Some(ExportNews::Counted {
            lufs, true_peak_db, ..
        }) => (*lufs, *true_peak_db),
        other => panic!("the job failed: {other:?}"),
    }
}

#[test]
fn a_job_measures_its_file_and_matches_it_and_one_for_each_track_does_not() {
    let place = Place::new("export-job-match");
    let (doc, _) = song();
    place.files_of(&doc);
    let span = (0, 2 * BAR);
    let run = place.start(place.setup(place.job(1, &doc, span, ExportTail::UntilSilent)));
    let (lufs, peak) = loudness_said(&run);
    let floats = run.pass(0);
    let (lufs, peak) = (lufs.unwrap(), peak.unwrap());
    // The true peak hears the tail; the loudness only the range.
    let range = 2 * frames_of(&doc, span.0, span.1);
    assert!(floats.len() > range, "the song rings on");
    let (whole_lufs, whole_peak) = remeasured(&floats, RATE);
    let (range_lufs, _) = remeasured(&floats[..range], RATE);
    assert!((peak - whole_peak).abs() < 0.01, "{peak} / {whole_peak}");
    assert!((lufs - range_lufs).abs() < 0.05, "{lufs} / {range_lufs}");
    assert!(
        (lufs - whole_lufs).abs() > 1e-4,
        "the tail is not in the loudness"
    );

    let path = place.root.join("out").join("matched.wav");
    run.handle.finish(matched(
        path.clone(),
        ExportFormat::Wav,
        RATE,
        RecordFormat::Bits24,
    ));
    let (files, wave) = finished_with_wave(&run);
    assert_eq!(files, std::slice::from_ref(&path));
    let file = read_wav(&path);
    assert_eq!(file.len(), floats.len());
    // E9: the waveform is of the file as written, the gain on.
    let bars = meloet_protocol::EXPORT_WAVE_BARS;
    assert_eq!(wave.len(), bars);
    let frames = file.len() / 2;
    let mut largest = vec![0.0f32; bars];
    for (i, frame) in file.chunks(2).enumerate() {
        let bar = i * bars / frames;
        largest[bar] = largest[bar].max(frame[0].abs()).max(frame[1].abs());
    }
    for (said, l) in wave.iter().zip(&largest) {
        let read = (l.min(1.0) * 255.0).round();
        assert!((f32::from(*said) - read).abs() <= 1.0, "{wave:?}");
    }
    assert!(wave.iter().any(|b| *b > 100), "{wave:?}");
    let m = loudness::match_gain(Some(lufs), Some(peak)).unwrap();
    let (lufs_after, peak_after) = remeasured(&file, RATE);
    let at_target = (lufs_after + 14.0).abs() <= 0.2;
    let at_ceiling = (peak_after + 1.0).abs() <= 0.2;
    assert!(
        at_target || at_ceiling,
        "{lufs_after} LUFS {peak_after} dBTP"
    );
    assert_eq!(!at_target, m.limited, "{m:?}");

    // For each track: nothing is measured, and nothing is matched.
    let mut job = place.job(2, &doc, span, ExportTail::Off);
    job.per_track = true;
    let run = place.start(place.setup(job));
    assert_eq!(loudness_said(&run), (None, None));
    let first = run.pass(0);
    let folder = place.root.join("tracks");
    run.handle.finish(matched(
        folder,
        ExportFormat::Wav,
        RATE,
        RecordFormat::Bits32,
    ));
    let (files, wave) = finished_with_wave(&run);
    assert!(read_wav(&files[0]) == first);
    assert!(wave.is_empty());
}

#[test]
fn an_arpeggio_on_the_tracks_changes_nothing_of_the_export() {
    // Only what the keyboards play goes through an arpeggio; the notes of the song do not.
    let place = Place::new("export-arp");
    let (doc, parts) = song();
    let arp = meloet_model::Arp {
        on: true,
        hold: true,
        ..Default::default()
    };
    let mut with = doc.clone();
    for track in [parts.lead, parts.drums] {
        with = edit::set_arp(&with, track, arp).unwrap().doc;
    }
    place.files_of(&doc);
    let span = (0, 2 * BAR);
    let plain = place.start(place.setup(place.job(1, &doc, span, ExportTail::Off)));
    plain.rendered();
    let on = place.start(place.setup(place.job(2, &with, span, ExportTail::Off)));
    on.rendered();
    let (plain, on) = (plain.pass(0), on.pass(0));
    assert!(plain.iter().any(|s| s.abs() > 0.05), "the song sounds");
    assert!(plain == on, "the same samples");
}
