//! The PC's own windows to pick a song file or a folder, and its file manager.
//!
//! On Linux the pickers are GTK's `FileChooserNative`: GTK already comes with Tauri, so nothing
//! is added, and GTK picks between its own window and the desktop portal.

use std::path::PathBuf;

use tauri::WebviewWindow;

// The shell shows these windows, not the screen, so their titles cannot come from
// ui/src/text/ja.ts. The words are spec 6 「PC のファイルの窓の題」 (ui/tests/text.test.ts reads them here).
pub const OPEN_TITLE: &str = "ひらく";
pub const FOLDER_TITLE: &str = "場所をえらぶ";
// docs/history/design-notes.ja.md B-59 (the user's decision of 2026-11-18); not in spec 6 yet.
pub const SAVE_AS_TITLE: &str = "名前をつけて保存";
// Spec 3 「PC の保存の窓の題（94-79）」.
pub const EXPORT_TITLE: &str = "書き出す";
pub const EXPORT_FOLDER_TITLE: &str = "書き出すフォルダをえらぶ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    SongFile,
    Folder,
    /// The folder window again, for a place to look for plugins in (docs/history/design-notes.ja.md A-16: the
    /// same title).
    PluginPlace,
    /// The save window, with the song's name filled in.
    SaveAs {
        name: String,
    },
    /// The open window again, for an SF2 file to add to the shelf (spec 6 has one title for
    /// opening: 「ひらく」).
    Sf2,
    /// The open window again, for audio files to bring into the song or to take for one
    /// that is missing (the same title). `many`: more than one may be chosen.
    Audio {
        many: bool,
    },
    /// The save window again, for the file an export is written to. The file is written
    /// over, so the window asks.
    ExportFile {
        name: String,
    },
    /// The folder window again, for where the files of 「トラックごと」 go.
    ExportFolder,
    /// The open window again, for the picture of an export's 「曲の情報」 (round 12 F2).
    Picture,
}

/// The ending of the files the SF2 window offers.
const SF2_EXTENSION: &str = "sf2";
/// The endings of the pictures the window for 「絵」 offers (png or jpg; the file's bytes decide).
const PICTURE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// The patterns of a filter that takes files with these endings, in small and in capital
/// letters.
#[cfg(target_os = "linux")]
fn patterns(endings: &[&str]) -> Vec<String> {
    (endings.iter())
        .flat_map(|e| [format!("*.{e}"), format!("*.{}", e.to_uppercase())])
        .collect()
}

#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use gtk::prelude::*;
    use tauri::WebviewWindow;

    use super::{
        EXPORT_FOLDER_TITLE, EXPORT_TITLE, FOLDER_TITLE, OPEN_TITLE, PICTURE_EXTENSIONS, Pick,
        SAVE_AS_TITLE, SF2_EXTENSION, patterns,
    };

    thread_local! {
        // A native dialog is not a widget: nothing else keeps it alive until it answers.
        static SHOWN: RefCell<Option<gtk::FileChooserNative>> = const { RefCell::new(None) };
    }

    /// Main thread only. `answer` gets the paths chosen (one, but for `Pick::Audio` with
    /// `many`), or none when the window was put away.
    pub fn pick(
        window: &WebviewWindow,
        pick: Pick,
        folder: Option<PathBuf>,
        answer: impl FnOnce(Vec<PathBuf>) + 'static,
    ) {
        let (title, action) = match pick {
            Pick::SongFile | Pick::Sf2 | Pick::Audio { .. } | Pick::Picture => {
                (OPEN_TITLE, gtk::FileChooserAction::Open)
            }
            Pick::Folder | Pick::PluginPlace => {
                (FOLDER_TITLE, gtk::FileChooserAction::SelectFolder)
            }
            Pick::ExportFolder => (EXPORT_FOLDER_TITLE, gtk::FileChooserAction::SelectFolder),
            Pick::SaveAs { .. } => (SAVE_AS_TITLE, gtk::FileChooserAction::Save),
            Pick::ExportFile { .. } => (EXPORT_TITLE, gtk::FileChooserAction::Save),
        };
        let parent = window.gtk_window().ok();
        let dialog = gtk::FileChooserNative::new(Some(title), parent.as_ref(), action, None, None);
        dialog.set_modal(true);
        if let Some(folder) = folder {
            dialog.set_current_folder(folder);
        }
        if let Pick::SaveAs { name } = &pick {
            dialog.set_current_name(name);
            // Nothing is written over: the song gets a new folder, " 2" when the name is taken.
            dialog.set_do_overwrite_confirmation(false);
        }
        if let Pick::ExportFile { name } = &pick {
            dialog.set_current_name(name);
            dialog.set_do_overwrite_confirmation(true);
        }
        if pick == Pick::SongFile {
            let filter = gtk::FileFilter::new();
            let pattern = format!("*.{}", meloet_core::songfile::EXTENSION);
            filter.add_pattern(&pattern);
            filter.set_name(Some(&pattern));
            dialog.add_filter(filter);
        }
        if pick == Pick::Sf2 {
            // Only SF2 files can be chosen, whatever case their ending is written in.
            let filter = gtk::FileFilter::new();
            for pattern in patterns(&[SF2_EXTENSION]) {
                filter.add_pattern(&pattern);
            }
            filter.set_name(Some(&format!("*.{SF2_EXTENSION}")));
            dialog.add_filter(filter);
        }
        if let Pick::Audio { many } = pick {
            // The files that read as audio (spec 3 「読めるファイル」).
            let endings = meloet_protocol::AUDIO_EXTENSIONS;
            let filter = gtk::FileFilter::new();
            for pattern in patterns(&endings) {
                filter.add_pattern(&pattern);
            }
            filter.set_name(Some(&endings.map(|e| format!("*.{e}")).join(" ")));
            dialog.add_filter(filter);
            dialog.set_select_multiple(many);
        }
        if pick == Pick::Picture {
            let filter = gtk::FileFilter::new();
            for pattern in patterns(&PICTURE_EXTENSIONS) {
                filter.add_pattern(&pattern);
            }
            filter.set_name(Some(
                &PICTURE_EXTENSIONS.map(|e| format!("*.{e}")).join(" "),
            ));
            dialog.add_filter(filter);
        }
        let answer = RefCell::new(Some(answer));
        dialog.connect_response(move |dialog, response| {
            let paths = match response {
                gtk::ResponseType::Accept => dialog.filenames(),
                _ => Vec::new(),
            };
            if let Some(answer) = answer.borrow_mut().take() {
                answer(paths);
            }
            SHOWN.with(|s| s.borrow_mut().take());
        });
        dialog.show();
        #[cfg(feature = "dev-hooks")]
        crate::devhooks::after_pc_window(title);
        SHOWN.with(|s| *s.borrow_mut() = Some(dialog));
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use std::path::PathBuf;

    use tauri::WebviewWindow;

    use super::Pick;

    /// Not made yet: the window counts as put away.
    pub fn pick(
        _: &WebviewWindow,
        _: Pick,
        _: Option<PathBuf>,
        answer: impl FnOnce(Vec<PathBuf>) + 'static,
    ) {
        answer(Vec::new());
    }
}

/// Shows `path` in the PC's file manager, picked out in its folder: the freedesktop
/// `org.freedesktop.FileManager1.ShowItems`, or else `xdg-open` on the folder. Does not wait.
#[cfg(target_os = "linux")]
pub fn show_file(path: PathBuf) {
    let shown = std::thread::Builder::new()
        .name("meloet-show-file".into())
        .spawn(move || {
            if let Err(e) = show_items(&path) {
                eprintln!("meloet: the file manager did not show the file ({e}); trying xdg-open");
                let folder = path.parent().unwrap_or(&path);
                let mut command = std::process::Command::new("xdg-open");
                meloet_plugin::launch::outside_appimage(&mut command);
                match command.arg(folder).status() {
                    Ok(s) if s.success() => {}
                    Ok(s) => eprintln!("meloet: xdg-open ended with {s}"),
                    Err(e) => eprintln!("meloet: cannot run xdg-open: {e}"),
                }
            }
        });
    if let Err(e) = shown {
        eprintln!("meloet: cannot start showing the file: {e}");
    }
}

#[cfg(target_os = "linux")]
fn show_items(path: &std::path::Path) -> Result<(), dbus::Error> {
    use gio::prelude::FileExt;

    let uri = gio::File::for_path(path).uri().to_string();
    let conn = dbus::blocking::Connection::new_session()?;
    let proxy = conn.with_proxy(
        "org.freedesktop.FileManager1",
        "/org/freedesktop/FileManager1",
        std::time::Duration::from_secs(10),
    );
    proxy.method_call(
        "org.freedesktop.FileManager1",
        "ShowItems",
        (vec![uri], String::new()),
    )
}

#[cfg(not(target_os = "linux"))]
pub fn show_file(_: PathBuf) {}

/// Opens `url` in the PC's browser with `xdg-open`. Only `https://`. Does not wait.
#[cfg(target_os = "linux")]
pub fn open_url(url: String) {
    if !url.starts_with("https://") {
        eprintln!("meloet: not opening {url:?}: not an https address");
        return;
    }
    xdg_open("meloet-open-url", url.into());
}

#[cfg(not(target_os = "linux"))]
pub fn open_url(_: String) {}

/// Opens `path` with the PC's own app for that kind of file, with `xdg-open`. Does not wait.
#[cfg(target_os = "linux")]
pub fn open_file(path: PathBuf) {
    xdg_open("meloet-open-file", path.into());
}

#[cfg(not(target_os = "linux"))]
pub fn open_file(_: PathBuf) {}

#[cfg(target_os = "linux")]
fn xdg_open(thread: &str, what: std::ffi::OsString) {
    let opened = std::thread::Builder::new()
        .name(thread.into())
        .spawn(move || {
            let mut command = std::process::Command::new("xdg-open");
            meloet_plugin::launch::outside_appimage(&mut command);
            match command.arg(&what).status() {
                Ok(s) if s.success() => {}
                Ok(s) => eprintln!("meloet: xdg-open {what:?} ended with {s}"),
                Err(e) => eprintln!("meloet: cannot run xdg-open: {e}"),
            }
        });
    if let Err(e) = opened {
        eprintln!("meloet: cannot start xdg-open: {e}");
    }
}

/// Main thread only. `answer` gets the paths chosen (more than one only for `Pick::Audio`
/// with `many`), or none when the window was put away.
pub fn pick(
    window: &WebviewWindow,
    pick: Pick,
    folder: Option<PathBuf>,
    answer: impl FnOnce(Vec<PathBuf>) + 'static,
) {
    imp::pick(window, pick, folder, answer);
}
