//! ALSA sequencer input with hot-plug. midir has no plug/unplug notifications, so this talks to
//! the sequencer directly and listens on the System:Announce port.
use std::collections::HashSet;
use std::ffi::CString;
use std::sync::mpsc::Sender;

use alsa::seq::{Addr, EvCtrl, EvNote, EventType, PortCap, PortInfo, PortSubscribe, PortType, Seq};

use crate::mono_ns;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MidiMsg {
    Note { on: bool, note: u8, vel: u8, t_ns: u64, kernel_ns: Option<u64> },
    Ctrl { param: u32, value: i32, t_ns: u64 },
    Bend { value: i32, t_ns: u64 },
    Device { connected: bool, name: String, client: i32, port: i32, t_ns: u64 },
}

/// What the audio thread gets: nothing else crosses into it.
#[derive(Clone, Copy)]
pub struct NoteEv {
    pub on: bool,
    pub note: u8,
    pub vel: u8,
    pub t_ns: u64,
}

fn wanted(info: &PortInfo, me: i32) -> bool {
    let caps = info.get_capability();
    let t = info.get_type();
    let c = info.get_client();
    let name = info.get_name().unwrap_or("");
    c != me && c != 0 && c != 14
        && caps.contains(PortCap::READ | PortCap::SUBS_READ)
        && (t.contains(PortType::HARDWARE) || name.starts_with("meloet-vkeys"))
}

fn port_name(seq: &Seq, a: Addr) -> String {
    let client = seq.get_any_client_info(a.client).ok().and_then(|c| c.get_name().ok().map(str::to_string)).unwrap_or_default();
    let port = seq.get_any_port_info(a).ok().and_then(|p| p.get_name().ok().map(str::to_string)).unwrap_or_default();
    format!("{client}:{port}")
}

pub fn spawn(mut to_audio: rtrb::Producer<NoteEv>, to_ui: Sender<MidiMsg>) {
    std::thread::Builder::new().name("midi-in".into()).spawn(move || {
        let seq = Seq::open(None, None, false).expect("open ALSA sequencer");
        seq.set_client_name(&CString::new("meloet-s4").unwrap()).unwrap();
        let me = seq.client_id().unwrap();
        let my_port = seq.create_simple_port(&CString::new("in").unwrap(), PortCap::WRITE | PortCap::SUBS_WRITE,
            PortType::MIDI_GENERIC | PortType::APPLICATION).unwrap();
        let dest = Addr { client: me, port: my_port };
        // A real-time queue so the kernel stamps each event when it arrives.
        let q = seq.alloc_named_queue(&CString::new("meloet").unwrap()).unwrap();
        seq.control_queue(q, EventType::Start, 0, None).unwrap();
        seq.drain_output().unwrap();
        let queue_zero = mono_ns();

        let subscribe = |src: Addr, stamped: bool| {
            let s = PortSubscribe::empty().unwrap();
            s.set_sender(src);
            s.set_dest(dest);
            if stamped { s.set_queue(q); s.set_time_update(true); s.set_time_real(true); }
            seq.subscribe_port(&s)
        };
        subscribe(Addr::system_announce(), false).expect("announce subscription");
        let mut connected: HashSet<(i32, i32)> = HashSet::new();
        for c in alsa::seq::ClientIter::new(&seq) {
            for p in alsa::seq::PortIter::new(&seq, c.get_client()) {
                if wanted(&p, me) && subscribe(p.addr(), true).is_ok() {
                    connected.insert((p.get_client(), p.get_port()));
                    let _ = to_ui.send(MidiMsg::Device { connected: true, name: port_name(&seq, p.addr()), client: p.get_client(), port: p.get_port(), t_ns: mono_ns() });
                }
            }
        }
        let mut input = seq.input();
        loop {
            let ev = match input.event_input() { Ok(e) => e, Err(_) => continue };
            let t = mono_ns();
            let kernel = ev.get_time().map(|d| queue_zero + d.as_nanos() as u64);
            match ev.get_type() {
                EventType::Noteon | EventType::Noteoff => {
                    let n: EvNote = ev.get_data().unwrap();
                    let on = ev.get_type() == EventType::Noteon && n.velocity > 0;
                    let _ = to_audio.push(NoteEv { on, note: n.note, vel: n.velocity, t_ns: t });
                    let _ = to_ui.send(MidiMsg::Note { on, note: n.note, vel: n.velocity, t_ns: t, kernel_ns: kernel });
                }
                EventType::Controller => {
                    let c: EvCtrl = ev.get_data().unwrap();
                    let _ = to_ui.send(MidiMsg::Ctrl { param: c.param, value: c.value, t_ns: t });
                }
                EventType::Pitchbend => {
                    let c: EvCtrl = ev.get_data().unwrap();
                    let _ = to_ui.send(MidiMsg::Bend { value: c.value, t_ns: t });
                }
                EventType::PortStart => {
                    let a: Addr = ev.get_data().unwrap();
                    drop(input);
                    if let Ok(p) = seq.get_any_port_info(a) {
                        if wanted(&p, me) && subscribe(a, true).is_ok() {
                            connected.insert((a.client, a.port));
                            let _ = to_ui.send(MidiMsg::Device { connected: true, name: port_name(&seq, a), client: a.client, port: a.port, t_ns: t });
                        }
                    }
                    input = seq.input();
                }
                EventType::PortExit => {
                    let a: Addr = ev.get_data().unwrap();
                    if connected.remove(&(a.client, a.port)) {
                        let _ = to_ui.send(MidiMsg::Device { connected: false, name: String::new(), client: a.client, port: a.port, t_ns: t });
                    }
                }
                _ => {}
            }
        }
    }).unwrap();
}

/// USB plug events from udev, to see how long the sequencer port takes to appear after the cable goes in.
pub fn spawn_udev(to_ui: Sender<(String, String, u64)>) {
    std::thread::Builder::new().name("udev".into()).spawn(move || {
        let Ok(sock) = udev::MonitorBuilder::new().and_then(|b| b.match_subsystem("sound")).and_then(|b| b.listen()) else { return };
        let fd = std::os::fd::AsRawFd::as_raw_fd(&sock);
        loop {
            let mut p = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
            unsafe { libc::poll(&mut p, 1, -1) };
            for ev in sock.iter() {
                let action = ev.action().map(|a| a.to_string_lossy().into_owned()).unwrap_or_default();
                let _ = to_ui.send((action, ev.sysname().to_string_lossy().into_owned(), mono_ns()));
            }
        }
    }).unwrap();
}
