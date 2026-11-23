//! 「音のクリップにする」 (round 12, A7) on the core thread: the job is a job of the export
//! whose file goes into the work place under a name of its own.

use meloet_protocol::{ExportFailure, ExportFormat, ExportLoudness};

use super::{Input, Worker};
use crate::export::tags::SongInfo;
use crate::export::{ExportAsk, ExportNews, ExportTarget};
use crate::state::BounceAsk;

impl Worker {
    /// [`Effect::Bounce`](crate::state::Effect::Bounce). Returns what the rules are to hear
    /// at once.
    pub(super) fn bounce_ask(&mut self, ask: BounceAsk) -> Vec<Input> {
        let id = ask.job.id;
        let Some(work) = self.audio_store.work_dir() else {
            let failed = ExportNews::Failed {
                job: id,
                reason: ExportFailure::CannotWrite,
            };
            return vec![Input::Export(failed)];
        };
        // One job of a second engine at a time.
        if self.ai_jobs() > 0 {
            return vec![Input::BounceBusy { job: id }];
        }
        let target = ExportTarget {
            path: work.join(format!(".bounce-{}-{id}.wav", std::process::id())),
            format: ExportFormat::Wav,
            sample_rate: ask.job.sample_rate,
            bits: ask.bits,
            loudness: ExportLoudness::AsIs,
            mp3_kbps: meloet_protocol::EXPORT_MP3_DEFAULT_KBPS,
            info: SongInfo::default(),
        };
        let mut heard: Vec<Input> = self.export_render(ask.job, None).into_iter().collect();
        heard.extend(self.export_ask(ExportAsk::Finish { job: id, target }));
        heard
    }
}
