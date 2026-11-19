//! The windows and the parameter lists of real plugins of this PC (Surge XT, Zam, LSP,
//! Dragonfly, abGate, swh). Ignored by default: they need an X display (on Wayland:
//! Xwayland) and put windows on the screen. A test whose plugin is not installed says so and
//! passes. The numbers they print are in docs/measurements/2026-11-19-round5-windows.md:
//!
//! ```text
//! cargo test -p meloet-plughost --test real_windows -- --ignored --nocapture --test-threads=1
//! ```

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use common::x11::X;
use common::{GEOMETRY, Host, note_off, note_on};
use meloet_plugin::ipc::futex::monotonic_ns;
use meloet_plugin::ipc::{EngineEnd, ToPlugin};
use meloet_plugin::wire::{EditorOpen, Notice, ParamInfo, ParamShape, Reply, Request, TextValue};
use meloet_plugin::{EditorKind, PluginKey};

type Plugin = (&'static str, &'static str, &'static str);

const SURGE_VST3: Plugin = (
    "Surge XT (VST3)",
    "vst3:ABCDEF019182FAEB566D624153675854",
    "/usr/lib/vst3/Surge XT.vst3",
);
const SURGE_LV2: Plugin = (
    "Surge XT (LV2)",
    "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
    "/usr/lib/lv2/Surge XT.lv2",
);
const ZAMCOMP_VST3: Plugin = (
    "ZamComp (VST3)",
    "vst3:2046504473616C63504D435A00000000",
    "/usr/lib/vst3/ZamComp.vst3",
);
const ZAMCOMP_LV2: Plugin = (
    "ZamComp (LV2)",
    "lv2:urn:zamaudio:ZamComp",
    "/usr/lib/lv2/ZamComp.lv2",
);
const LSP_COMP_VST3: Plugin = (
    "LSP Compressor Stereo (VST3)",
    "vst3:647370206B31732020202020756E7363",
    "/usr/lib/vst3/lsp-plugins.vst3",
);
const LSP_COMP_LV2: Plugin = (
    "LSP Compressor Stereo (LV2)",
    "lv2:http://lsp-plug.in/plugins/lv2/compressor_stereo",
    "/usr/lib/lv2/lsp-plugins.lv2",
);
const DRAGONFLY_LV2: Plugin = (
    "Dragonfly Hall Reverb (LV2)",
    "lv2:https://github.com/michaelwillis/dragonfly-reverb",
    "/usr/lib/lv2/DragonflyHallReverb.lv2",
);
/// Its windows are Gtk and Qt5 ones.
const ABGATE_LV2: Plugin = (
    "abGate (LV2)",
    "lv2:http://hippie.lt/lv2/gate",
    "/usr/lib/lv2/abGate.lv2",
);
/// It has no window.
const SWH_AMP_LV2: Plugin = (
    "swh amp (LV2)",
    "lv2:http://plugin.org.uk/swh-plugins/amp",
    "/usr/lib/lv2/amp-swh.lv2",
);

const WAIT: Duration = Duration::from_secs(20);
const QUIET: Duration = Duration::from_millis(300);
/// A block of 256 frames at 48 kHz.
const BLOCK: Duration = Duration::from_micros(5333);

fn installed(plugin: Plugin) -> bool {
    let there = Path::new(plugin.2).exists();
    if !there {
        eprintln!("{} is not installed; nothing tested", plugin.2);
    }
    there
}

fn alone() -> std::sync::MutexGuard<'static, ()> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn load(host: &mut Host, slot: u32, plugin: Plugin) -> (EngineEnd, EditorKind) {
    let key: PluginKey = plugin.1.parse().unwrap();
    let (engine, loaded) = host.load_plugin(slot, &key, plugin.2).unwrap();
    (engine, loaded.editor)
}

fn params(host: &mut Host, slot: u32) -> (Vec<ParamInfo>, Duration) {
    let asked = Instant::now();
    match host.ask(Request::Params { slot }) {
        Reply::Params { params } => (params, asked.elapsed()),
        other => panic!("{other:?}"),
    }
}

fn ms(time: Duration) -> f64 {
    time.as_secs_f64() * 1000.0
}

fn is_opened(notice: &Notice, slot: u32) -> bool {
    matches!(notice, Notice::EditorOpened { slot: of, .. } | Notice::EditorFailed { slot: of, .. } if *of == slot)
}

/// Asks for the window and waits until it is on the screen; how long that took, and its size.
fn open(host: &mut Host, slot: u32, title: &str) -> (Duration, (u32, u32), bool) {
    host.notices.retain(|notice| !is_opened(notice, slot));
    let asked = Instant::now();
    let open = host.ask(Request::OpenEditor {
        slot,
        title: title.into(),
    });
    let opening = EditorOpen::Opening;
    assert_eq!(open, Reply::Editor { open: opening });
    let answered = asked.elapsed();
    assert!(answered < Duration::from_millis(100), "{answered:?}");
    let opened = host.wait_for(WAIT, |notice| is_opened(notice, slot));
    match opened {
        Some(Notice::EditorOpened {
            width,
            height,
            larger_than_screen,
            ..
        }) => (asked.elapsed(), (width, height), larger_than_screen),
        other => panic!("{title}: {other:?}"),
    }
}

fn close(host: &mut Host, slot: u32) -> Duration {
    let asked = Instant::now();
    assert_eq!(host.ask(Request::CloseEditor { slot }), Reply::Ok);
    asked.elapsed()
}

/// One block with `events`, at once; how long the plugin took to answer.
fn timed_block(engine: &mut EngineEnd, events: &[ToPlugin]) -> Duration {
    assert!(engine.begin(256));
    engine.write_input(0, &[0.0; 256]);
    engine.write_input(1, &[0.0; 256]);
    for event in events {
        assert!(engine.push_event(*event));
    }
    let asked = Instant::now();
    engine.submit();
    assert!(engine.wait(monotonic_ns() + common::PATIENT_NS));
    asked.elapsed()
}

/// `blocks` blocks in real time, a key going down and up every 32 blocks. The longest answer
/// and how many answers came later than a block lasts.
fn play(engine: &mut EngineEnd, blocks: usize) -> (Duration, usize) {
    let started = Instant::now();
    let (mut longest, mut late) = (Duration::ZERO, 0);
    for n in 0..blocks {
        let events = match n % 32 {
            0 => vec![note_on(60)],
            24 => vec![note_off(60)],
            _ => vec![],
        };
        let took = timed_block(engine, &events);
        longest = longest.max(took);
        late += usize::from(took > BLOCK);
        let next = BLOCK * (n as u32 + 1);
        std::thread::sleep(next.saturating_sub(started.elapsed()));
    }
    (longest, late)
}

/// Resident and proportional memory of a process, in KiB.
fn memory(pid: u32) -> (u64, u64) {
    let text = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).unwrap();
    let field = |name: &str| {
        (text.lines())
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.split_whitespace().next()?.parse().ok())
            .unwrap_or(0)
    };
    (field("Rss:"), field("Pss:"))
}

/// Everything about one plugin's window and parameters, printed as it is found.
fn look(x: &X, plugin: Plugin) {
    if !installed(plugin) {
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let pid = host.child.0.id();
    let (mut engine, editor) = load(&mut host, 0, plugin);
    assert_eq!(editor, EditorKind::X11, "{}", plugin.0);
    play(&mut engine, 8);

    let (list, list_time) = params(&mut host, 0);
    let groups: BTreeSet<&str> = list.iter().filter_map(|p| p.group.as_deref()).collect();
    let no_group = list.iter().filter(|p| p.group.is_none()).count();
    let count = |shape: fn(&ParamInfo) -> bool| list.iter().filter(|p| shape(p)).count();
    let unknown = list.iter().filter(|p| p.value.is_none()).count();
    eprintln!(
        "{}: {} parameters ({:.1} ms): {} smooth, {} steps, {} toggles; {} unknown values; \
         {} groups + {} in none {:?}",
        plugin.0,
        list.len(),
        ms(list_time),
        count(|p| p.shape == ParamShape::Continuous),
        count(|p| matches!(p.shape, ParamShape::Stepped { .. })),
        count(|p| p.shape == ParamShape::Toggle),
        unknown,
        groups.len(),
        no_group,
        groups.iter().take(6).collect::<Vec<_>>(),
    );
    assert!(!list.is_empty());
    for param in &list {
        assert!((0.0..=1.0).contains(&param.default), "{param:?}");
        assert!(param.value.is_none_or(|v| (0.0..=1.0).contains(&v)));
        if let ParamShape::Stepped { steps } = param.shape {
            assert!(steps >= 3, "{param:?}");
            assert!(param.labels.is_empty() || param.labels.len() == steps as usize);
        }
    }
    let ungrouped: Vec<&str> = (list.iter())
        .filter(|p| p.group.is_none())
        .map(|p| p.name.as_str())
        .collect();
    if !groups.is_empty() {
        let last = ungrouped.len().saturating_sub(3);
        eprintln!(
            "  in no group: {:?} … {:?}; groups: {groups:?}",
            &ungrouped[..ungrouped.len().min(3)],
            &ungrouped[last..]
        );
    }
    if let Some(stepped) = list.iter().find(|p| !p.labels.is_empty()) {
        eprintln!("  steps of {:?}: {:?}", stepped.name, stepped.labels);
    }

    // The texts of a smooth parameter: at both ends, in the middle, and read back.
    // One whose text follows its value, if there is one.
    let smooth = (list.iter())
        .filter(|p| p.shape == ParamShape::Continuous && p.automatable)
        .find(|p| p.group.is_some() || groups.is_empty())
        .expect("a smooth parameter");
    let texts = match host.ask(Request::Texts {
        slot: 0,
        id: smooth.id,
        values: vec![0.0, 0.5, 1.0],
    }) {
        Reply::Texts { texts } => texts,
        other => panic!("{other:?}"),
    };
    let read = |host: &mut Host, text: &str| match host.ask(Request::ReadText {
        slot: 0,
        id: smooth.id,
        text: text.into(),
    }) {
        Reply::Read { read } => read,
        other => panic!("{other:?}"),
    };
    let middle = texts[1].clone().unwrap_or_default();
    let back = read(&mut host, &middle);
    let beyond = read(&mut host, "99999999");
    eprintln!(
        "  {:?}: now {:?} {:?}; texts {:?}; {middle:?} reads as {:?}; \"99999999\" reads as {:?}",
        smooth.name,
        smooth.value,
        smooth.text,
        texts,
        back.as_ref().map(|r| (r.value, r.clamped)),
        beyond
            .as_ref()
            .map(|r: &TextValue| (r.value, r.clamped, r.text.clone())),
    );

    let (took, size, larger) = open(&mut host, 0, plugin.0);
    let window = x
        .wait(pid, plugin.0, true)
        .expect("the window on the screen");
    assert!(x.on_screen(window));
    assert!(x.has_inside(window), "nothing was put into the window");
    play(&mut engine, 40);
    let settled = x.size(window);

    // The keyboard, as a person brings it and takes it away.
    host.wait_for(QUIET, |_| false);
    x.unfocus();
    host.wait_for(QUIET, |_| false);
    host.notices.clear();
    x.focus(window);
    let entered = (host.wait_for(WAIT, |n| *n == Notice::EditorEntered { slot: 0 })).is_some();
    x.unfocus();
    let left = (host.wait_for(WAIT, |n| *n == Notice::EditorLeft { slot: 0 })).is_some();
    let resized: Vec<Notice> = (host.notices.iter())
        .filter(|n| matches!(n, Notice::EditorResized { .. }))
        .cloned()
        .collect();

    // A value set from meloet while the window shows it.
    let set = host.ask(Request::SetParam {
        slot: 0,
        id: smooth.id,
        value: 0.25,
    });
    play(&mut engine, 12);
    let closed = close(&mut host, 0);
    assert_eq!(x.wait(pid, plugin.0, false), None);
    eprintln!(
        "  window: open {:.0} ms, {}x{} (on the screen {}x{}), larger than the screen: {larger}, \
         close {:.1} ms, entered: {entered}, left: {left}, resized: {resized:?}, set: {set:?}",
        ms(took),
        size.0,
        size.1,
        settled.0,
        settled.1,
        ms(closed),
    );
    assert!(entered && left, "{}", plugin.0);

    // Open again, closed by the person this time; then the plugin goes with its window open.
    let (again, _, _) = open(&mut host, 0, plugin.0);
    let window = x.wait(pid, plugin.0, true).expect("the window again");
    play(&mut engine, 12);
    host.notices.clear();
    x.ask_to_close(window);
    assert!(
        (host.wait_for(WAIT, |n| *n == Notice::EditorClosed { slot: 0 })).is_some(),
        "{}",
        plugin.0
    );
    let (third, _, _) = open(&mut host, 0, plugin.0);
    play(&mut engine, 12);
    assert_eq!(host.ask(Request::Unload { slot: 0 }), Reply::Ok);
    assert_eq!(x.wait(pid, plugin.0, false), None);
    assert_eq!(host.ask(Request::Quit), Reply::Ok);
    assert!(host.child.wait_for_exit(WAIT).is_some());
    eprintln!(
        "  opened again in {:.0} ms and {:.0} ms",
        ms(again),
        ms(third)
    );
}

macro_rules! looks {
    ($($name:ident: $plugin:expr,)*) => {$(
        #[test]
        #[ignore = "needs an X display and the plugins of this PC"]
        fn $name() {
            let _alone = alone();
            look(&X::open(), $plugin);
        }
    )*};
}

looks! {
    surge_xt_vst3_window_and_parameters: SURGE_VST3,
    surge_xt_lv2_window_and_parameters: SURGE_LV2,
    zamcomp_vst3_window_and_parameters: ZAMCOMP_VST3,
    zamcomp_lv2_window_and_parameters: ZAMCOMP_LV2,
    lsp_compressor_vst3_window_and_parameters: LSP_COMP_VST3,
    lsp_compressor_lv2_window_and_parameters: LSP_COMP_LV2,
    dragonfly_lv2_window_and_parameters: DRAGONFLY_LV2,
}

#[test]
#[ignore = "needs the plugins of this PC"]
fn windows_meloet_does_not_open_and_plugins_without_one() {
    for (plugin, kind) in [
        (ABGATE_LV2, EditorKind::Unsupported),
        (SWH_AMP_LV2, EditorKind::None),
    ] {
        if !installed(plugin) {
            continue;
        }
        let mut host = Host::start_with(GEOMETRY, &[]);
        let (mut engine, editor) = load(&mut host, 0, plugin);
        assert_eq!(editor, kind, "{}", plugin.0);
        let (list, _) = params(&mut host, 0);
        eprintln!(
            "{}: {editor:?}, {} parameters: {:?}",
            plugin.0,
            list.len(),
            (list.iter())
                .map(|p| (p.name.as_str(), p.text.as_deref()))
                .collect::<Vec<_>>()
        );
        assert!(!list.is_empty());
        // Asked for all the same, the window fails and the plugin is as before.
        let open = host.ask(Request::OpenEditor {
            slot: 0,
            title: plugin.0.into(),
        });
        let opening = EditorOpen::Opening;
        assert_eq!(open, Reply::Editor { open: opening });
        let failed = host.wait_for(WAIT, |n| matches!(n, Notice::EditorFailed { .. }));
        eprintln!("  asked to open: {failed:?}");
        assert!(failed.is_some());
        play(&mut engine, 8);
    }
}

/// Drags upward at points all over the plugin's window, with made-up pointer events, until
/// a knob is hit: what the window then tells of the parameter must reach meloet and the
/// plugin's sound side.
fn drag_a_knob(x: &X, plugin: Plugin, step: usize) {
    if !installed(plugin) {
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let pid = host.child.0.id();
    let (mut engine, _) = load(&mut host, 0, plugin);
    play(&mut engine, 8);
    let (list, _) = params(&mut host, 0);
    let (_, size, _) = open(&mut host, 0, plugin.0);
    let window = x
        .wait(pid, plugin.0, true)
        .expect("the window on the screen");
    play(&mut engine, 60);
    host.notices.clear();
    let mut hit = None;
    'sweep: for at_y in (step / 2..size.1 as usize).step_by(step) {
        for at_x in (step / 2..size.0 as usize).step_by(step) {
            x.drag(window, at_x as i16, at_y as i16, -30);
            play(&mut engine, 6);
            host.wait_for(Duration::from_millis(20), |_| false);
            if (host.notices.iter()).any(|n| matches!(n, Notice::ParamChanged { .. })) {
                hit = Some((at_x, at_y));
                break 'sweep;
            }
            host.notices.clear();
        }
    }
    host.wait_for(QUIET, |_| false);
    let told: Vec<Notice> = (host.notices.iter())
        .filter(|n| {
            matches!(
                n,
                Notice::ParamBegin { .. } | Notice::ParamChanged { .. } | Notice::ParamEnd { .. }
            )
        })
        .cloned()
        .collect();
    eprintln!("{}: a drag at {hit:?} told {told:?}", plugin.0);
    let Some(Notice::ParamChanged {
        id, value, text, ..
    }) = (told.iter().rev()).find(|n| matches!(n, Notice::ParamChanged { .. }))
    else {
        panic!("{}: no knob was hit", plugin.0);
    };
    let name = list.iter().find(|p| p.id == *id).map(|p| p.name.as_str());
    play(&mut engine, 6);
    // What meloet was told last is what the plugin has.
    let now = match host.ask(Request::GetParam { slot: 0, id: *id }) {
        Reply::Param { now } => now,
        other => panic!("{other:?}"),
    };
    eprintln!("  {name:?} is now {now:?}");
    assert!((now.value.unwrap() - value).abs() < 1e-6);
    assert_eq!(&now.text, text);
    close(&mut host, 0);
}

#[test]
#[ignore = "needs an X display and ZamComp"]
fn zamcomp_lv2_knob_dragged_in_its_window() {
    let _alone = alone();
    drag_a_knob(&X::open(), ZAMCOMP_LV2, 30);
}

#[test]
#[ignore = "needs an X display and ZamComp"]
fn zamcomp_vst3_knob_dragged_in_its_window() {
    let _alone = alone();
    drag_a_knob(&X::open(), ZAMCOMP_VST3, 30);
}

#[test]
#[ignore = "needs an X display and Surge XT"]
fn surge_xt_vst3_knob_dragged_in_its_window() {
    let _alone = alone();
    drag_a_knob(&X::open(), SURGE_VST3, 60);
}

/// Opens and closes a window many times, playing all the while.
fn many(x: &X, plugin: Plugin) {
    if !installed(plugin) {
        return;
    }
    let rounds: usize = (std::env::var("MELOET_WINDOW_ROUNDS").ok())
        .and_then(|rounds| rounds.parse().ok())
        .unwrap_or(50);
    let mut host = Host::start_with(GEOMETRY, &[]);
    let pid = host.child.0.id();
    let (mut engine, _) = load(&mut host, 0, plugin);
    play(&mut engine, 100);
    let (longest, late) = play(&mut engine, 3000);
    let (rss, pss) = memory(pid);
    eprintln!(
        "{}: without a window, 3000 blocks: longest answer {:.2} ms, {late} later than a block; \
         RSS {:.1} MB, PSS {:.1} MB",
        plugin.0,
        ms(longest),
        rss as f64 / 1024.0,
        pss as f64 / 1024.0
    );
    let mut opens = Vec::new();
    let (mut longest, mut late, mut blocks) = (Duration::ZERO, 0, 0);
    // The blocks played while a window was being made, and while one was being closed.
    let (mut longest_making, mut late_making, mut blocks_making) = (Duration::ZERO, 0, 0);
    for round in 1..=rounds {
        host.notices.clear();
        let asked = Instant::now();
        let open = host.ask(Request::OpenEditor {
            slot: 0,
            title: plugin.0.into(),
        });
        let opening = EditorOpen::Opening;
        assert_eq!(open, Reply::Editor { open: opening });
        loop {
            let took = timed_block(&mut engine, &[]);
            longest_making = longest_making.max(took);
            late_making += usize::from(took > BLOCK);
            blocks_making += 1;
            let wait = BLOCK.saturating_sub(took).max(Duration::from_millis(1));
            match host.wait_for(wait, |notice| is_opened(notice, 0)) {
                Some(Notice::EditorOpened { .. }) => break,
                Some(failed) => panic!("{failed:?}"),
                None => assert!(asked.elapsed() < WAIT),
            }
        }
        opens.push(asked.elapsed());
        let (long, lates) = play(&mut engine, 60);
        longest = longest.max(long);
        late += lates;
        blocks += 60;
        close(&mut host, 0);
        let (long, lates) = play(&mut engine, 4);
        longest_making = longest_making.max(long);
        late_making += lates;
        blocks_making += 4;
        if round % 10 == 0 || round == 1 {
            // The window is gone from the screen before the memory is read.
            assert_eq!(x.wait(pid, plugin.0, false), None);
            let (rss, pss) = memory(pid);
            eprintln!(
                "  after {round} windows: RSS {:.1} MB, PSS {:.1} MB",
                rss as f64 / 1024.0,
                pss as f64 / 1024.0
            );
        }
    }
    opens.sort();
    eprintln!(
        "  {rounds} windows: open min {:.0} / median {:.0} / max {:.0} ms; with a window, \
         {blocks} blocks: longest answer {:.2} ms, {late} later than a block; while a window \
         was made or closed, {blocks_making} blocks: longest answer {:.2} ms, {late_making} \
         later than a block",
        ms(opens[0]),
        ms(opens[opens.len() / 2]),
        ms(opens[opens.len() - 1]),
        ms(longest),
        ms(longest_making),
    );
    // Still there and sounding.
    play(&mut engine, 20);
    assert_eq!(host.ask(Request::Unload { slot: 0 }), Reply::Ok);
    assert_eq!(host.ask(Request::Quit), Reply::Ok);
    assert_eq!(
        host.child
            .wait_for_exit(WAIT)
            .and_then(|status| status.code()),
        Some(0)
    );
}

#[test]
#[ignore = "a measurement: needs an X display, Surge XT and a quiet PC"]
fn surge_xt_vst3_window_many_times() {
    let _alone = alone();
    many(&X::open(), SURGE_VST3);
}

#[test]
#[ignore = "a measurement: needs an X display, ZamComp and a quiet PC"]
fn zamcomp_lv2_window_many_times() {
    let _alone = alone();
    many(&X::open(), ZAMCOMP_LV2);
}
