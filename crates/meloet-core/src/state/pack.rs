//! The rules of the sound pack's fetch (spec 3 43-B, 44-A, 45-A; docs/architecture.md 23-9).
//! The fetching thread does the work; here is what the person answered, what the screen shows,
//! and what the shelf and the seats learn when a part comes.

use meloet_model::BuiltinInstrument;
use meloet_protocol::{
    Command, InstrumentKind, PackFailure, PackFetchView, SoundsView, StatusMessage,
};

use super::{Effect, Model};
use crate::instruments;
use crate::sounds::fetch::{Job, Numbers, Outlook, PackNews};

/// What the start found of the pack, for [`Model::with_pack`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackStart {
    /// The instruments of the catalog.
    pub instruments: Vec<BuiltinInstrument>,
    pub outlook: Outlook,
    /// The whole pack, in bytes.
    pub pack_bytes: u64,
    /// The place as shown.
    pub place: String,
}

/// What the fetching thread is asked; see [`Job`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackAsk {
    Job(Job),
    /// Stop the fetch of this round.
    Stop {
        round: u64,
    },
}

#[derive(Debug, Default)]
pub(super) struct PackSide {
    /// A fetching thread is there (not in tests, nor with a pack named by
    /// `MELOET_SOUNDS_DIR`).
    on: bool,
    ask: bool,
    /// 「取りにいく」 was answered and the fetch is not over: kept in the settings, so the
    /// next start goes on.
    pub(super) wanted: bool,
    /// Something of the catalog is not here.
    missing: bool,
    fetch: PackFetchView,
    numbers: Numbers,
    /// Goes up with every fetch, so that what an older one says late is not taken.
    round: u64,
    pack_bytes: u64,
    new_pack_bytes: Option<u64>,
    place: String,
    screen_up: bool,
}

impl PackSide {
    pub(super) fn new(wanted: bool) -> PackSide {
        PackSide {
            wanted,
            ..PackSide::default()
        }
    }
}

impl Model {
    /// The pack as the start found it, and whether the person said 「取りにいく」 before.
    pub fn with_pack(mut self, start: PackStart) -> Model {
        let wanted = self.pack.wanted;
        self.play.start_pack(start.instruments);
        self.pack = PackSide {
            on: true,
            ask: start.outlook.fresh && !wanted,
            wanted,
            missing: start.outlook.missing,
            numbers: start.outlook.numbers,
            pack_bytes: start.pack_bytes,
            place: start.place,
            ..PackSide::default()
        };
        self
    }

    pub(super) fn pack_command(&mut self, command: &Command, effects: &mut Vec<Effect>) {
        let failed = matches!(self.pack.fetch, PackFetchView::Failed { .. });
        let fetching = matches!(self.pack.fetch, PackFetchView::Fetching { .. });
        match command {
            Command::FetchPack => self.start_fetch(effects),
            Command::PackLater if self.pack.ask || failed => {
                self.pack.ask = false;
                self.pack.fetch = PackFetchView::Idle;
                self.pack.wanted = false;
                effects.push(Effect::SaveSettings(self.settings()));
            }
            Command::StopPackFetch if fetching => {
                let round = self.pack.round;
                effects.push(Effect::Pack(PackAsk::Stop { round }));
                // What the stopped fetch still says is not taken; what it put in place is.
                self.pack.round += 1;
                self.pack.fetch = PackFetchView::Idle;
                self.pack.wanted = false;
                effects.push(Effect::SaveSettings(self.settings()));
            }
            _ => {}
        }
    }

    fn start_fetch(&mut self, effects: &mut Vec<Effect>) {
        let pack = &mut self.pack;
        if !pack.on || !pack.missing || matches!(pack.fetch, PackFetchView::Fetching { .. }) {
            return;
        }
        pack.round += 1;
        pack.ask = false;
        pack.fetch = PackFetchView::Fetching {
            instrument: None,
            done_bytes: pack.numbers.done,
            total_bytes: pack.numbers.total,
            remaining_s: None,
        };
        effects.push(Effect::Pack(PackAsk::Job(Job::Fetch { round: pack.round })));
        if !pack.wanted {
            self.pack.wanted = true;
            effects.push(Effect::SaveSettings(self.settings()));
        }
    }

    /// The page is up, once: the look at the online catalog, and a fetch that was going on
    /// when meloet ended goes on.
    pub(super) fn pack_screen_up(&mut self, effects: &mut Vec<Effect>) {
        if !self.pack.on || self.pack.screen_up {
            return;
        }
        self.pack.screen_up = true;
        effects.push(Effect::Pack(PackAsk::Job(Job::Check)));
        if self.pack.wanted {
            self.start_fetch(effects);
        }
    }

    pub(super) fn pack_news(&mut self, news: PackNews, effects: &mut Vec<Effect>) {
        let fetching = matches!(self.pack.fetch, PackFetchView::Fetching { .. });
        match news {
            PackNews::Progress {
                round,
                instrument,
                numbers,
                remaining_s,
            } => {
                if round == self.pack.round && fetching {
                    self.pack.numbers = numbers;
                    self.pack.fetch = PackFetchView::Fetching {
                        instrument: instrument.map(instruments::kind_of),
                        done_bytes: numbers.done,
                        total_bytes: numbers.total,
                        remaining_s,
                    };
                }
            }
            PackNews::Installed {
                round, instrument, ..
            } => {
                let Some(instrument) = instrument else {
                    return;
                };
                let new = self.play.sound_installed(instrument, effects);
                if new && round == self.pack.round && fetching {
                    self.notice = Some(StatusMessage::SoundReady {
                        instrument: instruments::kind_of(instrument),
                    });
                }
            }
            PackNews::Done { round } => {
                if round != self.pack.round {
                    return;
                }
                let pack = &mut self.pack;
                pack.missing = false;
                pack.new_pack_bytes = None;
                pack.numbers.done = pack.numbers.total;
                pack.fetch = PackFetchView::Idle;
                if pack.wanted {
                    self.pack.wanted = false;
                    effects.push(Effect::SaveSettings(self.settings()));
                }
                self.notice = Some(StatusMessage::PackFetched { count: self.have() });
            }
            PackNews::Failed {
                round,
                failure,
                numbers,
            } => {
                if round != self.pack.round || !fetching {
                    return;
                }
                self.pack.numbers = numbers;
                self.pack.fetch = PackFetchView::Failed {
                    failure,
                    done_bytes: numbers.done,
                    total_bytes: numbers.total,
                };
                self.play.set_pack_broken(failure == PackFailure::Corrupt);
                self.notice = Some(StatusMessage::PackFailed {
                    failure,
                    done_bytes: numbers.done,
                    total_bytes: numbers.total,
                });
            }
            PackNews::NewPack {
                bytes,
                pack_bytes,
                instruments,
            } => {
                let pack = &mut self.pack;
                pack.missing = true;
                pack.new_pack_bytes = Some(bytes);
                pack.pack_bytes = pack_bytes;
                if !fetching {
                    pack.numbers = Numbers {
                        done: 0,
                        total: bytes,
                    };
                }
                self.play.set_fetchable(instruments);
                self.notice = Some(StatusMessage::NewPack { bytes });
            }
        }
    }

    fn have(&self) -> u32 {
        self.play.available_sounds().len() as u32
    }

    /// "音の素材がまだありません ・ いまはサイン波で鳴ります" while the band asks.
    pub(super) fn pack_asking(&self) -> bool {
        self.pack.ask
    }

    pub(super) fn sounds_view(&self) -> SoundsView {
        let total = InstrumentKind::ALL.len() as u32;
        let pack = &self.pack;
        if !pack.on {
            return SoundsView {
                have: total,
                total,
                ..SoundsView::default()
            };
        }
        SoundsView {
            ask: pack.ask,
            fetch: pack.fetch.clone(),
            have: self.have(),
            total,
            pack_bytes: pack.pack_bytes,
            new_pack_bytes: pack.new_pack_bytes,
            place: pack.place.clone(),
        }
    }
}
