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
// docs/design-notes.md B-59 (the user's decision of 2026-11-18); not in spec 6 yet.
pub const SAVE_AS_TITLE: &str = "名前をつけて保存";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    SongFile,
    Folder,
    /// The save window, with the song's name filled in.
    SaveAs {
        name: String,
    },
}

#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use gtk::prelude::*;
    use tauri::WebviewWindow;

    use super::{FOLDER_TITLE, OPEN_TITLE, Pick, SAVE_AS_TITLE};

    thread_local! {
        // A native dialog is not a widget: nothing else keeps it alive until it answers.
        static SHOWN: RefCell<Option<gtk::FileChooserNative>> = const { RefCell::new(None) };
    }

    /// Main thread only. `answer` gets the path, or `None` when the window was put away.
    pub fn pick(
        window: &WebviewWindow,
        pick: Pick,
        folder: Option<PathBuf>,
        answer: impl FnOnce(Option<PathBuf>) + 'static,
    ) {
        let (title, action) = match pick {
            Pick::SongFile => (OPEN_TITLE, gtk::FileChooserAction::Open),
            Pick::Folder => (FOLDER_TITLE, gtk::FileChooserAction::SelectFolder),
            Pick::SaveAs { .. } => (SAVE_AS_TITLE, gtk::FileChooserAction::Save),
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
        if pick == Pick::SongFile {
            let filter = gtk::FileFilter::new();
            let pattern = format!("*.{}", meloet_core::songfile::EXTENSION);
            filter.add_pattern(&pattern);
            filter.set_name(Some(&pattern));
            dialog.add_filter(filter);
        }
        let answer = RefCell::new(Some(answer));
        dialog.connect_response(move |dialog, response| {
            let path = (response == gtk::ResponseType::Accept)
                .then(|| dialog.filename())
                .flatten();
            if let Some(answer) = answer.borrow_mut().take() {
                answer(path);
            }
            SHOWN.with(|s| s.borrow_mut().take());
        });
        dialog.show();
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
        answer: impl FnOnce(Option<PathBuf>) + 'static,
    ) {
        answer(None);
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
                match std::process::Command::new("xdg-open").arg(folder).status() {
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

/// Main thread only. `answer` gets the path, or `None` when the window was put away.
pub fn pick(
    window: &WebviewWindow,
    pick: Pick,
    folder: Option<PathBuf>,
    answer: impl FnOnce(Option<PathBuf>) + 'static,
) {
    imp::pick(window, pick, folder, answer);
}
