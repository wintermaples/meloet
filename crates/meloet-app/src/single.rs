//! One meloet per user. The first one holds a lock in `$XDG_RUNTIME_DIR/meloet/` for as long as
//! it runs, and listens on a socket beside it. One started later hands it the song file it was
//! given (and the desktop's activation token, so that the window may come to the front) and
//! ends before it starts anything two would fight over: the sound output, the keyboards, the AI
//! socket, the recovery file.
//!
//! The lock (`flock`) decides who is first, so a socket left by a meloet that was killed is no
//! question: the kernel lets the lock go with the process. The folder is 0700 and the socket
//! 0600, as for the AI socket; who connects is checked again by uid.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const LOCK: &str = "app.lock";
pub const SOCKET: &str = "app.sock";

const HEAD: &[u8] = b"meloet-open 1\0";
const ANSWER: &[u8] = b"ok";
/// Longer than any path and token.
const LONGEST: u64 = 64 * 1024;
/// How long a later meloet tries to reach the first one, which may be between taking the lock
/// and binding the socket.
const REACH: Duration = Duration::from_secs(5);
/// How long either side waits for the other once connected.
const TALK: Duration = Duration::from_secs(2);

/// What a later meloet hands over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Handed {
    /// The song file it was started with, as a full path.
    pub song: Option<PathBuf>,
    /// The token the desktop gave for bringing a window to the front (`XDG_ACTIVATION_TOKEN`
    /// on Wayland, `DESKTOP_STARTUP_ID` on X11).
    pub token: Option<String>,
}

impl Handed {
    /// From the command line (`meloet /path/song.meloet`, the `%f` of the .desktop file) and
    /// the environment.
    pub fn from_start() -> Handed {
        let token = ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"]
            .iter()
            .find_map(|name| std::env::var(name).ok().filter(|t| !t.is_empty()));
        Handed {
            song: song_arg(std::env::args_os()).map(|p| full_path(&p)),
            token,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let mut bytes = HEAD.to_vec();
        bytes.extend_from_slice(self.token.as_deref().unwrap_or("").as_bytes());
        bytes.push(0);
        if let Some(song) = &self.song {
            bytes.extend_from_slice(song.as_os_str().as_bytes());
        }
        bytes
    }

    fn decode(bytes: &[u8]) -> Option<Handed> {
        let rest = bytes.strip_prefix(HEAD)?;
        let end = rest.iter().position(|&b| b == 0)?;
        let (token, song) = (&rest[..end], &rest[end + 1..]);
        if song.contains(&0) {
            return None;
        }
        let token = std::str::from_utf8(token).ok()?;
        Some(Handed {
            song: (!song.is_empty()).then(|| PathBuf::from(OsString::from_vec(song.to_vec()))),
            token: (!token.is_empty()).then(|| token.to_owned()),
        })
    }
}

/// The first argument that is not an option.
fn song_arg(args: impl Iterator<Item = OsString>) -> Option<PathBuf> {
    args.skip(1)
        .find(|a| !a.is_empty() && !a.as_bytes().starts_with(b"-"))
        .map(PathBuf::from)
}

/// The first meloet runs somewhere else: the path is made whole here.
fn full_path(path: &Path) -> PathBuf {
    let in_appimage = std::env::var_os("APPIMAGE").is_some();
    let path = from_start_folder(path, in_appimage, std::env::var_os("OWD"));
    fs::canonicalize(&path)
        .or_else(|_| std::path::absolute(&path))
        .unwrap_or(path)
}

/// An AppImage's `AppRun` moves into the AppImage before meloet starts; the folder it was
/// started in is `OWD`, set by the AppImage's runtime.
fn from_start_folder(path: &Path, in_appimage: bool, owd: Option<OsString>) -> PathBuf {
    match owd {
        Some(owd) if in_appimage && path.is_relative() && !owd.is_empty() => {
            Path::new(&owd).join(path)
        }
        _ => path.to_path_buf(),
    }
}

pub enum Start {
    /// This is the only meloet.
    First(First),
    /// Another meloet took what was handed over: this one ends.
    Handed,
    /// Another meloet holds the lock and did not answer.
    NoAnswer(io::Error),
    /// There is no place for the lock: this one runs without the check.
    Alone(String),
}

/// The lock, held until the process ends, and the socket.
pub struct First {
    _lock: File,
    pub listener: UnixListener,
    socket: PathBuf,
    id: (u64, u64),
}

impl First {
    /// Takes the socket's file away, unless another socket has its name now.
    pub fn remove_socket(&self) {
        let ours = fs::symlink_metadata(&self.socket).is_ok_and(|m| (m.dev(), m.ino()) == self.id);
        if ours {
            let _ = fs::remove_file(&self.socket);
        }
    }
}

/// Becomes the first meloet, or hands `handed` to the one there is.
pub fn start(runtime: Option<&Path>, uid: u32, handed: &Handed) -> Start {
    let Some(runtime) = runtime else {
        return Start::Alone("XDG_RUNTIME_DIR is not set".into());
    };
    let dir = runtime.join(meloet_ai::DIR);
    if let Err(e) = prepare_dir(&dir, uid) {
        return Start::Alone(format!("{}: {e}", dir.display()));
    }
    let lock = match take_lock(&dir.join(LOCK)) {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            return match hand_over(&dir.join(SOCKET), handed) {
                Ok(()) => Start::Handed,
                Err(e) => Start::NoAnswer(e),
            };
        }
        Err(e) => return Start::Alone(format!("{}: {e}", dir.join(LOCK).display())),
    };
    match bind(&dir.join(SOCKET)) {
        Ok((listener, id)) => Start::First(First {
            _lock: lock,
            listener,
            socket: dir.join(SOCKET),
            id,
        }),
        Err(e) => Start::Alone(format!("{}: {e}", dir.join(SOCKET).display())),
    }
}

/// The folder, made 0700 when it is not there, and narrowed to 0700 when it is ours.
fn prepare_dir(dir: &Path, uid: u32) -> io::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let meta = fs::symlink_metadata(dir)?;
    if !meta.is_dir() || meta.uid() != uid {
        return Err(io::Error::other("not a folder of this user"));
    }
    if meta.mode() & 0o777 != 0o700 {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// `None`: another process holds it.
fn take_lock(path: &Path) -> io::Result<Option<File>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    // SAFETY: the descriptor is open for as long as `file` lives.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(Some(file));
    }
    let e = io::Error::last_os_error();
    if e.kind() == ErrorKind::WouldBlock {
        Ok(None)
    } else {
        Err(e)
    }
}

/// With the lock held, a file in the socket's place is left over from a meloet that ended.
/// The umask is not touched (it is the whole process's): the socket is narrowed to 0600 right
/// after it is made, and until then the 0700 folder keeps everyone else out.
fn bind(path: &Path) -> io::Result<(UnixListener, (u64, u64))> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let listener = UnixListener::bind(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let meta = fs::symlink_metadata(path)?;
    Ok((listener, (meta.dev(), meta.ino())))
}

fn hand_over(path: &Path, handed: &Handed) -> io::Result<()> {
    let began = Instant::now();
    let mut stream = loop {
        match UnixStream::connect(path) {
            Ok(stream) => break stream,
            Err(e) if began.elapsed() >= REACH => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    stream.set_write_timeout(Some(TALK))?;
    stream.set_read_timeout(Some(TALK))?;
    stream.write_all(&handed.encode())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut answer = Vec::new();
    stream.take(ANSWER.len() as u64).read_to_end(&mut answer)?;
    if answer == ANSWER {
        Ok(())
    } else {
        Err(io::Error::new(ErrorKind::InvalidData, "no answer"))
    }
}

/// Listens on a thread of its own for as long as the process runs. A connection from another
/// uid, or one that does not say what it should in time, is dropped without a word.
pub fn listen(
    listener: UnixListener,
    expect_uid: u32,
    on_handed: impl Fn(Handed) + Send + 'static,
) -> io::Result<()> {
    std::thread::Builder::new()
        .name("meloet-single".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        if let Some(handed) = take(stream, expect_uid) {
                            on_handed(handed);
                        }
                    }
                    Err(e) => {
                        eprintln!("meloet: the socket for a second meloet: {e}");
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        })
        .map(|_| ())
}

fn take(mut stream: UnixStream, expect_uid: u32) -> Option<Handed> {
    if peer_uid(&stream).ok()? != expect_uid {
        return None;
    }
    stream.set_read_timeout(Some(TALK)).ok()?;
    stream.set_write_timeout(Some(TALK)).ok()?;
    let mut bytes = Vec::new();
    (&mut stream)
        .take(LONGEST + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > LONGEST {
        return None;
    }
    let handed = Handed::decode(&bytes)?;
    stream.write_all(ANSWER).ok()?;
    Some(handed)
}

fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    // SAFETY: `ucred` is plain data; getsockopt writes at most `len` bytes into it.
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let done = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &mut len,
        )
    };
    if done == 0 {
        Ok(cred.uid)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn my_uid() -> u32 {
    // SAFETY: geteuid takes nothing and cannot fail.
    unsafe { libc::geteuid() }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("k10deb-single-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn mode(path: &Path) -> u32 {
        fs::symlink_metadata(path).unwrap().mode() & 0o777
    }

    fn song(path: &str) -> Handed {
        Handed {
            song: Some(PathBuf::from(path)),
            token: Some("gnome-shell/meloet/123_TIME456".into()),
        }
    }

    #[test]
    fn in_an_appimage_a_short_path_is_from_where_it_was_started() {
        let owd = Some(OsString::from("/home/a/songs"));
        assert_eq!(
            from_start_folder(Path::new("x.meloet"), true, owd.clone()),
            PathBuf::from("/home/a/songs/x.meloet")
        );
        assert_eq!(
            from_start_folder(Path::new("/b/x.meloet"), true, owd.clone()),
            PathBuf::from("/b/x.meloet")
        );
        assert_eq!(
            from_start_folder(Path::new("x.meloet"), false, owd),
            PathBuf::from("x.meloet")
        );
        assert_eq!(
            from_start_folder(Path::new("x.meloet"), true, None),
            PathBuf::from("x.meloet")
        );
    }

    #[test]
    fn a_second_meloet_hands_its_song_to_the_first_and_ends() {
        let runtime = scratch("hand");
        let Start::First(first) = start(Some(&runtime), my_uid(), &Handed::default()) else {
            panic!("the first");
        };
        assert_eq!(mode(&runtime.join("meloet")), 0o700);
        assert_eq!(mode(&runtime.join("meloet").join(SOCKET)), 0o600);
        assert_eq!(mode(&runtime.join("meloet").join(LOCK)), 0o600);
        let (tx, rx) = mpsc::channel();
        listen(first.listener.try_clone().unwrap(), my_uid(), move |h| {
            let _ = tx.send(h);
        })
        .unwrap();

        let handed = song("/home/someone/ミュージック/曲/曲.meloet");
        assert!(matches!(
            start(Some(&runtime), my_uid(), &handed),
            Start::Handed
        ));
        assert_eq!(rx.recv_timeout(TALK).unwrap(), handed);
        // Started from the list of apps: only the window comes to the front.
        assert!(matches!(
            start(Some(&runtime), my_uid(), &Handed::default()),
            Start::Handed
        ));
        assert_eq!(rx.recv_timeout(TALK).unwrap(), Handed::default());

        first.remove_socket();
        assert!(!runtime.join("meloet").join(SOCKET).exists());
        drop(first);
        let _ = fs::remove_dir_all(runtime);
    }

    #[test]
    fn once_the_first_is_gone_the_next_one_is_first() {
        let runtime = scratch("gone");
        let Start::First(first) = start(Some(&runtime), my_uid(), &Handed::default()) else {
            panic!("the first");
        };
        // Killed: the socket's file stays, the lock goes with the process.
        drop(first);
        assert!(runtime.join("meloet").join(SOCKET).exists());
        let Start::First(again) = start(Some(&runtime), my_uid(), &song("/a.meloet")) else {
            panic!("first again");
        };
        assert_eq!(mode(&runtime.join("meloet").join(SOCKET)), 0o600);
        drop(again);
        let _ = fs::remove_dir_all(runtime);
    }

    #[test]
    fn someone_else_is_dropped_without_a_word() {
        let runtime = scratch("uid");
        let Start::First(first) = start(Some(&runtime), my_uid(), &Handed::default()) else {
            panic!("the first");
        };
        let (tx, rx) = mpsc::channel();
        listen(
            first.listener.try_clone().unwrap(),
            my_uid() + 1,
            move |h| {
                let _ = tx.send(h);
            },
        )
        .unwrap();
        assert!(matches!(
            start(Some(&runtime), my_uid(), &song("/a.meloet")),
            Start::NoAnswer(_)
        ));
        assert!(rx.try_recv().is_err());
        drop(first);
        let _ = fs::remove_dir_all(runtime);
    }

    #[test]
    fn a_folder_that_is_not_ours_or_no_place_runs_alone() {
        assert!(matches!(
            start(None, my_uid(), &Handed::default()),
            Start::Alone(_)
        ));
        let runtime = scratch("link");
        let elsewhere = scratch("link-elsewhere");
        std::os::unix::fs::symlink(&elsewhere, runtime.join("meloet")).unwrap();
        assert!(matches!(
            start(Some(&runtime), my_uid(), &Handed::default()),
            Start::Alone(_)
        ));
        assert!(!elsewhere.join(LOCK).exists());
        let _ = (fs::remove_dir_all(runtime), fs::remove_dir_all(elsewhere));
    }

    #[test]
    fn what_is_handed_reads_back_and_nothing_else_does() {
        for handed in [
            Handed::default(),
            song("/曲 1/曲.meloet"),
            Handed {
                song: Some(PathBuf::from("/x.meloet")),
                token: None,
            },
        ] {
            assert_eq!(Handed::decode(&handed.encode()), Some(handed));
        }
        assert_eq!(Handed::decode(b"hello"), None);
        assert_eq!(Handed::decode(b"meloet-open 1\0no end"), None);
        assert_eq!(Handed::decode(b"meloet-open 1\0\0/a\0b"), None);
    }

    #[test]
    fn the_song_is_the_first_argument_that_is_not_an_option() {
        let args = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(song_arg(args(&["meloet"]).into_iter()), None);
        assert_eq!(
            song_arg(args(&["meloet", "--x", "a.meloet"]).into_iter()),
            Some(PathBuf::from("a.meloet"))
        );
        assert!(full_path(Path::new("a.meloet")).is_absolute());
    }
}
