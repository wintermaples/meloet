//! Surge XT's modulation sources as its patches number them (1.3.x: 41 ids, of which 0 is
//! "off" and is not listed here, so 40 sources;
//! docs/research/2026-11-23-plugin-presets-state.md Q4). The short names are Surge XT's own
//! (its OSC names); the long ones are ours.

pub struct Source {
    pub id: u32,
    pub short: &'static str,
    pub name: &'static str,
}

const fn s(id: u32, short: &'static str, name: &'static str) -> Source {
    Source { id, short, name }
}

impl Source {
    /// One value for the whole scene (not one for each voice): only these may move a
    /// parameter of the patch as a whole, or of a scene LFO.
    pub fn scene_level(&self) -> bool {
        matches!(self.id, 4..=14 | 23..=28 | 35..=40)
    }
}

pub const SOURCES: [Source; 40] = [
    s(1, "vel", "Velocity"),
    s(2, "keytrk", "Keytrack"),
    s(3, "pat", "Poly aftertouch"),
    s(4, "at", "Channel aftertouch"),
    s(5, "pb", "Pitch bend"),
    s(6, "mw", "Modulation wheel"),
    s(7, "macro_1", "Macro 1"),
    s(8, "macro_2", "Macro 2"),
    s(9, "macro_3", "Macro 3"),
    s(10, "macro_4", "Macro 4"),
    s(11, "macro_5", "Macro 5"),
    s(12, "macro_6", "Macro 6"),
    s(13, "macro_7", "Macro 7"),
    s(14, "macro_8", "Macro 8"),
    s(15, "aeg", "Amp EG"),
    s(16, "feg", "Filter EG"),
    s(17, "vlfo_1", "LFO 1"),
    s(18, "vlfo_2", "LFO 2"),
    s(19, "vlfo_3", "LFO 3"),
    s(20, "vlfo_4", "LFO 4"),
    s(21, "vlfo_5", "LFO 5"),
    s(22, "vlfo_6", "LFO 6"),
    s(23, "slfo_1", "Scene LFO 1"),
    s(24, "slfo_2", "Scene LFO 2"),
    s(25, "slfo_3", "Scene LFO 3"),
    s(26, "slfo_4", "Scene LFO 4"),
    s(27, "slfo_5", "Scene LFO 5"),
    s(28, "slfo_6", "Scene LFO 6"),
    s(29, "timbre", "Timbre (MPE)"),
    s(30, "rel_vel", "Release velocity"),
    s(31, "rand_bi", "Random bipolar"),
    s(32, "rand_uni", "Random unipolar"),
    s(33, "alt_bi", "Alternate bipolar"),
    s(34, "alt_uni", "Alternate unipolar"),
    s(35, "breath", "Breath (CC 2)"),
    s(36, "expr", "Expression (CC 11)"),
    s(37, "sus", "Sustain pedal (CC 64)"),
    s(38, "lowest_key", "Lowest key"),
    s(39, "highest_key", "Highest key"),
    s(40, "latest_key", "Latest key"),
];

pub fn by_id(id: u32) -> Option<&'static Source> {
    SOURCES.iter().find(|s| s.id == id)
}

/// A source by its number, its short name or its name (any case, spaces and `_` alike).
pub fn find(text: &str) -> Option<&'static Source> {
    let text = text.trim();
    if let Ok(id) = text.parse::<u32>() {
        return by_id(id);
    }
    let plain = |t: &str| t.to_lowercase().replace(['_', ' ', '-'], "");
    let wanted = plain(text);
    SOURCES
        .iter()
        .find(|s| plain(s.short) == wanted || plain(s.name) == wanted)
}

/// The voice LFO (0..=5) or scene LFO (6..=11) whose `a_lfoN_` parameters shape a source.
pub fn lfo_of(id: u32) -> Option<u32> {
    (17..=28).contains(&id).then(|| id - 17)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_found_by_any_of_their_names() {
        assert_eq!(find("17").unwrap().short, "vlfo_1");
        assert_eq!(find("vlfo_1").unwrap().id, 17);
        assert_eq!(find("LFO 1").unwrap().id, 17);
        assert_eq!(find("scene lfo 2").unwrap().id, 24);
        assert_eq!(find("Macro_3").unwrap().id, 9);
        assert_eq!(find("mw").unwrap().id, 6);
        assert!(find("0").is_none() && find("41").is_none() && find("lfo").is_none());
        assert_eq!(lfo_of(17), Some(0));
        assert_eq!(lfo_of(23), Some(6));
        assert_eq!(lfo_of(6), None);
        let level: Vec<u32> = SOURCES
            .iter()
            .filter(|s| s.scene_level())
            .map(|s| s.id)
            .collect();
        assert_eq!(level.len(), 23);
        assert!(
            !find("LFO 1").unwrap().scene_level() && find("Scene LFO 1").unwrap().scene_level()
        );
        for (i, s) in SOURCES.iter().enumerate() {
            assert_eq!(s.id as usize, i + 1);
        }
    }
}
