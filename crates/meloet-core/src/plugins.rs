//! Plugins: where they are looked for, what the scan found and what stopped it
//! (docs/architecture.md 20-4). For now only the places, as the screen lists them; the scan
//! itself comes with the second step of round 5.

use std::ffi::OsString;
use std::path::Path;

use meloet_protocol::{PluginPlaceView, PluginsView, ScanView};

/// `path` as the screen shows it: the home folder written as "~".
pub(crate) fn shown_path(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// The usual places of Linux and nothing else: no scan runs and nothing has stopped.
pub(crate) fn view(var: impl Fn(&str) -> Option<OsString>) -> PluginsView {
    let home = var("HOME")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from);
    PluginsView {
        scan: ScanView::Idle,
        stopped: Vec::new(),
        places: meloet_plugin::scan::default_places(&var)
            .iter()
            .map(|place| PluginPlaceView {
                path: place.display().to_string(),
                shown_path: shown_path(place, home.as_deref()),
                added: false,
            })
            .collect(),
        stopped_open: false,
        places_open: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_folder_is_shown_as_a_tilde() {
        let home = Path::new("/home/me");
        assert_eq!(shown_path(Path::new("/home/me/.lv2"), Some(home)), "~/.lv2");
        assert_eq!(shown_path(Path::new("/home/me"), Some(home)), "~");
        assert_eq!(
            shown_path(Path::new("/home/meme/.lv2"), Some(home)),
            "/home/meme/.lv2"
        );
        assert_eq!(
            shown_path(Path::new("/usr/lib/lv2"), Some(home)),
            "/usr/lib/lv2"
        );
        assert_eq!(
            shown_path(Path::new("/home/me/.lv2"), None),
            "/home/me/.lv2"
        );
    }

    #[test]
    fn the_usual_places_are_listed_and_cannot_be_removed() {
        let vars = |name: &str| (name == "HOME").then(|| OsString::from("/home/me"));
        let view = view(vars);
        let shown: Vec<&str> = view.places.iter().map(|p| p.shown_path.as_str()).collect();
        assert_eq!(
            shown,
            [
                "~/.lv2",
                "/usr/lib/lv2",
                "/usr/local/lib/lv2",
                "~/.vst3",
                "/usr/lib/vst3",
                "/usr/local/lib/vst3"
            ]
        );
        assert_eq!(view.places[0].path, "/home/me/.lv2");
        assert!(view.places.iter().all(|p| !p.added));
        assert_eq!(view.scan, ScanView::Idle);
        assert!(view.stopped.is_empty() && !view.stopped_open && !view.places_open);
    }
}
