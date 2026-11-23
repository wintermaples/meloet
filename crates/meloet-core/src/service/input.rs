//! The PC's input on the core thread: the stream, its feed into the engine, the disk writer
//! and the clicks of the latency measurement (docs/architecture.md 22-5 to 22-7).

#[cfg(feature = "dev-hooks")]
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use meloet_engine::{
    AudioInput, EngineCommand, InputFeed, InputMeters, InputTarget, ProbeHeard, Recorded,
    RunningInput, probe_wave,
};
use meloet_model::{
    Doc, NewAudio, TempoMap, TrackId, VocalInput, VocalRecording, add_vocal_recordings, edit,
};
use meloet_protocol::{KeptVocals, RecordFormat};
use sha2::{Digest, Sha256};

use super::{Audio, Shared, Worker, lock};
use crate::peaks::{PeaksPlace, SharedPeaks};
use crate::play::VocalFileAsk;
use crate::state::{Delays, Input};
use crate::vocalrec::{self, DiskWriter, HeardLine, RecNote, RecordFile, RecordJob, Written};

/// How long the peaks of a recording that stopped wait for it to come into the song.
const PEAKS_WAIT: Duration = Duration::from_secs(10);

/// How long after a click is asked for its answer is looked for.
const CLICK_WAIT: Duration = Duration::from_millis(300);
/// How often the core thread looks while a click is out.
pub(super) const CLICK_TICK: Duration = Duration::from_millis(20);
/// The first click of a measurement waits until the input has sent frames for this long: an
/// input that has only just opened is often silent for its first moments.
const CLICK_SETTLE: Duration = Duration::from_secs(1);
/// It goes out after this even if the input sends nothing (its answer then says so).
const CLICK_SETTLE_MOST: Duration = Duration::from_secs(3);

/// What the telemetry thread reads the mic meters from.
pub(super) struct MicFacts {
    pub meters: Arc<InputMeters>,
    pub channels: u16,
    /// The metered vocal tracks, in the order of the song.
    pub tracks: Vec<(TrackId, VocalInput)>,
}

/// A recording of the input going on.
struct Job {
    x_us: f64,
    /// The disk writer was told where the files begin.
    from_said: bool,
    trouble_said: bool,
    /// (times the input skipped, blocks with no room) as last told to the rules.
    faults_said: (u32, u32),
}

/// The first click of a measurement, waiting for the input.
struct FirstClick {
    asked: Instant,
    /// What the input had sent when the click was asked for.
    frames: u64,
    /// When the input was first seen sending frames after that.
    flowing: Option<Instant>,
}

/// The peaks of a file that is recorded, or was until a moment ago.
struct RecPeaks {
    /// The file's number in the song's audio table.
    audio: u32,
    peaks: SharedPeaks,
    /// When the recording stopped; `None` while it goes on.
    stopped: Option<Instant>,
    /// The peaks file is being written.
    writing: Option<Receiver<std::io::Result<()>>>,
}

/// The note of a recording that a start brought back (docs/architecture.md 22-18 A1): it
/// stays on the disk until the recovery file holds the clips made from it.
#[derive(Default)]
pub(super) enum KeptNote {
    #[default]
    None,
    /// The recovery file has not been asked for yet.
    Waiting,
    /// Told when the recovery file is written.
    Writing(Receiver<()>),
}

pub(super) struct InputSide {
    stream: Option<RunningInput>,
    meters: Option<Arc<InputMeters>>,
    writer: DiskWriter,
    state_dir: Option<PathBuf>,
    job: Option<Job>,
    /// A click of the latency measurement is out since then.
    click: Option<Instant>,
    /// The next click asked for is the first of a measurement.
    click_first: bool,
    click_waits: Option<FirstClick>,
    delays_said: Option<Delays>,
    peaks: Vec<RecPeaks>,
    pub(super) kept_note: KeptNote,
    /// `MELOET_MEASURE_DIR`: what a recording skipped is written there too.
    #[cfg(feature = "dev-hooks")]
    pub(super) measure_dir: Option<PathBuf>,
}

impl InputSide {
    pub(super) fn new(state_dir: Option<PathBuf>) -> InputSide {
        InputSide {
            stream: None,
            meters: None,
            writer: DiskWriter::spawn(),
            state_dir,
            job: None,
            click: None,
            click_first: false,
            click_waits: None,
            delays_said: None,
            peaks: Vec::new(),
            kept_note: KeptNote::None,
            #[cfg(feature = "dev-hooks")]
            measure_dir: None,
        }
    }

    /// The peaks of `audio` while its file is recorded, and until its peaks file is there.
    pub(super) fn growing_peaks(&self, audio: u32) -> Option<PeaksPlace> {
        (self.peaks.iter().rev())
            .find(|p| p.audio == audio)
            .map(|p| PeaksPlace::Growing(p.peaks.clone()))
    }

    pub(super) fn click_out(&self) -> bool {
        self.click.is_some() || self.click_waits.is_some()
    }
}

impl Worker {
    /// `Effect::OpenInput`.
    pub(super) fn open_input(&mut self, node_name: String, sink_monitor: bool) -> Input {
        self.close_input();
        let Some(sample_rate) = self.audio.stream.as_ref().map(|s| s.sample_rate()) else {
            return Input::InputOpenFailed { node_name };
        };
        let target = InputTarget {
            node_name: node_name.clone(),
            sink_monitor,
        };
        let mut stream = match AudioInput::open(&target, sample_rate, self.audio.wish_frames) {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("meloet: cannot open the input {node_name}: {e}");
                return Input::InputOpenFailed { node_name };
            }
        };
        let meters = stream.meters().clone();
        if let Some(monitor) = stream.take_monitor() {
            let feed = Box::new(InputFeed::new(monitor, meters.clone()));
            self.audio
                .push(EngineCommand::SetInput { feed: Some(feed) });
        }
        if let Some(record) = stream.take_record() {
            self.input.writer.set_input(Some((record, meters.clone())));
        }
        let channels = stream.channels();
        *lock(&self.shared.mics) = Some(MicFacts {
            meters: meters.clone(),
            channels,
            tracks: self.model.metered_vocals(),
        });
        self.input.meters = Some(meters);
        self.input.stream = Some(stream);
        Input::InputOpened {
            node_name,
            channels,
            sample_rate,
        }
    }

    /// `Effect::CloseInput`, and before the output is opened anew or closed. A recording
    /// going on ends with what it has; the rules ask for it with `StopVocalRecording`.
    pub(super) fn close_input(&mut self) {
        let Some(stream) = self.input.stream.take() else {
            return;
        };
        *lock(&self.shared.mics) = None;
        self.input.writer.set_input(None);
        self.audio.unplug_input();
        stream.close();
        self.input.meters = None;
        self.input.click = None;
        self.input.delays_said = None;
    }

    /// `Effect::StartVocalRecording`.
    pub(super) fn start_vocals(
        &mut self,
        files: Vec<VocalFileAsk>,
        format: RecordFormat,
        x_us: i64,
    ) -> Input {
        let Some(dir) = self
            .input
            .state_dir
            .as_deref()
            .map(vocalrec::work_audio_dir)
        else {
            return Input::VocalsNotStarted;
        };
        // A name that is taken (a recording that was undone keeps its file) gets a number.
        let mut names: Vec<String> = Vec::new();
        for ask in &files {
            let stem = ask.name.strip_suffix(".wav").unwrap_or(&ask.name);
            let free = (1u32..)
                .map(|n| match n {
                    1 => ask.name.clone(),
                    n => format!("{stem} ({n}).wav"),
                })
                .find(|name| !names.contains(name) && !dir.join(name).exists());
            names.extend(free);
        }
        let job = RecordJob {
            files: (files.iter().zip(&names))
                .map(|(ask, name)| RecordFile {
                    path: dir.join(name),
                    channels: ask.channels.clone(),
                })
                .collect(),
            format,
            heard: Some(HeardLine {
                clock: self.audio.handle.clock().clone(),
                x_us: x_us as f64,
            }),
        };
        match self.input.writer.start(job) {
            Ok(peaks) => {
                self.input.job = Some(Job {
                    x_us: x_us as f64,
                    from_said: false,
                    trouble_said: false,
                    faults_said: (0, 0),
                });
                for (ask, peaks) in files.iter().zip(peaks) {
                    self.input.peaks.retain(|p| p.audio != ask.audio);
                    self.input.peaks.push(RecPeaks {
                        audio: ask.audio,
                        peaks,
                        stopped: None,
                        writing: None,
                    });
                }
                Input::VocalsStarted { names }
            }
            Err(e) => {
                eprintln!("meloet: cannot record into {}: {e}", dir.display());
                for name in &names {
                    let _ = std::fs::remove_file(dir.join(name));
                }
                Input::VocalsNotStarted
            }
        }
    }

    /// `Effect::StopVocalRecording`.
    pub(super) fn stop_vocals(&mut self) -> Input {
        self.input.job = None;
        let written = self.input.writer.stop().unwrap_or_default();
        if let Some(e) = &written.error {
            eprintln!("meloet: the recording was not written whole: {e}");
        }
        for peaks in self.input.peaks.iter_mut() {
            peaks.stopped.get_or_insert_with(Instant::now);
        }
        self.log_faults(&written);
        Input::VocalsWritten(Box::new(written))
    }

    /// Writes down what a recording skipped: on stderr, and in the folder of the
    /// measurements when there is one.
    fn log_faults(&self, written: &Written) {
        if written.gaps == 0 && written.dropped == 0 && written.cut == 0 {
            return;
        }
        let line = format!(
            "the recording has silence where the input did not come: {} frames in {} skips \
             of the input, {} frames in {} blocks the disk was too slow for; {} frames of the \
             input left out in {} skips of the output (of {} frames)",
            written.gaps,
            written.gap_count,
            written.dropped,
            written.dropped_blocks,
            written.cut,
            written.cut_count,
            written.frames
        );
        eprintln!("meloet: {line}");
        #[cfg(feature = "dev-hooks")]
        if let Some(dir) = &self.input.measure_dir {
            let file = std::fs::File::options()
                .create(true)
                .append(true)
                .open(dir.join("vocal-faults.log"));
            if let Ok(mut file) = file {
                let _ = writeln!(file, "{} {line}", crate::recovery::now_ms());
            }
        }
    }

    /// The peaks of the recordings that are in the song by now are written into their
    /// peaks files, and this waits for them (they are small).
    pub(super) fn keep_recorded_peaks(&mut self) {
        self.settle_peaks();
        self.input.peaks.retain_mut(|entry| {
            let Some(answer) = entry.writing.take() else {
                return true;
            };
            if let Ok(Err(e)) = answer.recv() {
                eprintln!("meloet: the peaks of a recording were not written: {e}");
            }
            false
        });
    }

    /// The peaks of the recordings that stopped: once a file is in the song (its peaks
    /// file has a name then) they are written there, and the screen is answered from the
    /// ones in memory until that file is there.
    fn settle_peaks(&mut self) {
        let mut at = 0;
        while at < self.input.peaks.len() {
            let entry = &mut self.input.peaks[at];
            let done = match (&entry.writing, entry.stopped) {
                (Some(answer), _) => match answer.try_recv() {
                    Ok(Err(e)) => {
                        eprintln!("meloet: the peaks of a recording were not written: {e}");
                        true
                    }
                    Ok(Ok(())) | Err(TryRecvError::Disconnected) => true,
                    Err(TryRecvError::Empty) => false,
                },
                (None, Some(stopped)) => match self.model.peaks_place(entry.audio) {
                    Some(PeaksPlace::File(path)) => {
                        let answer = self.input.writer.write_peaks(entry.peaks.clone(), path);
                        entry.writing = Some(answer);
                        false
                    }
                    // Not in the song (yet): a recording that left nothing never is.
                    _ => stopped.elapsed() > PEAKS_WAIT,
                },
                (None, None) => false,
            };
            if done {
                self.input.peaks.remove(at);
            } else {
                at += 1;
            }
        }
    }

    /// The note a start brought back goes once the recovery file holds its clips.
    fn settle_kept_note(&mut self) {
        if let KeptNote::Writing(done) = &self.input.kept_note
            && !matches!(done.try_recv(), Err(TryRecvError::Empty))
        {
            self.input.kept_note = KeptNote::None;
            if let Some(dir) = &self.input.state_dir {
                vocalrec::remove_note(dir);
            }
        }
    }

    /// `Effect::DropVocalFiles`.
    pub(super) fn drop_vocal_files(&self, names: &[String]) {
        let Some(dir) = self
            .input
            .state_dir
            .as_deref()
            .map(vocalrec::work_audio_dir)
        else {
            return;
        };
        for name in names {
            // Only a plain name: never a way out of the work folder.
            if !name.contains(['/', '\\']) && name != ".." {
                let _ = std::fs::remove_file(dir.join(name));
            }
        }
    }

    /// Where the engine says the song rolls or a lap begins: the frame of the files that was
    /// sung when that frame of the engine was heard (docs/architecture.md 22-6). At the roll
    /// the disk writer is told where the files begin, too. `None`: no files are written, or
    /// it is not a mark.
    pub(super) fn vocal_mark(&mut self, recorded: &Recorded) -> Option<f64> {
        let (frame, rolling) = match *recorded {
            Recorded::Rolling { frame, tick } => (frame, Some(tick)),
            Recorded::LapChanged { frame, .. } => (frame, None),
            _ => return None,
        };
        let job = self.input.job.as_mut()?;
        let handle = &self.audio.handle;
        let heard_us = handle.clock().time_of_frame(frame as f64);
        if let Some(tick) = rolling.filter(|_| !job.from_said) {
            job.from_said = true;
            // Half a beat before the song rolls, as the keys keep.
            let (map, _) = handle.tempo_map();
            let half_beat_us = 30e6 / map.tempo_at(tick).max(1.0);
            let from_us = heard_us.map_or(f64::NEG_INFINITY, |t| t - half_beat_us + job.x_us);
            self.input.writer.from(from_us);
        }
        self.input.writer.mark(heard_us? + job.x_us)
    }

    /// Looks at the input: the head of a recording, a disk that cannot go on, a stream that
    /// ended, the click that is out, and what the PC says of its delays.
    pub(super) fn poll_input(&mut self) {
        self.settle_peaks();
        self.settle_kept_note();
        let mut inputs = Vec::new();
        if let Some(job) = &mut self.input.job {
            let status: Written = self.input.writer.status();
            if !job.trouble_said && (status.full || status.error.is_some()) {
                job.trouble_said = true;
                inputs.push(Input::VocalsCannotGoOn);
            }
            let faults = (status.gap_count + status.cut_count, status.dropped_blocks);
            if faults != job.faults_said {
                job.faults_said = faults;
                inputs.push(Input::VocalFaults {
                    gaps: faults.0,
                    dropped: faults.1,
                });
            }
        }
        if self.input.meters.as_ref().is_some_and(|m| m.stream_lost()) {
            eprintln!("meloet: the input stream ended by itself");
            self.close_input();
            inputs.push(Input::InputLost);
        }
        if let Some(first) = &mut self.input.click_waits {
            let frames = self.input.meters.as_ref().map_or(0, |m| m.frames());
            if first.flowing.is_none() && frames > first.frames {
                first.flowing = Some(Instant::now());
            }
            if first.flowing.is_some_and(|at| at.elapsed() >= CLICK_SETTLE)
                || first.asked.elapsed() >= CLICK_SETTLE_MOST
            {
                self.input.click_waits = None;
                self.send_click();
            }
        }
        if self
            .input
            .click
            .is_some_and(|at| at.elapsed() >= CLICK_WAIT)
        {
            self.input.click = None;
            inputs.push(Input::LatencyAnswer {
                x_us: self.click_answer(),
            });
        }
        let delays = self.delays();
        if delays != self.input.delays_said {
            self.input.delays_said = delays;
            inputs.push(Input::AudioDelays(delays));
        }
        for input in inputs {
            self.handle(input);
        }
    }

    fn delays(&self) -> Option<Delays> {
        let meters = self.input.meters.as_ref()?;
        let rate = self.input.stream.as_ref()?.sample_rate().max(1);
        let block_us = u64::from(meters.last_frames()) * 1_000_000 / u64::from(rate);
        if block_us == 0 || !self.shared.stream.open.load(Relaxed) {
            return None;
        }
        let out_us = u64::from(self.audio.handle.meters().out_delay_us());
        // To a tenth of a millisecond: the PC's numbers move a little from block to block.
        let round = |us: u64| ((us + 50) / 100 * 100) as u32;
        // A sink's monitor says its frames a block early (`InputBlock::first_frame_us`).
        let late_us = if meters.sink_monitor() { block_us } else { 0 };
        let round_us = out_us + u64::from(meters.delay_us()) + late_us;
        Some(Delays {
            round_us: round(round_us),
            monitor_us: round(round_us + block_us),
        })
    }

    /// `Effect::LatencyClick`.
    pub(super) fn latency_click(&mut self) {
        if std::mem::take(&mut self.input.click_first) {
            self.input.click_waits = Some(FirstClick {
                asked: Instant::now(),
                frames: self.input.meters.as_ref().map_or(0, |m| m.frames()),
                flowing: None,
            });
        } else {
            self.send_click();
        }
    }

    fn send_click(&mut self) {
        // From nothing: what is kept then holds this click only.
        self.input.writer.capture(true);
        while self.audio.handle.pop_probe().is_some() {}
        self.audio.command(EngineCommand::Probe);
        self.input.click = Some(Instant::now());
    }

    pub(super) fn latency_listen(&mut self, on: bool) {
        self.input.writer.capture(on);
        self.input.click_first = on;
        if !on {
            self.input.click = None;
            self.input.click_waits = None;
        }
    }

    /// What the round trip of the click took more than the PC says, in microseconds: from
    /// the channel that heard it best.
    fn click_answer(&mut self) -> Option<f64> {
        let mut sent = None;
        while let Some(probe) = self.audio.handle.pop_probe() {
            sent = Some(probe);
        }
        let left_us = self
            .audio
            .handle
            .clock()
            .time_of_frame(sent?.frame as f64)?;
        let out_delay_us = f64::from(self.audio.handle.meters().out_delay_us());
        let kept = self.input.writer.captured();
        let wave = probe_wave(kept.sample_rate);
        (0..usize::from(kept.channels))
            .filter_map(|channel| {
                let heard = kept.channel(channel);
                let all = ProbeHeard {
                    blocks: &kept.blocks,
                    first_frame: kept.first_frame(),
                    heard: &heard,
                    sample_rate: kept.sample_rate,
                };
                all.way_round(&wave, left_us, out_delay_us)
            })
            .max_by(|a, b| a.1.clear.total_cmp(&b.1.clear))
            .map(|(x_us, _)| x_us)
    }

    /// The metered vocal tracks changed, perhaps.
    pub(super) fn sync_mics(&self) {
        if let Some(mics) = lock(&self.shared.mics).as_mut() {
            let tracks = self.model.metered_vocals();
            if mics.tracks != tracks {
                mics.tracks = tracks;
            }
        }
    }
}

impl Audio {
    /// The engine lets go of the input's feed.
    fn unplug_input(&mut self) {
        if self.stream.is_some() || self.engine.is_some() {
            self.push(EngineCommand::SetInput { feed: None });
            self.settle();
        }
        self.handle.drop_retired();
    }
}

/// The mic meters for one telemetry message: every channel is read once, and each metered
/// track gets what its channels read.
pub(super) fn mic_meters(shared: &Shared) -> (Vec<meloet_protocol::MicMeter>, bool) {
    let guard = lock(&shared.mics);
    let Some(mics) = guard.as_ref() else {
        return (Vec::new(), false);
    };
    let channels = usize::from(mics.channels).min(meloet_engine::MAX_INPUT_CHANNELS);
    let mut read = [(0.0f32, false); meloet_engine::MAX_INPUT_CHANNELS];
    for (channel, slot) in read.iter_mut().enumerate().take(channels) {
        *slot = mics.meters.take_peak(channel);
    }
    let mut clipped = false;
    let meters = (mics.tracks.iter())
        .filter(|(_, input)| input.fits(mics.channels))
        .map(|(track, input)| {
            let of = input.channels();
            let (left, right) = (
                read[usize::from(of[0])],
                read[usize::from(of[of.len() - 1])],
            );
            clipped |= left.1 || right.1;
            meloet_protocol::MicMeter {
                track: track.0,
                peak: [left.0, right.0],
                clipped: left.1 || right.1,
                stereo: of.len() == 2,
            }
        })
        .collect();
    (meters, clipped)
}

/// Puts the files a recording left when meloet ended into `doc`, each as a clip on its vocal
/// track, as far as it was written. Returns how many were kept and where the last ends.
pub(super) fn keep_vocals(
    state_dir: &Path,
    note: &RecNote,
    doc: &mut Doc,
    names: meloet_model::Language,
) -> Option<KeptVocals> {
    let dir = vocalrec::work_audio_dir(state_dir);
    let map = TempoMap::new(&doc.timeline);
    let mut until = 0u64;
    let mut recordings = Vec::new();
    for file in &note.files {
        // A start that ended before the recovery file was written reads the note again.
        if doc.audio.iter().any(|a| a.file == file.file) {
            continue;
        }
        let info = match vocalrec::mend_wav(&dir.join(&file.file)) {
            Ok(info) if info.frames > 0 => info,
            Ok(_) => {
                // Only the head: meloet ended while counting in. Nothing to keep.
                let _ = std::fs::remove_file(dir.join(&file.file));
                continue;
            }
            Err(e) => {
                eprintln!("meloet: {} was not kept: {e}", file.file);
                continue;
            }
        };
        let takes = vocalrec::takes_from(&note.marks, &map, info.sample_rate, info.frames, None);
        let Some(end) = takes.iter().map(|t| t.origin + t.length).max() else {
            continue;
        };
        // The track is gone from the song as it was kept (it was added just before the
        // recording): a vocal track is made for the file.
        let on_song = doc
            .track(TrackId(file.track))
            .is_some_and(|t| t.kind.is_vocal());
        let track = match on_song {
            true => TrackId(file.track),
            false => match edit::add_vocal_track(doc, None, names) {
                Ok((e, id)) => {
                    *doc = e.doc;
                    id
                }
                Err(_) => continue,
            },
        };
        let mut hash = Sha256::new();
        hash.update(b"meloet kept audio");
        hash.update(file.file.as_bytes());
        hash.update(info.frames.to_le_bytes());
        hash.update(crate::recovery::now_ms().to_le_bytes());
        let mut uid = [0u8; 16];
        uid.copy_from_slice(&hash.finalize()[..16]);
        until = until.max(end);
        recordings.push(VocalRecording {
            track,
            audio: NewAudio {
                uid,
                file: file.file.clone(),
                frames: info.frames,
                sample_rate: info.sample_rate,
                channels: info.channels,
                bits: Some(note.bits),
            },
            takes,
        });
    }
    let count = recordings.len() as u32;
    if count == 0 {
        return None;
    }
    match add_vocal_recordings(doc, recordings, None) {
        Ok((e, _)) => {
            *doc = e.doc;
            Some(KeptVocals { count, until })
        }
        Err(e) => {
            eprintln!("meloet: the recording was not kept: {e:?}");
            None
        }
    }
}
