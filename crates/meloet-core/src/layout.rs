//! Band rules (design spec section 3). All sizes are px at 100 % zoom.

pub const TRANSPORT: f64 = 48.0;
pub const BAND_ROW: f64 = 24.0;
pub const STATUS: f64 = 24.0;
pub const METER_FULL: f64 = 52.0;
pub const METER_SLIM: f64 = 24.0;
pub const OVERVIEW: f64 = 40.0;
pub const PIANO_ROLL_MIN: f64 = 268.0;
pub const MIXER: f64 = 432.0;
pub const HISTORY: f64 = 248.0;
pub const SHELF_WIDTH: f64 = 320.0;
pub const PARAMS_WIDTH: f64 = 320.0;
/// From this window height on, the input meter stays full.
pub const TALL_WINDOW: f64 = 960.0;
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
            Self::Shelf => SHELF_WIDTH,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterSize {
    Full,
    Slim,
}

impl MeterSize {
    pub fn height(self) -> f64 {
        match self {
            Self::Full => METER_FULL,
            Self::Slim => METER_SLIM,
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
    /// The arrange view is the 40 px overview (the piano roll is open).
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
        let rest = self.rest();
        let fixed = self.fixed_sum();
        let piano_roll = self.bottom_open.contains(&BottomBand::PianoRoll);
        let arrange_height = if piano_roll {
            OVERVIEW
        } else {
            (rest - fixed).max(0.0)
        };
        let piano_height = (rest - OVERVIEW - fixed).max(0.0);

        let mut open = self.bottom_open.clone();
        open.sort();
        let bottom = open
            .into_iter()
            .map(|band| OpenBand {
                band,
                height: fixed_height(band).unwrap_or(piano_height),
            })
            .collect();

        Layout {
            meter,
            arrange_height,
            arrange_overview: piano_roll,
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
        self.window.height - TRANSPORT - self.meter().height() - BAND_ROW - STATUS
    }

    /// Sum of the open bands with a fixed height (all but the piano roll).
    fn fixed_sum(&self) -> f64 {
        self.bottom_open
            .iter()
            .filter_map(|&b| fixed_height(b))
            .sum()
    }

    fn fits(&self) -> bool {
        let rest = self.rest();
        let fixed = self.fixed_sum();
        if self.bottom_open.contains(&BottomBand::PianoRoll) {
            rest - OVERVIEW - fixed >= PIANO_ROLL_MIN
        } else {
            rest - fixed >= OVERVIEW
        }
    }

    /// Fold the oldest bands until everything fits. The newest band is never folded.
    fn fit(&mut self) {
        while self.bottom_open.len() > 1 && !self.fits() {
            self.bottom_open.remove(0);
        }
    }
}

/// None for the piano roll, which takes what is left.
fn fixed_height(band: BottomBand) -> Option<f64> {
    match band {
        BottomBand::PianoRoll => None,
        BottomBand::Mixer => Some(MIXER),
        BottomBand::History => Some(HISTORY),
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
    /// The table says 772 for 1080, which counts the slim meter (24); with the
    /// 960 rule the meter stays 52 there, so the note area is 28 less.
    #[test]
    fn piano_roll_note_area_matches_spec_table() {
        for (window, notes) in [
            (720.0, 412.0),
            (768.0, 460.0),
            (800.0, 492.0),
            (1080.0, 772.0 - 28.0),
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
        assert_eq!(height_of(&l, Mixer), 432.0);
        assert_eq!(
            height_of(&l, PianoRoll),
            1080.0 - 48.0 - 52.0 - 40.0 - 432.0 - 24.0 - 24.0
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
            1080.0 - 48.0 - 52.0 - 432.0 - 248.0 - 24.0 - 24.0
        );
    }

    #[test]
    fn mixer_and_history_800_fold_oldest() {
        let mut b = bands(800.0);
        b.toggle(Mixer.into());
        b.toggle(History.into());
        // 800 - 48 - 24 - 432 - 248 - 24 - 24 = 0 < 40
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
    fn mixer_stays_432_on_wide_window() {
        let mut b = bands(1080.0);
        b.toggle(Mixer.into());
        let l = b.layout();
        assert_eq!(height_of(&l, Mixer), 432.0);
        assert_eq!(l.arrange_height, 1080.0 - 48.0 - 52.0 - 432.0 - 24.0 - 24.0);
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
