//! Band rules (design spec section 3). All sizes are px at 100 % zoom.
//! `SIZE_*` are the spec's `size.*` values (section 1).

/// The menu row above the transport.
pub const SIZE_MENUBAR: f64 = 24.0;
pub const SIZE_TRANSPORT: f64 = 48.0;
pub const SIZE_BANDROW: f64 = 24.0;
pub const SIZE_STATUS: f64 = 24.0;
pub const SIZE_METER_FULL: f64 = 52.0;
pub const SIZE_METER_SLIM: f64 = 24.0;
pub const SIZE_OVERVIEW: f64 = 40.0;
/// Ruler 72 + one track 64: from this height on the arrange view shows track rows.
pub const ARRANGE_ROWS: f64 = 136.0;
pub const SIZE_LANE: f64 = 48.0;
/// Tools 32 + chord row 24 + ruler 20 + 10 rows 120 + velocity 72 + CC row 24.
pub const PIANO_ROLL_MIN: f64 = 292.0;
/// 49 keys.
pub const PIANO_ROLL_MAX: f64 = 760.0;
/// Tools 32 + strips 484.
pub const MIXER_MIN: f64 = 516.0;
pub const MIXER_MAX: f64 = 660.0;
pub const HISTORY: f64 = 248.0;
/// The top row every lower band has (tools, or the heading of the history). A band that is
/// squeezed keeps at least this.
pub const BAND_TOOLS: f64 = 32.0;
pub const SIZE_SHELF: f64 = 320.0;
pub const PARAMS_WIDTH: f64 = 320.0;
/// The smallest window, in px at 100 % zoom (spec 1).
pub const MIN_WINDOW: WindowSize = WindowSize {
    width: 1024.0,
    height: 640.0,
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
    /// The arrange view is drawn as the overview: it is lower than [`ARRANGE_ROWS`].
    pub arrange_overview: bool,
    /// Open bottom bands, top to bottom.
    pub bottom: Vec<OpenBand>,
    pub side_front: Option<SidePanel>,
    /// Open side panels, oldest first; the last one is in front.
    pub side_open: Vec<SidePanel>,
    /// Show "Esc ・ ひとつ戻る…" at the right end of the band row.
    pub show_back_hint: bool,
}

/// A bottom band folded by the rules, not by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fold {
    pub band: BottomBand,
    pub why: FoldWhy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldWhy {
    /// A third band was opened: two at most.
    Third,
    /// The opened band and this one do not fit together.
    TooLow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bands {
    window: WindowSize,
    /// Oldest first.
    bottom_open: Vec<BottomBand>,
    /// Oldest first; the last one is in front.
    side_open: Vec<SidePanel>,
}

impl Bands {
    pub fn new(window: WindowSize) -> Self {
        Self {
            window,
            bottom_open: Vec::new(),
            side_open: Vec::new(),
        }
    }

    pub fn window(&self) -> WindowSize {
        self.window
    }

    /// Returns the band that was folded because the window got too low for both.
    pub fn set_window(&mut self, window: WindowSize) -> Option<BottomBand> {
        self.window = window;
        self.fit()
    }

    /// Returns the band folded to open this one, and why. When one toggle folds two bands (the
    /// oldest as the third, then one more that does not fit), the later fold is the one returned.
    pub fn toggle(&mut self, band: Band) -> Option<Fold> {
        match band {
            Band::Bottom(b) => {
                if let Some(i) = self.bottom_open.iter().position(|&x| x == b) {
                    self.bottom_open.remove(i);
                } else {
                    self.bottom_open.push(b);
                    let third = (self.bottom_open.len() > MAX_BOTTOM_OPEN).then(|| Fold {
                        band: self.bottom_open.remove(0),
                        why: FoldWhy::Third,
                    });
                    let low = self.fit().map(|band| Fold {
                        band,
                        why: FoldWhy::TooLow,
                    });
                    return low.or(third);
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
        None
    }

    /// A: fold every bottom band. Side panels stay.
    /// Opens the panel, or brings it to the front.
    pub fn show_side(&mut self, panel: SidePanel) {
        self.side_open.retain(|&x| x != panel);
        self.side_open.push(panel);
    }

    pub fn bottom_is_open(&self, band: BottomBand) -> bool {
        self.bottom_open.contains(&band)
    }

    pub fn side_is_open(&self, panel: SidePanel) -> bool {
        self.side_open.contains(&panel)
    }

    pub fn go_arrange(&mut self) {
        self.bottom_open.clear();
    }

    /// The Rust part of Esc. Returns false when nothing was open.
    pub fn back(&mut self) -> bool {
        self.side_open.pop().is_some() || self.bottom_open.pop().is_some()
    }

    pub fn layout(&self) -> Layout {
        self.layout_with(false)
    }

    /// `timeline_open`: the tempo band is open in the arrange view, a lane above the first
    /// track row. Spec 3 (94-92) keeps the arrange view's 40 first, so the track rows come only
    /// when one track still fits under it; lower, the arrange view is the overview.
    pub fn layout_with(&self, timeline_open: bool) -> Layout {
        let rows_from = ARRANGE_ROWS + if timeline_open { SIZE_LANE } else { 0.0 };
        let mut spare = self.spare();
        let mut bottom: Vec<OpenBand> = Vec::with_capacity(self.bottom_open.len());
        // Newest first: it grows first.
        for &band in self.bottom_open.iter().rev() {
            let grow = spare.clamp(0.0, largest(band) - smallest(band));
            spare -= grow;
            bottom.push(OpenBand {
                band,
                height: smallest(band) + grow,
            });
        }
        // A band that is open alone and still does not fit gets what is left.
        if let ([only], true) = (bottom.as_mut_slice(), spare < 0.0) {
            only.height = (only.height + spare).max(BAND_TOOLS);
        }
        bottom.sort_by_key(|open| open.band);
        let arrange_height = SIZE_OVERVIEW + spare.max(0.0);

        Layout {
            meter: self.meter(),
            arrange_height,
            arrange_overview: arrange_height < rows_from,
            bottom,
            side_front: self.side_open.last().copied(),
            side_open: self.side_open.clone(),
            show_back_hint: !self.bottom_open.is_empty(),
        }
    }

    fn meter(&self) -> MeterSize {
        if self.bottom_open.is_empty() {
            MeterSize::Full
        } else {
            MeterSize::Slim
        }
    }

    /// What is left when the arrange view is at `size.overview` and every open band at its
    /// smallest. Negative: they do not fit.
    fn spare(&self) -> f64 {
        let fixed =
            SIZE_MENUBAR + SIZE_TRANSPORT + self.meter().height() + SIZE_BANDROW + SIZE_STATUS;
        let smallest_sum: f64 = self.bottom_open.iter().map(|&b| smallest(b)).sum();
        self.window.height - fixed - SIZE_OVERVIEW - smallest_sum
    }

    /// Folds the older band when two do not fit. The newest band is never folded.
    fn fit(&mut self) -> Option<BottomBand> {
        (self.bottom_open.len() > 1 && self.spare() < 0.0).then(|| self.bottom_open.remove(0))
    }
}

fn smallest(band: BottomBand) -> f64 {
    match band {
        BottomBand::PianoRoll => PIANO_ROLL_MIN,
        BottomBand::Mixer => MIXER_MIN,
        BottomBand::History => HISTORY,
    }
}

fn largest(band: BottomBand) -> f64 {
    match band {
        BottomBand::PianoRoll => PIANO_ROLL_MAX,
        BottomBand::Mixer => MIXER_MAX,
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
        assert_eq!(size(100), (1024.0, 640.0));
        assert_eq!(size(125), (1280.0, 800.0));
    }

    #[test]
    fn nothing_open_800() {
        let l = bands(800.0).layout();
        assert_eq!(l.meter, MeterSize::Full);
        assert_eq!(l.meter.height(), 52.0);
        assert_eq!(l.arrange_height, 800.0 - 24.0 - 48.0 - 52.0 - 24.0 - 24.0);
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
            800.0 - 24.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
        );
        assert!(l.show_back_hint);
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
            800.0 - 24.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
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
    fn mixer_and_history_800_fold_oldest() {
        let mut b = bands(800.0);
        b.toggle(Mixer.into());
        b.toggle(History.into());
        // 800 - 24 - 48 - 24 - 516 - 248 - 24 - 24 < 40
        let l = b.layout();
        assert_eq!(open_bottom(&l), [History]);
        assert_eq!(
            l.arrange_height,
            800.0 - 24.0 - 48.0 - 24.0 - 248.0 - 24.0 - 24.0
        );
    }

    #[test]
    fn piano_roll_then_mixer_800_folds_piano_roll() {
        let mut b = bands(800.0);
        b.toggle(PianoRoll.into());
        b.toggle(Mixer.into());
        let l = b.layout();
        assert_eq!(open_bottom(&l), [Mixer]);
        assert_eq!(height_of(&l, Mixer), 616.0);
        assert_eq!(l.arrange_height, 40.0);
    }

    /// Spec 3: one open band that does not fit is not folded; it gets what is left.
    #[test]
    fn one_band_in_a_low_window_gets_the_rest() {
        // The mixer fits from 700.
        for h in [640.0, 699.0] {
            let mut b = bands(h);
            b.toggle(Mixer.into());
            let l = b.layout();
            assert_eq!(open_bottom(&l), [Mixer], "{h}");
            assert_eq!(l.arrange_height, 40.0);
            assert_eq!(
                height_of(&l, Mixer),
                h - 24.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0,
                "{h}"
            );
        }
    }

    #[test]
    fn low_window_keeps_only_the_newest_band() {
        let mut b = bands(1080.0);
        b.toggle(History.into());
        b.toggle(Mixer.into());
        b.set_window(MIN_WINDOW);
        let l = b.layout();
        assert_eq!(open_bottom(&l), [Mixer]);
        assert_eq!(
            height_of(&l, Mixer),
            640.0 - 24.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
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
        assert_eq!(l.arrange_height, 800.0 - 24.0 - 48.0 - 52.0 - 24.0 - 24.0);
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
        assert_eq!(l.arrange_height, 1080.0 - 24.0 - 48.0 - 52.0 - 24.0 - 24.0);
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

    /// Opens the bands in this order at this window height.
    fn opened(height: f64, order: &[BottomBand]) -> Bands {
        let mut b = bands(height);
        for &band in order {
            b.toggle(band.into());
        }
        b
    }

    /// Spec "ピアノロールの縦の足し算", with the menu row (B-59): the note area is the piano roll
    /// minus tools 32, chord row 24, ruler 20, velocity 72 and CC row 24.
    #[test]
    fn piano_roll_note_area_matches_spec_table() {
        for (window, notes, rows) in [
            (640.0, 284.0, 23.0),
            (720.0, 364.0, 30.0),
            (768.0, 412.0, 34.0),
            (800.0, 444.0, 37.0),
            (1080.0, 588.0, 49.0),
        ] {
            let l = opened(window, &[PianoRoll]).layout();
            assert_eq!(l.meter, MeterSize::Slim, "window {window}");
            let area = height_of(&l, PianoRoll) - 32.0 - 24.0 - 20.0 - 72.0 - 24.0;
            assert_eq!(area, notes, "window {window}");
            assert_eq!((area / 12.0).floor(), rows, "window {window}");
        }
    }

    /// Spec "縦の分けかたの例". `order` is oldest first; heights are arrange, then the bands
    /// top to bottom.
    #[test]
    fn split_examples_match_spec_table() {
        struct Row {
            order: &'static [BottomBand],
            fits: f64,
            arrange_grows: f64,
            arrange_rows: f64,
            at_800: Option<&'static [f64]>,
            at_1080: &'static [f64],
        }
        let rows = [
            Row {
                order: &[PianoRoll],
                fits: 476.0,
                arrange_grows: 944.0,
                arrange_rows: 1040.0,
                at_800: Some(&[40.0, 616.0]),
                at_1080: &[176.0, 760.0],
            },
            Row {
                order: &[Mixer],
                fits: 700.0,
                arrange_grows: 844.0,
                arrange_rows: 940.0,
                at_800: Some(&[40.0, 616.0]),
                at_1080: &[276.0, 660.0],
            },
            Row {
                order: &[History],
                fits: 432.0,
                arrange_grows: 432.0,
                arrange_rows: 528.0,
                at_800: Some(&[408.0, 248.0]),
                at_1080: &[688.0, 248.0],
            },
            Row {
                order: &[Mixer, PianoRoll],
                fits: 992.0,
                arrange_grows: 1604.0,
                arrange_rows: 1700.0,
                at_800: None,
                at_1080: &[40.0, 380.0, 516.0],
            },
            Row {
                order: &[History, Mixer],
                fits: 948.0,
                arrange_grows: 1092.0,
                arrange_rows: 1188.0,
                at_800: None,
                at_1080: &[40.0, 648.0, 248.0],
            },
        ];
        let heights = |l: &Layout| {
            let mut v = vec![l.arrange_height];
            v.extend(l.bottom.iter().map(|b| b.height));
            v
        };
        for row in rows {
            let name = format!("{:?}", row.order);
            let at = |h: f64| opened(h, row.order).layout();
            let smallest_all = |l: &Layout| l.bottom.iter().all(|b| b.height == smallest(b.band));
            let largest_all = |l: &Layout| l.bottom.iter().all(|b| b.height == largest(b.band));

            // "入る": every band has its smallest height, and one px lower it does not.
            let l = at(row.fits);
            assert_eq!(l.bottom.len(), row.order.len(), "{name} fits");
            assert_eq!(l.arrange_height, 40.0, "{name} fits");
            assert!(smallest_all(&l), "{name} fits");
            let below = at(row.fits - 1.0);
            if row.order.len() > 1 {
                assert_eq!(below.bottom.len(), 1, "{name} below fits");
            } else {
                assert!(below.bottom[0].height < smallest(row.order[0]), "{name}");
            }

            // "アレンジがのびはじめる"
            let l = at(row.arrange_grows);
            assert!(largest_all(&l), "{name} grows");
            assert_eq!(l.arrange_height, 40.0, "{name} grows");
            assert_eq!(at(row.arrange_grows + 1.0).arrange_height, 41.0, "{name}");
            if row.arrange_grows > row.fits {
                assert!(!largest_all(&at(row.arrange_grows - 1.0)), "{name}");
            }

            // "アレンジがトラックの行になる"
            let l = at(row.arrange_rows);
            assert_eq!(l.arrange_height, 136.0, "{name} rows");
            assert!(!l.arrange_overview, "{name} rows");
            assert!(at(row.arrange_rows - 1.0).arrange_overview, "{name} rows");

            match row.at_800 {
                Some(expected) => assert_eq!(heights(&at(800.0)), expected, "{name} 800"),
                None => assert_eq!(at(800.0).bottom.len(), 1, "{name} 800"),
            }
            let l = at(1080.0);
            assert_eq!(heights(&l), row.at_1080, "{name} 1080");
            assert_eq!(l.meter, MeterSize::Slim, "{name} 1080");
        }
    }

    const ORDERS: [&[BottomBand]; 9] = [
        &[PianoRoll],
        &[Mixer],
        &[History],
        &[PianoRoll, Mixer],
        &[Mixer, PianoRoll],
        &[PianoRoll, History],
        &[History, PianoRoll],
        &[Mixer, History],
        &[History, Mixer],
    ];

    /// Spec 3: a taller window never takes anything away.
    #[test]
    fn growing_window_keeps_the_bands_and_shrinks_nothing() {
        for order in ORDERS.iter().copied().chain([&[][..]]) {
            for start in [300.0, 448.0, 700.0, 944.0, 1200.0] {
                let mut b = opened(start, order);
                let mut last = b.layout();
                let mut h = start;
                while h < 2000.0 {
                    h += 1.0;
                    let folded = b.set_window(WindowSize {
                        width: 1280.0,
                        height: h,
                    });
                    assert_eq!(folded, None, "{order:?} {h}");
                    let l = b.layout();
                    assert_eq!(open_bottom(&l), open_bottom(&last), "{order:?} {h}");
                    assert_eq!(l.meter, last.meter, "{order:?} {h}");
                    assert!(l.arrange_height >= last.arrange_height, "{order:?} {h}");
                    for (now, before) in l.bottom.iter().zip(&last.bottom) {
                        assert!(now.height >= before.height, "{order:?} {h}");
                    }
                    last = l;
                }
            }
        }
    }

    /// The heights always add up to the window, from the height where the bands fit.
    #[test]
    fn heights_add_up_to_the_window() {
        for order in ORDERS {
            for h in (300..2000).map(f64::from) {
                let b = opened(h, order);
                let l = b.layout();
                let bands: f64 = l.bottom.iter().map(|b| b.height).sum();
                let sum = 24.0 + 48.0 + l.meter.height() + l.arrange_height + bands + 24.0 + 24.0;
                if b.spare() >= 0.0 {
                    assert_eq!(sum, h, "{order:?}");
                } else {
                    assert_eq!(l.bottom.len(), 1, "{order:?} {h}");
                }
            }
        }
    }

    #[test]
    fn meter_is_slim_exactly_while_a_bottom_band_is_open() {
        for h in [640.0, 800.0, 959.0, 960.0, 1080.0, 2000.0] {
            assert_eq!(bands(h).layout().meter, MeterSize::Full, "{h}");
            for band in [PianoRoll, Mixer, History] {
                let mut b = opened(h, &[band]);
                assert_eq!(b.layout().meter, MeterSize::Slim, "{h} {band:?}");
                b.toggle(band.into());
                assert_eq!(b.layout().meter, MeterSize::Full, "{h} {band:?}");
            }
            let mut b = bands(h);
            b.toggle(Shelf.into());
            b.toggle(Params.into());
            assert_eq!(b.layout().meter, MeterSize::Full, "{h}");
        }
    }

    #[test]
    fn overview_only_depends_on_the_arrange_height() {
        // Nothing open in a window 640 high: 640 - 24 - 48 - 52 - 24 - 24 = 468.
        assert!(!bands(640.0).layout().arrange_overview);
        // The piano roll open with tall arrange rows above it.
        let l = opened(1080.0, &[PianoRoll]).layout();
        assert_eq!(l.arrange_height, 176.0);
        assert!(!l.arrange_overview);
        // The piano roll open, 135 left.
        let l = opened(1039.0, &[PianoRoll]).layout();
        assert_eq!(l.arrange_height, 135.0);
        assert!(l.arrange_overview);
    }

    /// The older band grows only after the newest reached its largest height.
    #[test]
    fn newest_band_grows_first() {
        // 1300: 1300 - 144 - 40 - 292 - 516 = 308 to share.
        let l = opened(1300.0, &[Mixer, PianoRoll]).layout();
        assert_eq!(height_of(&l, PianoRoll), 292.0 + 308.0);
        assert_eq!(height_of(&l, Mixer), 516.0);
        let l = opened(1300.0, &[PianoRoll, Mixer]).layout();
        assert_eq!(height_of(&l, Mixer), 660.0);
        assert_eq!(height_of(&l, PianoRoll), 292.0 + 308.0 - 144.0);
        assert_eq!(l.arrange_height, 40.0);
        // Opening a band again makes it the newest.
        let mut b = opened(1300.0, &[PianoRoll, Mixer]);
        b.toggle(PianoRoll.into());
        b.toggle(PianoRoll.into());
        assert_eq!(height_of(&b.layout(), Mixer), 516.0);
    }

    #[test]
    fn folds_are_reported_with_the_reason() {
        let too_low = |band| Fold {
            band,
            why: FoldWhy::TooLow,
        };
        // Too low for both: the older one folds and is named.
        let mut b = opened(800.0, &[Mixer]);
        assert_eq!(b.toggle(PianoRoll.into()), Some(too_low(Mixer)));
        // A third band: the oldest folds, even where all three would fit.
        let mut b = opened(1300.0, &[Mixer, History]);
        assert_eq!(
            b.toggle(PianoRoll.into()),
            Some(Fold {
                band: Mixer,
                why: FoldWhy::Third,
            })
        );
        assert_eq!(open_bottom(&b.layout()), [PianoRoll, History]);
        // A third band that then does not fit beside the second: the later fold is named.
        let mut b = opened(960.0, &[History, Mixer]);
        assert_eq!(b.toggle(PianoRoll.into()), Some(too_low(Mixer)));
        assert_eq!(open_bottom(&b.layout()), [PianoRoll]);
        // Closing a band folds nothing.
        let mut b = opened(1080.0, &[Mixer, History]);
        assert_eq!(b.toggle(History.into()), None);
        // A lower window.
        let mut b = opened(1080.0, &[Mixer, PianoRoll]);
        let low = WindowSize {
            width: 1280.0,
            height: 967.0,
        };
        assert_eq!(b.set_window(low), Some(Mixer));
        assert_eq!(b.set_window(low), None);
    }

    #[test]
    fn folded_band_stays_folded_in_a_taller_window() {
        let mut b = opened(1080.0, &[Mixer, PianoRoll]);
        let size = |height| WindowSize {
            width: 1280.0,
            height,
        };
        b.set_window(size(800.0));
        b.set_window(size(2000.0));
        assert_eq!(open_bottom(&b.layout()), [PianoRoll]);
    }

    /// Spec 3 (94-92): the arrange view's 40 comes first. A window 960 high at 100 %
    /// with the piano roll, the mixer and the tempo band open:
    /// the piano roll is folded, and the arrange view is the overview, not track rows 36 high.
    #[test]
    fn the_arrange_views_40_comes_first() {
        let mut b = bands(960.0);
        assert_eq!(b.toggle(PianoRoll.into()), None);
        assert_eq!(
            b.toggle(Mixer.into()),
            Some(Fold {
                band: PianoRoll,
                why: FoldWhy::TooLow
            })
        );
        let l = b.layout_with(true);
        assert_eq!(open_bottom(&l), [Mixer]);
        let left = 960.0 - 24.0 - 48.0 - 24.0 - 660.0 - 24.0 - 24.0;
        assert_eq!(l.arrange_height, left);
        assert!(left >= ARRANGE_ROWS && left - 72.0 - SIZE_LANE < 64.0);
        assert!(
            l.arrange_overview,
            "one track does not fit under the tempo band"
        );
        assert!(
            !b.layout().arrange_overview,
            "the tempo band shut: one track fits"
        );
        // Tall enough for the ruler, the tempo band and one track.
        let mut b = bands(800.0);
        b.toggle(History.into());
        let l = b.layout_with(true);
        assert!(l.arrange_height >= ARRANGE_ROWS + SIZE_LANE);
        assert!(!l.arrange_overview);
        // Any window: the arrange view keeps 40, and track rows keep room for one track.
        let orders: [&[BottomBand]; 4] = [
            &[PianoRoll, History],
            &[Mixer, History],
            &[History, Mixer],
            &[PianoRoll],
        ];
        for h in (448..1400).step_by(7).map(f64::from) {
            for order in orders {
                for open in [false, true] {
                    let l = opened(h, order).layout_with(open);
                    assert!(l.arrange_height >= SIZE_OVERVIEW, "{h} {order:?}");
                    let ruler = 72.0 + if open { SIZE_LANE } else { 0.0 };
                    assert!(
                        l.arrange_overview || l.arrange_height - ruler >= 64.0,
                        "{h} {order:?} {open}"
                    );
                }
            }
        }
    }
}
