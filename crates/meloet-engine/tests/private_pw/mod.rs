//! A PipeWire daemon of this test's own, with one null sink, in a fresh runtime directory, and
//! a WirePlumber that only links (no ALSA, Bluetooth or camera monitor, no D-Bus, no saved
//! state), so that streams reach the sink the way they do on a desktop.
//!
//! The daemon of the person using the PC is never touched: the children get their own
//! `PIPEWIRE_RUNTIME_DIR`, and this process is pointed at it before any PipeWire call.
#![allow(dead_code, clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const SINK: &str = "meloet-test-sink";

const CONFIG: &str = r#"
context.properties = {
    core.daemon = true
    core.name   = pipewire-0
    support.dbus = false
}
context.spa-libs = {
    audio.convert.* = audioconvert/libspa-audioconvert
    audio.adapt     = audioconvert/libspa-audioconvert
    support.*       = support/libspa-support
}
context.modules = [
    { name = libpipewire-module-protocol-native }
    { name = libpipewire-module-metadata }
    { name = libpipewire-module-spa-node-factory }
    { name = libpipewire-module-client-node }
    { name = libpipewire-module-access }
    { name = libpipewire-module-adapter }
    { name = libpipewire-module-link-factory }
]
context.objects = [
    { factory = metadata args = { metadata.name = default } }
    { factory = adapter
        args = {
            factory.name     = support.null-audio-sink
            node.name        = meloet-test-sink
            node.description = "meloet test sink"
            media.class      = Audio/Sink
            audio.position   = [ FL FR ]
            audio.rate       = 48000
            node.driver      = true
            priority.driver  = 30000
        }
    }
]
"#;

const WIREPLUMBER_PROFILE: &str = r#"
wireplumber.profiles = {
  meloet-test = {
    inherits = [ policy, mixin.systemwide-session, mixin.stateless ]
  }
}
"#;

pub struct PrivatePipeWire {
    dir: PathBuf,
    daemon: Option<Child>,
    wireplumber: Option<Child>,
}

impl PrivatePipeWire {
    /// Starts the daemon and points this process at it. `None` when there is no `pipewire`
    /// program. Call it first in the test, before any other thread exists.
    pub fn start() -> Option<PrivatePipeWire> {
        if Command::new("pipewire")
            .arg("--version")
            .stdout(Stdio::null())
            .status()
            .is_err()
        {
            return None;
        }
        // A socket path must fit in 108 bytes, so this stays short.
        let base =
            std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
        let dir = base.join(format!("meloet-pw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("runtime dir");
        std::fs::write(dir.join("test.conf"), CONFIG).expect("config");
        let profiles = dir.join("config/wireplumber/wireplumber.conf.d");
        std::fs::create_dir_all(&profiles).expect("WirePlumber config dir");
        std::fs::write(profiles.join("meloet-test.conf"), WIREPLUMBER_PROFILE)
            .expect("WirePlumber config");
        // SAFETY: the test calls this before it starts any thread, so nothing reads the
        // environment at the same time.
        unsafe {
            std::env::remove_var("PIPEWIRE_REMOTE");
            std::env::remove_var("PIPEWIRE_QUANTUM");
            std::env::set_var("PIPEWIRE_RUNTIME_DIR", &dir);
        }
        let mut pw = PrivatePipeWire {
            dir,
            daemon: None,
            wireplumber: None,
        };
        pw.restart();
        Some(pw)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Like a crash: SIGKILL, no goodbye to the clients.
    pub fn kill(&mut self) {
        if let Some(mut child) = self.daemon.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(mut child) = self.wireplumber.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(self.dir.join("pipewire-0"));
        let _ = std::fs::remove_file(self.dir.join("pipewire-0.lock"));
        let _ = std::fs::remove_file(self.dir.join("pipewire-0-manager"));
        let _ = std::fs::remove_file(self.dir.join("pipewire-0-manager.lock"));
    }

    pub fn restart(&mut self) {
        self.kill();
        self.daemon = Some(
            self.spawn(
                Command::new("pipewire")
                    .arg("-c")
                    .arg(self.dir.join("test.conf")),
            ),
        );
        let socket = self.dir.join("pipewire-0");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !socket.exists() {
            assert!(
                Instant::now() < deadline,
                "the private PipeWire made no socket"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        self.wireplumber = Some(
            self.spawn(
                Command::new("wireplumber")
                    .args(["-p", "meloet-test"])
                    .env("XDG_CONFIG_HOME", self.dir.join("config"))
                    .env("XDG_STATE_HOME", self.dir.join("state"))
                    .env("DBUS_SESSION_BUS_ADDRESS", "disabled:"),
            ),
        );
        self.wait_for_sink_ports();
    }

    fn spawn(&self, command: &mut Command) -> Child {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("daemon.log"))
            .expect("log file");
        command
            .env_remove("PIPEWIRE_REMOTE")
            .env_remove("PIPEWIRE_QUANTUM")
            .env("PIPEWIRE_RUNTIME_DIR", &self.dir)
            .env("XDG_RUNTIME_DIR", &self.dir)
            .stdin(Stdio::null())
            .stdout(log.try_clone().expect("log file"))
            .stderr(log)
            .spawn()
            .expect("the program starts")
    }

    /// WirePlumber gives the sink its ports; until then nothing can be linked to it.
    fn wait_for_sink_ports(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let ports = Command::new("pw-link")
                .arg("-i")
                .env_remove("PIPEWIRE_REMOTE")
                .env("PIPEWIRE_RUNTIME_DIR", &self.dir)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default();
            if ports.contains(&format!("{SINK}:playback_FL")) {
                return;
            }
            assert!(Instant::now() < deadline, "the sink got no ports");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for PrivatePipeWire {
    fn drop(&mut self) {
        self.kill();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
