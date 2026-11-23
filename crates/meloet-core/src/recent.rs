//! The song files opened or saved lately, the newest first: `recent-songs.json` in the state
//! folder (the AI link's `list_songs`; a menu may show them later). Not part of a song, and
//! no step.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "recent-songs.json";
const VERSION: u32 = 1;
/// Songs the list keeps.
pub const MOST: usize = 20;

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    songs: Vec<PathBuf>,
}

/// A missing, unreadable or broken file is an empty list. Files that are gone are left out.
pub fn load(dir: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(dir.join(FILE_NAME)) else {
        return Vec::new();
    };
    match serde_json::from_str::<OnDisk>(&text) {
        Ok(disk) if disk.version == VERSION => {
            let mut songs = Vec::new();
            for song in disk.songs {
                if song.is_absolute() && !songs.contains(&song) && song.is_file() {
                    songs.push(song);
                }
            }
            songs.truncate(MOST);
            songs
        }
        _ => Vec::new(),
    }
}

/// `songs` with `song` first. A path JSON cannot hold (not UTF-8) is not kept.
pub fn noted(songs: &[PathBuf], song: &Path) -> Vec<PathBuf> {
    if song.to_str().is_none() || !song.is_absolute() {
        return songs.to_vec();
    }
    let mut list = vec![song.to_path_buf()];
    list.extend(songs.iter().filter(|s| *s != song).cloned());
    list.truncate(MOST);
    list
}

/// As the settings are written: a whole temporary file, synced, renamed over the old one.
pub fn save(dir: &Path, songs: &[PathBuf]) -> std::io::Result<()> {
    let disk = OnDisk {
        version: VERSION,
        songs: songs.to_vec(),
    };
    let mut text = serde_json::to_string_pretty(&disk).map_err(std::io::Error::other)?;
    text.push('\n');
    std::fs::create_dir_all(dir)?;
    let temporary = dir.join(format!("{FILE_NAME}.tmp"));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, dir.join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::temp_dir;

    #[test]
    fn the_newest_first_each_once_and_at_most_twenty() {
        let dir = temp_dir("recent");
        let song = |n: usize| {
            let path = dir.join(format!("{n}.meloet"));
            std::fs::write(&path, "x").unwrap();
            path
        };
        let mut list = Vec::new();
        for n in 0..25 {
            list = noted(&list, &song(n));
        }
        assert_eq!(list.len(), MOST);
        assert_eq!(list[0], dir.join("24.meloet"));
        assert_eq!(list[MOST - 1], dir.join("5.meloet"));
        let list = noted(&list, &dir.join("10.meloet"));
        assert_eq!(list[0], dir.join("10.meloet"));
        assert_eq!(list.len(), MOST, "once");
        assert_eq!(noted(&list, Path::new("relative.meloet")), list);

        save(&dir, &list).unwrap();
        assert!(!dir.join("recent-songs.json.tmp").exists());
        assert_eq!(load(&dir), list);
        std::fs::remove_file(dir.join("24.meloet")).unwrap();
        let back = load(&dir);
        assert_eq!(back.len(), MOST - 1, "a file that is gone is left out");
        assert!(!back.contains(&dir.join("24.meloet")));
    }

    #[test]
    fn missing_or_broken_files_are_an_empty_list() {
        let dir = temp_dir("recent-broken");
        assert!(load(&dir).is_empty());
        for broken in ["", "{", r#"{"version":2,"songs":[]}"#, r#"{"version":1}"#] {
            std::fs::write(dir.join(FILE_NAME), broken).unwrap();
            assert!(load(&dir).is_empty(), "{broken:?}");
        }
    }
}
