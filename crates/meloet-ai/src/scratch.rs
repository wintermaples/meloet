//! The folder the tools that render write their audio into, for the AI app to read
//! (`render_audio`, `render_note`, `get_clip_audio`): `<temp>/meloet-ai-<uid>`, only for this
//! user. What is in it goes when the switch is turned off, when meloet ends, and after an
//! hour.

use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

/// Files older than this are taken away.
pub const KEEP: Duration = Duration::from_secs(60 * 60);

#[derive(Debug)]
pub struct Scratch {
    /// The folder the temp folder of the PC is (`std::env::temp_dir()`; tests give another).
    temp: PathBuf,
    uid: u32,
    made: AtomicU32,
}

impl Scratch {
    pub fn new(temp: PathBuf, uid: u32) -> Scratch {
        Scratch {
            temp,
            uid,
            made: AtomicU32::new(0),
        }
    }

    fn path(&self) -> PathBuf {
        self.temp.join(format!("meloet-ai-{}", self.uid))
    }

    /// The folder, made when it is not there. One that is there must be a folder of this
    /// user, not a symlink; it is narrowed to 0700.
    pub fn dir(&self) -> io::Result<PathBuf> {
        let dir = self.path();
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let meta = fs::symlink_metadata(&dir)?;
        if !meta.file_type().is_dir() || meta.uid() != self.uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not a folder of this user", dir.display()),
            ));
        }
        if meta.mode() & 0o777 != 0o700 {
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        Ok(dir)
    }

    /// A name in the folder that is not taken: the time, what it is, and `ending` (without a
    /// dot; empty for a folder).
    pub fn name(&self, dir: &Path, what: &str, ending: &str) -> PathBuf {
        let n = self.made.fetch_add(1, Ordering::Relaxed);
        let what: String = (what.chars())
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
                _ => '-',
            })
            .take(64)
            .collect();
        let stem = format!("{}-{what}-{n}", local_time());
        match ending {
            "" => dir.join(stem),
            ending => dir.join(format!("{stem}.{ending}")),
        }
    }

    /// Takes away what is older than `older` (everything with zero). Only what this user
    /// owns, in the folder itself; a folder of `render_audio` goes as a whole.
    pub fn clean(&self, older: Duration) {
        let dir = self.path();
        let Ok(meta) = fs::symlink_metadata(&dir) else {
            return;
        };
        if !meta.file_type().is_dir() || meta.uid() != self.uid {
            return;
        }
        let now = SystemTime::now();
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            let age = (meta.modified().ok())
                .and_then(|m| now.duration_since(m).ok())
                .unwrap_or(Duration::MAX);
            if meta.uid() != self.uid || (age < older && !older.is_zero()) {
                continue;
            }
            let path = entry.path();
            let _ = match meta.file_type().is_dir() {
                true => fs::remove_dir_all(&path),
                false => fs::remove_file(&path),
            };
        }
    }
}

/// "20261123-142501", in the PC's own time.
fn local_time() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as libc::time_t;
    // SAFETY: `tm` is filled by localtime_r, which reads `now` and writes only `tm`.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("k10ai2-scratch-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uid() -> u32 {
        // SAFETY: geteuid takes nothing and cannot fail.
        unsafe { libc::geteuid() }
    }

    #[test]
    fn the_folder_is_ours_and_closed() {
        let t = temp("ours");
        let s = Scratch::new(t.clone(), uid());
        let dir = s.dir().unwrap();
        assert_eq!(fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        s.dir().unwrap();
        assert_eq!(fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
        let a = s.name(&dir, "note C4/../x", "wav");
        let b = s.name(&dir, "note C4/../x", "wav");
        assert_ne!(a, b);
        assert_eq!(a.parent(), Some(dir.as_path()));
        assert!(!a.to_string_lossy().contains("/../"), "{a:?}");
        let _ = fs::remove_dir_all(t);
    }

    #[test]
    fn a_symlink_or_a_file_is_not_used() {
        let t = temp("link");
        let elsewhere = t.join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, t.join(format!("meloet-ai-{}", uid()))).unwrap();
        assert!(Scratch::new(t.clone(), uid()).dir().is_err());
        let f = temp("file");
        fs::write(f.join(format!("meloet-ai-{}", uid())), b"x").unwrap();
        assert!(Scratch::new(f.clone(), uid()).dir().is_err());
        let _ = fs::remove_dir_all(t);
        let _ = fs::remove_dir_all(f);
    }

    #[test]
    fn cleaning_takes_what_is_old() {
        let t = temp("clean");
        let s = Scratch::new(t.clone(), uid());
        let dir = s.dir().unwrap();
        let file = s.name(&dir, "a", "wav");
        fs::write(&file, b"x").unwrap();
        let folder = s.name(&dir, "b", "");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("1.wav"), b"x").unwrap();
        s.clean(KEEP);
        assert!(file.exists() && folder.exists());
        s.clean(Duration::ZERO);
        assert!(!file.exists() && !folder.exists());
        assert!(dir.exists());
        let _ = fs::remove_dir_all(t);
    }
}
