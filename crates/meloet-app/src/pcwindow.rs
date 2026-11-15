//! The PC's own windows to pick a song file or a folder.
//!
//! On Linux they are GTK's `FileChooserNative`: GTK already comes with Tauri, so nothing is
//! added, and GTK picks between its own window and the desktop portal.

use std::path::PathBuf;

use tauri::WebviewWindow;

// The shell shows these windows, not the screen, so their titles cannot come from
// ui/src/text/ja.ts; they are the same words as there (the song menu row, `button.chooseLocation`).
pub const OPEN_TITLE: &str = "ひらく";
pub const FOLDER_TITLE: &str = "場所をえらぶ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    SongFile,
    Folder,
}

#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use gtk::prelude::*;
    use tauri::WebviewWindow;

    use super::{FOLDER_TITLE, OPEN_TITLE, Pick};

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
        };
        let parent = window.gtk_window().ok();
        let dialog = gtk::FileChooserNative::new(Some(title), parent.as_ref(), action, None, None);
        dialog.set_modal(true);
        if let Some(folder) = folder {
            dialog.set_current_folder(folder);
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

/// Main thread only. `answer` gets the path, or `None` when the window was put away.
pub fn pick(
    window: &WebviewWindow,
    pick: Pick,
    folder: Option<PathBuf>,
    answer: impl FnOnce(Option<PathBuf>) + 'static,
) {
    imp::pick(window, pick, folder, answer);
}
