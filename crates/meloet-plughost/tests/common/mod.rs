//! What the tests of the real `meloet-plughost` binary share: starting it, asking it, and
//! running blocks through the shared memory. Every child is killed when its test ends.
#![allow(dead_code)]

pub mod x11;

use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_plugin::ipc::futex::monotonic_ns;
use meloet_plugin::ipc::{EngineEnd, Geometry, Mapping, ToPlugin};
use meloet_plugin::wire::{
    FromHost, LoadedPlugin, Notice, OutputSettings, Reply, Request, ToHost, WIRE_VERSION,
    read_frame, write_frame,
};
use meloet_plugin::{PluginKey, launch};

/// The binary of this build with its fake plugins (feature `dev-hooks`). The binary Cargo
/// builds for these tests has none, so it is built again here, into the same place; that does
/// nothing when it is up to date.
pub fn exe() -> &'static str {
    static EXE: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    EXE.get_or_init(|| {
        let exe = env!("CARGO_BIN_EXE_meloet-plughost");
        let profile = Path::new(exe).parent().expect("the profile folder");
        let target = profile.parent().expect("the target folder");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut build = std::process::Command::new(cargo);
        build.args([
            "build",
            "--quiet",
            "-p",
            "meloet-plughost",
            "--features",
            "dev-hooks",
        ]);
        build.arg("--target-dir").arg(target);
        if profile.ends_with("release") {
            build.arg("--release");
        }
        let built = build.status().expect("cargo runs");
        assert!(built.success(), "meloet-plughost did not build");
        exe
    })
}
pub const FAKE: &str = "MELOET_PLUGHOST_FAKE";
pub const GEOMETRY: Geometry = Geometry {
    max_frames: 256,
    channels: 2,
    slots: 4,
    to_plugin_events: 64,
    from_plugin_events: 64,
};
/// For blocks that must be answered: long, so a busy PC does not fail the test.
pub const PATIENT_NS: u64 = 5_000_000_000;

pub struct Killed(pub Child);

impl Killed {
    pub fn wait_for_exit(&mut self, within: Duration) -> Option<ExitStatus> {
        let until = Instant::now() + within;
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return Some(status);
            }
            if Instant::now() > until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub struct Host {
    pub child: Killed,
    pub control: UnixStream,
    pub map: Arc<Mapping>,
    /// What the host said without being asked, in the order it came.
    pub notices: Vec<Notice>,
    next_id: u64,
}

impl Host {
    /// A host process with the fake plugins.
    pub fn start() -> Host {
        Host::start_with(GEOMETRY, &[(FAKE, "1")])
    }

    /// A host process with real plugins only. `env` is added to the test's own environment.
    pub fn start_with(geometry: Geometry, env: &[(&str, &str)]) -> Host {
        Host::start_as(geometry, env, false)
    }

    /// As `start_with`; `offline`: a host process for an export.
    pub fn start_as(geometry: Geometry, env: &[(&str, &str)], offline: bool) -> Host {
        let map = Arc::new(Mapping::create(geometry).unwrap());
        let (ours, theirs) = UnixStream::pair().unwrap();
        let mut command =
            launch::host_command(Path::new(exe()), theirs.as_fd(), map.fd(), None, offline)
                .unwrap();
        command.env_remove(FAKE);
        for (name, value) in env {
            command.env(name, value);
        }
        let child = command.spawn().unwrap();
        drop((command, theirs));
        ours.set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let mut host = Host {
            child: Killed(child),
            control: ours,
            map,
            notices: Vec::new(),
            next_id: 1,
        };
        assert_eq!(
            host.ask(Request::Hello {
                version: WIRE_VERSION
            }),
            Reply::Hello {
                version: WIRE_VERSION,
                offline,
            }
        );
        host
    }

    pub fn ask_with(&mut self, request: Request, blob: &[u8]) -> (Reply, Vec<u8>) {
        let id = self.next_id;
        self.next_id += 1;
        write_frame(&mut self.control, &ToHost { id, request }, blob).unwrap();
        loop {
            match read_frame::<FromHost>(&mut self.control).unwrap() {
                (FromHost::Reply { id: got, reply }, blob) => {
                    assert_eq!(got, id);
                    return (reply, blob);
                }
                (FromHost::Notice { notice }, _) => self.notices.push(notice),
            }
        }
    }

    pub fn ask(&mut self, request: Request) -> Reply {
        self.ask_with(request, &[]).0
    }

    /// Makes the engine's end of the slot, then loads the plugin into it.
    pub fn load_plugin(
        &mut self,
        slot: u32,
        key: &PluginKey,
        bundle: &str,
    ) -> Result<(EngineEnd, LoadedPlugin), String> {
        let geometry = self.map.geometry();
        let engine = EngineEnd::new(self.map.clone(), slot).unwrap();
        let reply = self.ask(Request::Load {
            slot,
            key: key.clone(),
            bundle: bundle.into(),
            sample_rate: 48_000,
            max_frames: geometry.max_frames,
            outputs: OutputSettings::default(),
        });
        match reply {
            Reply::Loaded { plugin } => Ok((engine, plugin)),
            Reply::Error { message } => Err(message),
            other => panic!("{other:?}"),
        }
    }

    /// Loads a fake plugin.
    pub fn load(&mut self, slot: u32, fake: &str) -> (EngineEnd, LoadedPlugin) {
        let key = PluginKey::lv2(&format!("urn:meloet:fake:{fake}")).unwrap();
        self.load_plugin(slot, &key, "/nowhere/fake.lv2").unwrap()
    }

    pub fn state(&mut self, slot: u32) -> Vec<u8> {
        match self.ask_with(Request::GetState { slot }, &[]) {
            (Reply::State, state) => state,
            other => panic!("{other:?}"),
        }
    }

    /// The value and the text of a parameter whose value the plugin tells.
    pub fn param(&mut self, slot: u32, id: u32) -> (f64, String) {
        match self.ask(Request::GetParam { slot, id }) {
            Reply::Param { now } => (now.value.unwrap(), now.text.unwrap_or_default()),
            other => panic!("{other:?}"),
        }
    }

    /// The value of a parameter, if the plugin tells it.
    pub fn value(&mut self, slot: u32, id: u32) -> Option<f64> {
        match self.ask(Request::GetParam { slot, id }) {
            Reply::Param { now } => now.value,
            other => panic!("{other:?}"),
        }
    }

    /// Reads what the host says by itself until `wanted` is among it, for `within` at most.
    /// Everything read is in `notices`.
    pub fn wait_for(
        &mut self,
        within: Duration,
        mut wanted: impl FnMut(&Notice) -> bool,
    ) -> Option<Notice> {
        let until = Instant::now() + within;
        let mut seen = 0;
        loop {
            if let Some(found) = self.notices[seen..].iter().find(|n| wanted(n)) {
                return Some(found.clone());
            }
            seen = self.notices.len();
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() || !launch::wait_readable(self.control.as_fd(), left) {
                return None;
            }
            match read_frame::<FromHost>(&mut self.control).unwrap() {
                (FromHost::Notice { notice }, _) => self.notices.push(notice),
                (FromHost::Reply { .. }, _) => panic!("an answer nobody asked for"),
            }
        }
    }
}

/// One answered block: `input` on both channels, `events` at their frames; the left and right
/// output.
pub fn block(engine: &mut EngineEnd, input: &[f32], events: &[ToPlugin]) -> (Vec<f32>, Vec<f32>) {
    assert!(engine.begin(input.len()));
    engine.write_input(0, input);
    engine.write_input(1, input);
    for event in events {
        assert!(engine.push_event(*event));
    }
    engine.submit();
    assert!(engine.wait(monotonic_ns() + PATIENT_NS));
    let (mut left, mut right) = (vec![9.0; input.len()], vec![9.0; input.len()]);
    engine.read_output(0, &mut left);
    engine.read_output(1, &mut right);
    (left, right)
}

pub fn note_on(key: u8) -> ToPlugin {
    ToPlugin::Midi {
        frame: 0,
        data: [0x90, key, 100],
    }
}

pub fn note_off(key: u8) -> ToPlugin {
    ToPlugin::Midi {
        frame: 0,
        data: [0x80, key, 0],
    }
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |m, x| m.max(x.abs()))
}
