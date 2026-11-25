# fix-core-state — seen while fixing, not fixed

- crates/meloet-core/src/songfile.rs `folder_name` — a song's folder and file name come from the song's name with no byte limit; a song name over about 80 Japanese characters makes the first save and 「名前をつけて保存…」 fail (ENAMETOOLONG), the same fault as CORE-EXPORT-003. `export::job::cut_name` could be used there.
- crates/meloet-core/src/export.rs — the export still writes over a file there without a question when 「えらぶ…」 was never used (the default place and the song's name), when the format is changed after a place was picked (asked about `.wav`, writes over `.flac`), and when the file comes between picking and 「書き出す」. Asking at 「書き出す」 would need new wording.
- crates/meloet-core/src/state/songs.rs `close_requested` — a close while an open is read (`Waiting::Read`) or while the PC's open or folder window is up is still dropped without a word; only the save keeps it now (CORE-STATE-003 was about the save).
