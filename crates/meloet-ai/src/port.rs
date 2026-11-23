//! The socket in `$XDG_RUNTIME_DIR/meloet/` (the PC's place for one user's running
//! programs). The folder is 0700 and the socket 0600; who connects is checked again by uid.

use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub const DIR: &str = "meloet";
pub const SOCKET: &str = "ai.sock";

#[derive(Debug)]
pub enum OpenError {
    /// No `XDG_RUNTIME_DIR`.
    NoPlace,
    /// Another meloet answers on the socket.
    InUse,
    /// The folder is not ours, or is not a folder.
    NotOurs(PathBuf),
    Io(io::Error),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::NoPlace => write!(f, "XDG_RUNTIME_DIR is not set"),
            OpenError::InUse => write!(f, "another meloet has the socket"),
            OpenError::NotOurs(p) => write!(f, "{} is not a folder of this user", p.display()),
            OpenError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<io::Error> for OpenError {
    fn from(e: io::Error) -> OpenError {
        OpenError::Io(e)
    }
}

/// The socket's file, and how to know it is still this one's.
#[derive(Debug)]
pub struct Bound {
    pub path: PathBuf,
    id: (u64, u64),
}

impl Bound {
    /// Takes the socket's file away, unless another socket has its name now.
    pub fn remove(&self) {
        let ours = fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.id);
        if ours {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// The folder, made 0700 when it is not there, and narrowed to 0700 when it is ours.
fn prepare_dir(dir: &Path, uid: u32) -> Result<(), OpenError> {
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    let meta = fs::symlink_metadata(dir)?;
    if !meta.is_dir() || meta.uid() != uid {
        return Err(OpenError::NotOurs(dir.to_path_buf()));
    }
    if meta.mode() & 0o777 != 0o700 {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// A socket left by a meloet that ended without taking it away is removed, but only when
/// nobody answers on it.
fn clear_stale(path: &Path) -> Result<(), OpenError> {
    match UnixStream::connect(path) {
        Ok(_) => Err(OpenError::InUse),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
            eprintln!("meloet: removing the AI socket left at {}", path.display());
            fs::remove_file(path)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Binds `<runtime>/meloet/ai.sock`. The umask is not touched (it is the whole process's):
/// the socket is narrowed to 0600 right after it is made, and until then the 0700 folder
/// keeps everyone else out.
pub fn open(runtime: Option<&Path>, uid: u32) -> Result<(UnixListener, Bound), OpenError> {
    let runtime = runtime.ok_or(OpenError::NoPlace)?;
    let dir = runtime.join(DIR);
    prepare_dir(&dir, uid)?;
    let path = dir.join(SOCKET);
    clear_stale(&path)?;
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let meta = fs::symlink_metadata(&path)?;
    let bound = Bound {
        path,
        id: (meta.dev(), meta.ino()),
    };
    Ok((listener, bound))
}

pub fn my_uid() -> u32 {
    // SAFETY: geteuid takes nothing and cannot fail.
    unsafe { libc::geteuid() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("k10ai-port-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn mode(path: &Path) -> u32 {
        fs::symlink_metadata(path).unwrap().mode() & 0o777
    }

    #[test]
    fn the_folder_and_the_socket_are_this_users_only() {
        let runtime = scratch("modes");
        let (_listener, bound) = open(Some(&runtime), my_uid()).unwrap();
        assert_eq!(mode(&runtime.join(DIR)), 0o700);
        assert_eq!(mode(&bound.path), 0o600);
        bound.remove();
        assert!(!bound.path.exists());

        // A folder that is there and too open is narrowed.
        fs::set_permissions(runtime.join(DIR), fs::Permissions::from_mode(0o755)).unwrap();
        let (_listener, bound) = open(Some(&runtime), my_uid()).unwrap();
        assert_eq!(mode(&runtime.join(DIR)), 0o700);
        bound.remove();
        let _ = fs::remove_dir_all(runtime);
    }

    #[test]
    fn no_place_no_socket() {
        assert!(matches!(open(None, my_uid()), Err(OpenError::NoPlace)));
    }

    #[test]
    fn a_folder_of_someone_else_is_not_used() {
        let runtime = scratch("other");
        assert!(matches!(
            open(Some(&runtime), my_uid() + 1),
            Err(OpenError::NotOurs(_))
        ));
        let _ = fs::remove_dir_all(runtime);
    }

    #[test]
    fn a_link_in_place_of_the_folder_is_not_followed() {
        let runtime = scratch("link");
        let elsewhere = scratch("link-target");
        std::os::unix::fs::symlink(&elsewhere, runtime.join(DIR)).unwrap();
        assert!(matches!(
            open(Some(&runtime), my_uid()),
            Err(OpenError::NotOurs(_))
        ));
        let _ = fs::remove_dir_all(runtime);
        let _ = fs::remove_dir_all(elsewhere);
    }

    #[test]
    fn a_left_socket_goes_and_a_live_one_stays() {
        let runtime = scratch("stale");
        let (listener, first) = open(Some(&runtime), my_uid()).unwrap();
        // A second meloet finds the first one answering.
        assert!(matches!(
            open(Some(&runtime), my_uid()),
            Err(OpenError::InUse)
        ));
        // The first one ends without taking its file away.
        drop(listener);
        assert!(first.path.exists());
        let (_listener, again) = open(Some(&runtime), my_uid()).unwrap();
        assert_eq!(mode(&again.path), 0o600);
        // The first one's `remove` does not take the new file.
        first.remove();
        assert!(again.path.exists());
        again.remove();
        let _ = fs::remove_dir_all(runtime);
    }
}
