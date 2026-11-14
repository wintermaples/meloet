//! Band rules (design spec section 3). All sizes are px at 100 % zoom.
//! `SIZE_*` are the spec's `size.*` values (section 1).

pub const SIZE_TRANSPORT: f64 = 48.0;
pub const SIZE_BANDROW: f64 = 24.0;
pub const SIZE_STATUS: f64 = 24.0;
pub const SIZE_METER_FULL: f64 = 52.0;
pub const SIZE_METER_SLIM: f64 = 24.0;
pub const SIZE_OVERVIEW: f64 = 40.0;
pub const PIANO_ROLL_MIN: f64 = 268.0;
/// Tools 32 + strips 484.
pub const MIXER_MIN: f64 = 516.0;
pub const HISTORY: f64 = 248.0;
/// The top row every lower band has (tools, or the heading of the history). A band that is
/// squeezed keeps at least this.
pub const BAND_TOOLS: f64 = 32.0;
pub const SIZE_SHELF: f64 = 320.0;
pub const PARAMS_WIDTH: f64 = 320.0;
/// From this window height on, the input meter stays full and the mixer stays at its smallest.
pub const TALL_WINDOW: f64 = 960.0;
/// The smallest window, in px at 100 % zoom (a 1280×800 window at 150 %).
pub const MIN_WINDOW: WindowSize = WindowSize {
    width: 853.0,
    height: 533.0,
};
/// Bottom bands that may be open at once, besides the arrange view.
pub const MAX_BOTTOM_OPEN: usize = 2;

/// Ordered top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BottomBand {
    PianoRoll,
    Mixer,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidePanel {
    Shelf,
    Params,
}

impl SidePanel {
    pub fn width(self) -> f64 {
        match self {
            Self::Shelf => SIZE_SHELF,
            Self::Params => PARAMS_WIDTH,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Band {
    Bottom(BottomBand),
    Side(SidePanel),
}

impl From<BottomBand> for Band {
    fn from(b: BottomBand) -> Self {
        Self::Bottom(b)
    }
}

impl From<SidePanel> for Band {
    fn from(p: SidePanel) -> Self {
        Self::Side(p)
    }
}

/// Window size in px at 100 % zoom (CSS px divided by the zoom).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

/// The smallest window in logical px at this zoom, rounded up to whole px.
pub fn min_window(scale_percent: u16) -> WindowSize {
    let at = |px: f64| (px * f64::from(scale_percent) / 100.0).ceil();
    WindowSize {
        width: at(MIN_WINDOW.width),
        height: at(MIN_WINDOW.height),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterSize {
    Full,
    Slim,
}

impl MeterSize {
    pub fn height(self) -> f64 {
        match self {
            Self::Full => SIZE_METER_FULL,
            Self::Slim => SIZE_METER_SLIM,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenBand {
    pub band: BottomBand,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub meter: MeterSize,
    pub arrange_height: f64,
    /// The arrange view is drawn as the overview: the piano roll is open, or the view is no
    /// taller than `size.overview`.
    pub arrange_overview: bool,
    /// Open bottom bands, top to bottom.
    pub bottom: Vec<OpenBand>,
    pub side_front: Option<SidePanel>,
    /// Open side panels, oldest first; the last one is in front.
    pub side_open: Vec<SidePanel>,
    /// Show "Esc ・ ひとつ戻る…" at the right end of the band row.
    pub show_back_hint: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bands {
    window: WindowSize,
    recording: bool,
    /// Oldest first.
    bottom_open: Vec<BottomBand>,
    /// Oldest first; the last one is in front.
    side_open: Vec<SidePanel>,
}

impl Bands {
    pub fn new(window: WindowSize) -> Self {
        Self {
            window,
            recording: false,
            bottom_open: Vec::new(),
            side_open: Vec::new(),
        }
    }

    pub fn window(&self) -> WindowSize {
        self.window
    }

    pub fn set_window(&mut self, window: WindowSize) {
        self.window = window;
        self.fit();
    }

    pub fn set_recording(&mut self, recording: bool) {
        self.recording = recording;
        self.fit();
    }

    pub fn toggle(&mut self, band: Band) {
        match band {
            Band::Bottom(b) => {
                if let Some(i) = self.bottom_open.iter().position(|&x| x == b) {
                    self.bottom_open.remove(i);
                } else {
                    self.bottom_open.push(b);
                    if self.bottom_open.len() > MAX_BOTTOM_OPEN {
                        self.bottom_open.remove(0);
                    }
                    self.fit();
                }
            }
            Band::Side(p) => {
                if self.side_open.last() == Some(&p) {
                    self.side_open.pop();
                } else {
                    self.side_open.retain(|&x| x != p);
                    self.side_open.push(p);
                }
            }
        }
    }

    /// A: fold every bottom band. Side panels stay.
    pub fn go_arrange(&mut self) {
        self.bottom_open.clear();
    }

    /// The Rust part of Esc. Returns false when nothing was open.
    pub fn back(&mut self) -> bool {
        self.side_open.pop().is_some() || self.bottom_open.pop().is_some()
    }

    pub fn layout(&self) -> Layout {
        let meter = self.meter();
        let piano_roll = self.bottom_open.contains(&BottomBand::PianoRoll);
        let spare = self.spare();
        let taker = self.spare_taker();
        let arrange_height = SIZE_OVERVIEW + if taker.is_none() { spare.max(0.0) } else { 0.0 };

        let mut open = self.bottom_open.clone();
        open.sort();
        let alone = open.len() == 1;
        let bottom = open
            .into_iter()
            .map(|band| {
                // A band that is open alone and still does not fit gets what is left.
                let extra = if taker == Some(band) || (alone && spare < 0.0) {
                    spare
                } else {
                    0.0
                };
                OpenBand {
                    band,
                    height: (smallest(band) + extra).max(BAND_TOOLS),
                }
            })
            .collect();

        Layout {
            meter,
            arrange_height,
            arrange_overview: piano_roll || arrange_height <= SIZE_OVERVIEW,
            bottom,
            side_front: self.side_open.last().copied(),
            side_open: self.side_open.clone(),
            show_back_hint: !self.bottom_open.is_empty(),
        }
    }

    fn meter(&self) -> MeterSize {
        if self.recording || self.window.height >= TALL_WINDOW || self.bottom_open.is_empty() {
            MeterSize::Full
        } else {
            MeterSize::Slim
        }
    }

    /// Height left for the arrange view and the bottom bands.
    fn rest(&self) -> f64 {
        self.window.height - SIZE_TRANSPORT - self.meter().height() - SIZE_BANDROW - SIZE_STATUS
    }

    /// What is left when the arrange view is at `size.overview` and every open band at its
    /// smallest. Negative: they do not fit.
    fn spare(&self) -> f64 {
        let smallest_sum: f64 = self.bottom_open.iter().map(|&b| smallest(b)).sum();
        self.rest() - SIZE_OVERVIEW - smallest_sum
    }

    /// The band that grows with the window; `None` is the arrange view.
    fn spare_taker(&self) -> Option<BottomBand> {
        let open = |b| self.bottom_open.contains(&b);
        if open(BottomBand::PianoRoll) {
            Some(BottomBand::PianoRoll)
        } else if open(BottomBand::Mixer) && self.window.height < TALL_WINDOW {
            Some(BottomBand::Mixer)
        } else {
            None
        }
    }

    fn fits(&self) -> bool {
        self.spare() >= 0.0
    }

    /// Fold the oldest bands until everything fits. The newest band is never folded.
    fn fit(&mut self) {
        while self.bottom_open.len() > 1 && !self.fits() {
            self.bottom_open.remove(0);
        }
    }
}

fn smallest(band: BottomBand) -> f64 {
    match band {
        BottomBand::PianoRoll => PIANO_ROLL_MIN,
        BottomBand::Mixer => MIXER_MIN,
        BottomBand::History => HISTORY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BottomBand::*;
    use SidePanel::*;

    fn bands(height: f64) -> Bands {
        Bands::new(WindowSize {
            width: 1280.0,
            height,
        })
    }

    fn open_bottom(l: &Layout) -> Vec<BottomBand> {
        l.bottom.iter().map(|b| b.band).collect()
    }

    fn height_of(l: &Layout, band: BottomBand) -> f64 {
        l.bottom.iter().find(|b| b.band == band).unwrap().height
    }

    #[test]
    fn smallest_window_follows_the_zoom() {
        let size = |scale| {
            let w = min_window(scale);
            (w.width, w.height)
        };
        assert_eq!(size(100), (853.0, 533.0));
        assert_eq!(size(125), (1067.0, 667.0));
        assert_eq!(size(150), (1280.0, 800.0));
    }

    #[test]
    fn nothing_open_800() {
        let l = bands(800.0).layout();
        assert_eq!(l.meter, MeterSize::Full);
        assert_eq!(l.meter.height(), 52.0);
        assert_eq!(l.arrange_height, 800.0 - 48.0 - 52.0 - 24.0 - 24.0);
        assert!(!l.arrange_overview);
        assert!(l.bottom.is_empty());
        assert!(!l.show_back_hint);
    }

    #[test]
    fn piano_roll_only_800() {
        let mut b = bands(800.0);
        b.toggle(PianoRoll.into());
        let l = b.layout();
        assert_eq!(l.meter, MeterSize::Slim);
        assert_eq!(l.arrange_height, 40.0);
        assert!(l.arrange_overview);
        assert_eq!(
            height_of(&l, PianoRoll),
            800.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
        );
        assert!(l.show_back_hint);
    }

    /// Spec "ピアノロールの縦の足し算": the note area is the piano roll minus
    /// tools 32, ruler 20, velocity 72 and CC row 24.
    #[test]
    fn piano_roll_note_area_matches_spec_table() {
        for (window, notes) in [
            (720.0, 412.0),
            (768.0, 460.0),
            (800.0, 492.0),
            (1080.0, 744.0),
        ] {
            let mut b = bands(window);
            b.toggle(PianoRoll.into());
            let l = b.layout();
            let area = height_of(&l, PianoRoll) - 32.0 - 20.0 - 72.0 - 24.0;
            assert_eq!(area, notes, "window {window}");
        }
    }

    #[test]
    fn mixer_then_piano_roll_800_folds_mixer() {
        let mut b = bands(800.0);
        b.toggle(Mixer.into());
        b.toggle(PianoRoll.into());
        let l = b.layout();
        assert_eq!(open_bottom(&l), [PianoRoll]);
        assert_eq!(
            height_of(&l, PianoRoll),
            800.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
        );
    }

    #[test]
    fn mixer_then_piano_roll_1080_keeps_both() {
        let mut b = bands(1080.0);
        b.toggle(Mixer.into());
        b.toggle(PianoRoll.into());
        let l = b.layout();
        assert_eq!(open_bottom(&l), [PianoRoll, Mixer]);
        assert_eq!(l.meter, MeterSize::Full);
        assert_eq!(l.arrange_height, 40.0);
        assert_eq!(height_of(&l, Mixer), 516.0);
        assert_eq!(
            height_of(&l, PianoRoll),
            1080.0 - 48.0 - 52.0 - 40.0 - 516.0 - 24.0 - 24.0
        );
    }

    #[test]
    fn third_band_folds_oldest() {
        let mut b = bands(1080.0);
        b.toggle(Mixer.into());
        b.toggle(History.into());
        assert_eq!(open_bottom(&b.layout()), [Mixer, History]);
        b.toggle(PianoRoll.into());
        let l = b.layout();
        assert_eq!(open_bottom(&l), [PianoRoll, History]);
        assert_eq!(height_of(&l, History), 248.0);
    }

    #[test]
    fn mixer_and_history_squeeze_arrange() {
        let mut b = bands(1080.0);
        b.toggle(Mixer.into());
        b.toggle(History.into());
        let l = b.layout();
        assert!(!l.arrange_overview);
        assert_eq!(
            l.arrange_height,
            1080.0 - 48.0 - 52.0 - 516.0 - 248.0 - 24.0 - 24.0
        );
    }

    #[test]
    fn mixer_and_history_800_fold_oldest() {
        let mut b = bands(800.0);
        b.toggle(Mixer.into());
        b.toggle(History.into());
        // 800 - 48 - 24 - 516 - 248 - 24 - 24 < 40
        let l = b.layout();
        assert_eq!(open_bottom(&l), [History]);
        assert_eq!(l.arrange_height, 800.0 - 48.0 - 24.0 - 248.0 - 24.0 - 24.0);
    }

    #[test]
    fn tall_window_keeps_full_meter() {
        for h in [960.0, 1080.0] {
            for band in [PianoRoll, Mixer, History] {
                let mut b = bands(h);
                b.toggle(band.into());
                assert_eq!(b.layout().meter, MeterSize::Full, "{h} {band:?}");
            }
        }
        let mut b = bands(959.0);
        b.toggle(Mixer.into());
        assert_eq!(b.layout().meter, MeterSize::Slim);
    }

    #[test]
    fn tall_window_compares_at_100_percent() {
        // 150 %: CSS 1440 -> 960 at 100 %.
        let mut b = Bands::new(WindowSize {
            width: 1920.0 / 1.5,
            height: 1440.0 / 1.5,
        });
        b.toggle(PianoRoll.into());
        assert_eq!(b.layout().meter, MeterSize::Full);
        b.set_window(WindowSize {
            width: 1920.0 / 1.5,
            height: 1080.0 / 1.5,
        });
        assert_eq!(b.layout().meter, MeterSize::Slim);
    }

    #[test]
    fn recording_keeps_full_meter() {
        let mut b = bands(800.0);
        b.toggle(History.into());
        b.set_recording(true);
        assert_eq!(b.layout().meter, MeterSize::Full);
        b.set_recording(false);
        assert_eq!(b.layout().meter, MeterSize::Slim);
    }

    #[test]
    fn mixer_stays_516_from_960_and_arrange_takes_the_rest() {
        for h in [960.0, 1080.0] {
            let mut b = bands(h);
            b.toggle(Mixer.into());
            let l = b.layout();
            assert_eq!(height_of(&l, Mixer), 516.0);
            assert_eq!(l.arrange_height, h - 48.0 - 52.0 - 516.0 - 24.0 - 24.0);
            assert!(!l.arrange_overview);
        }
    }

    #[test]
    fn mixer_below_960_takes_all_but_the_overview() {
        for h in [720.0, 800.0, 959.0] {
            let mut b = bands(h);
            b.toggle(Mixer.into());
            let l = b.layout();
            assert_eq!(l.meter, MeterSize::Slim);
            assert_eq!(l.arrange_height, 40.0);
            assert!(l.arrange_overview);
            assert_eq!(
                height_of(&l, Mixer),
                h - 48.0 - 24.0 - 40.0 - 24.0 - 24.0,
                "{h}"
            );
        }
    }

    #[test]
    fn mixer_and_history_below_960() {
        // 959 - 48 - 24 - 40 - 248 - 24 - 24 = 551 >= 516
        let mut b = bands(959.0);
        b.toggle(History.into());
        b.toggle(Mixer.into());
        let l = b.layout();
        assert_eq!(open_bottom(&l), [Mixer, History]);
        assert_eq!(l.arrange_height, 40.0);
        assert_eq!(height_of(&l, Mixer), 551.0);
        assert_eq!(height_of(&l, History), 248.0);

        // 900: 492 < 516, the older one folds.
        b.set_window(WindowSize {
            width: 1280.0,
            height: 900.0,
        });
        let l = b.layout();
        assert_eq!(open_bottom(&l), [Mixer]);
        assert_eq!(
            height_of(&l, Mixer),
            900.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
        );
    }

    #[test]
    fn mixer_then_piano_roll_just_below_960_keeps_mixer_at_516() {
        // 959 - 48 - 24 - 40 - 516 - 24 - 24 = 283 >= 268
        let mut b = bands(959.0);
        b.toggle(Mixer.into());
        b.toggle(PianoRoll.into());
        let l = b.layout();
        assert_eq!(open_bottom(&l), [PianoRoll, Mixer]);
        assert_eq!(l.arrange_height, 40.0);
        assert_eq!(height_of(&l, Mixer), 516.0);
        assert_eq!(height_of(&l, PianoRoll), 283.0);
    }

    #[test]
    fn piano_roll_then_mixer_800_folds_piano_roll() {
        let mut b = bands(800.0);
        b.toggle(PianoRoll.into());
        b.toggle(Mixer.into());
        let l = b.layout();
        assert_eq!(open_bottom(&l), [Mixer]);
        assert_eq!(height_of(&l, Mixer), 640.0);
        assert_eq!(l.arrange_height, 40.0);
    }

    /// Spec 3: one open band that does not fit is not folded; it gets what is left.
    #[test]
    fn one_band_in_a_low_window_gets_the_rest() {
        // The smallest window (853x533) at 100 %, and about what it is at 150 %.
        for h in [533.0, 356.0] {
            let left = h - 48.0 - 24.0 - 40.0 - 24.0 - 24.0;
            for band in [PianoRoll, Mixer] {
                let mut b = bands(h);
                b.toggle(band.into());
                let l = b.layout();
                assert_eq!(open_bottom(&l), [band], "{h}");
                assert_eq!(l.arrange_height, 40.0);
                assert_eq!(height_of(&l, band), left, "{h} {band:?}");
            }
        }
        // History fits at 533 and leaves the rest to the arrange view; at 356 it does not.
        let mut b = bands(533.0);
        b.toggle(History.into());
        let l = b.layout();
        assert_eq!(height_of(&l, History), 248.0);
        assert_eq!(l.arrange_height, 533.0 - 48.0 - 24.0 - 248.0 - 24.0 - 24.0);
        b.set_window(WindowSize {
            width: 853.0,
            height: 356.0,
        });
        let l = b.layout();
        assert_eq!(
            height_of(&l, History),
            356.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
        );
        assert_eq!(l.arrange_height, 40.0);
    }

    #[test]
    fn a_band_is_never_height_zero() {
        for h in [1.0, 100.0, 160.0, 200.0] {
            for band in [PianoRoll, Mixer, History] {
                let mut b = bands(h);
                b.toggle(band.into());
                let l = b.layout();
                assert_eq!(open_bottom(&l), [band]);
                assert!(height_of(&l, band) >= 32.0, "{h} {band:?}");
                assert_eq!(l.arrange_height, 40.0);
            }
        }
    }

    #[test]
    fn low_window_keeps_only_the_newest_band() {
        let mut b = bands(1080.0);
        b.toggle(History.into());
        b.toggle(Mixer.into());
        b.set_window(WindowSize {
            width: 853.0,
            height: 533.0,
        });
        let l = b.layout();
        assert_eq!(open_bottom(&l), [Mixer]);
        assert_eq!(
            height_of(&l, Mixer),
            533.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
        );
    }

    #[test]
    fn toggle_closes_open_bottom_band() {
        let mut b = bands(1080.0);
        b.toggle(Mixer.into());
        b.toggle(Mixer.into());
        let l = b.layout();
        assert!(l.bottom.is_empty());
        assert!(!l.show_back_hint);
    }

    #[test]
    fn side_panels_share_one_place() {
        let mut b = bands(800.0);
        b.toggle(Shelf.into());
        b.toggle(Params.into());
        let l = b.layout();
        assert_eq!(l.side_front, Some(Params));
        assert_eq!(l.side_open, [Shelf, Params]);

        // Pressing the one behind brings it to the front.
        b.toggle(Shelf.into());
        let l = b.layout();
        assert_eq!(l.side_front, Some(Shelf));
        assert_eq!(l.side_open, [Params, Shelf]);

        // Pressing the front one again closes it.
        b.toggle(Shelf.into());
        let l = b.layout();
        assert_eq!(l.side_front, Some(Params));
        assert_eq!(l.side_open, [Params]);
    }

    #[test]
    fn side_panels_take_no_height() {
        let mut b = bands(800.0);
        b.toggle(Shelf.into());
        b.toggle(Params.into());
        let l = b.layout();
        assert_eq!(l.meter, MeterSize::Full);
        assert_eq!(l.arrange_height, 800.0 - 48.0 - 52.0 - 24.0 - 24.0);
        assert!(!l.show_back_hint);
    }

    #[test]
    fn back_order_then_arrange_only() {
        let mut b = bands(1080.0);
        b.toggle(Mixer.into());
        b.toggle(PianoRoll.into());
        b.toggle(Shelf.into());
        b.toggle(Params.into());

        assert!(b.back());
        assert_eq!(b.layout().side_open, [Shelf]);
        assert!(b.back());
        assert!(b.layout().side_open.is_empty());
        assert!(b.back());
        assert_eq!(open_bottom(&b.layout()), [Mixer]);
        assert!(b.back());
        assert!(b.layout().bottom.is_empty());
        assert!(!b.back());
        assert_eq!(b.layout(), bands(1080.0).layout());
    }

    #[test]
    fn back_always_returns_to_arrange_only() {
        let all = [
            Band::from(PianoRoll),
            Mixer.into(),
            History.into(),
            Shelf.into(),
            Params.into(),
        ];
        // Every sequence of up to 4 toggles.
        for n in 0..=4u32 {
            for code in 0..all.len().pow(n) {
                let mut b = bands(800.0);
                let mut c = code;
                for _ in 0..n {
                    b.toggle(all[c % all.len()]);
                    c /= all.len();
                }
                let mut presses = 0;
                while b.back() {
                    presses += 1;
                    assert!(presses <= 4);
                }
                assert_eq!(b.layout(), bands(800.0).layout());
            }
        }
    }

    #[test]
    fn go_arrange_keeps_side_panels() {
        let mut b = bands(1080.0);
        b.toggle(Shelf.into());
        b.toggle(Mixer.into());
        b.toggle(History.into());
        b.go_arrange();
        let l = b.layout();
        assert!(l.bottom.is_empty());
        assert_eq!(l.meter, MeterSize::Full);
        assert_eq!(l.arrange_height, 1080.0 - 48.0 - 52.0 - 24.0 - 24.0);
        assert_eq!(l.side_front, Some(Shelf));
        assert!(!l.show_back_hint);
    }

    #[test]
    fn newest_band_is_never_folded() {
        let mut b = bands(800.0);
        b.toggle(PianoRoll.into());
        b.toggle(Mixer.into());
        assert_eq!(open_bottom(&b.layout()), [Mixer]);
    }

    #[test]
    fn shrinking_window_folds_oldest() {
        let mut b = bands(1080.0);
        b.toggle(Mixer.into());
        b.toggle(PianoRoll.into());
        b.set_window(WindowSize {
            width: 1280.0,
            height: 800.0,
        });
        assert_eq!(open_bottom(&b.layout()), [PianoRoll]);
    }
}
